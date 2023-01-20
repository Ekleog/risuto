use anyhow::Context;
use axum::{
    extract::{ws::Message, State, WebSocketUpgrade},
    routing::{get, post},
    Json, Router,
};
use futures::{SinkExt, StreamExt};
use risuto_api::Error as ApiError;
use risuto_api::{
    Action, AuthInfo, AuthToken, Event, NewSession, NewUser, Search, Tag, Task, User, UserId, Uuid,
};
use std::net::SocketAddr;
use tower_http::trace::TraceLayer;

mod db;
mod extractors;
mod feeds;
mod fuzz;
mod query;

use extractors::*;
use feeds::UserFeeds;

#[derive(Debug, structopt::StructOpt)]
struct Opt {
    /// Enable the admin interface. This will print the admin token to risuto-server's stdout.
    ///
    /// Note that the admin token changes on each server start.
    #[structopt(long)]
    enable_admin: bool,
}

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = <Opt as structopt::StructOpt>::from_args();

    tracing_subscriber::fmt::init();

    let db_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    let db = PgPool::new(
        sqlx::postgres::PgPoolOptions::new()
            .max_connections(8)
            .connect(&db_url)
            .await
            .with_context(|| format!("Error opening database {:?}", db_url))?,
    );
    MIGRATOR
        .run(
            &mut *db
                .acquire()
                .await
                .context("acquiring conn for migration running")?,
        )
        .await
        .context("running pending migrations")?;

    let admin_token = match opt.enable_admin {
        false => None,
        true => {
            let t = Uuid::new_v4();
            // Do NOT go through tracing, as it could end up in various metrics collection things
            println!("admin interface enabled; admin token is {t:?}");
            Some(AuthToken(t))
        }
    };

    let app = app(db, admin_token).await;

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .context("serving axum webserver")
}

async fn app(db: PgPool, admin_token: Option<AuthToken>) -> Router {
    let feeds = UserFeeds::new();

    let state = AppState {
        db,
        feeds,
        admin_token,
    };

    Router::new()
        .route("/api/admin/create-user", post(admin_create_user))
        .route("/api/auth", post(auth))
        .route("/api/unauth", post(unauth))
        .route("/api/whoami", get(whoami))
        .route("/api/fetch-users", get(fetch_users))
        .route("/api/fetch-tags", get(fetch_tags))
        .route("/api/fetch-searches", get(fetch_searches))
        .route("/api/search-tasks", post(search_tasks))
        .route("/ws/action-feed", get(action_feed))
        .route("/api/submit-action", post(submit_action))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),

    #[error(transparent)]
    Api(#[from] ApiError),
}

impl Error {
    fn permission_denied() -> Error {
        Error::Api(ApiError::PermissionDenied)
    }

    fn uuid_already_used(uuid: Uuid) -> Error {
        Error::Api(ApiError::UuidAlreadyUsed(uuid))
    }

    fn name_already_used(name: String) -> Error {
        Error::Api(ApiError::NameAlreadyUsed(name))
    }

    fn invalid_pow() -> Error {
        Error::Api(ApiError::InvalidPow)
    }
}

impl axum::response::IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let err = match self {
            Error::Anyhow(err) => {
                tracing::error!(?err, "internal server error");
                ApiError::Unknown(String::from("Internal server error, see logs for details"))
            }
            Error::Api(err) => {
                tracing::info!("returning error to client: {err}");
                err
            }
        };
        (err.status_code(), err.contents()).into_response()
    }
}

async fn admin_create_user(
    AdminAuth: AdminAuth,
    State(feeds): State<UserFeeds>,
    mut conn: PgConn,
    Json(data): Json<NewUser>,
) -> Result<(), Error> {
    data.validate()?;
    db::create_user(&mut *conn, data.clone()).await?;
    feeds
        .relay_action(
            &mut *conn,
            Action::NewUser(User {
                id: data.id,
                name: data.name,
            }),
        )
        .await;
    Ok(())
}

async fn auth(mut conn: PgConn, Json(data): Json<NewSession>) -> Result<Json<AuthToken>, Error> {
    data.validate_except_pow()?;
    // in test setup, also allow the "empty" pow to work
    #[cfg(test)]
    if !data.verify_pow() && !data.pow.is_empty() {
        return Err(Error::invalid_pow());
    }
    #[cfg(not(test))]
    if !data.verify_pow() {
        return Err(Error::invalid_pow());
    }
    Ok(Json(
        db::login_user(&mut *conn, &data)
            .await
            .context("logging user in")?
            .ok_or(Error::permission_denied())?,
    ))
}

async fn unauth(user: PreAuth, mut conn: PgConn) -> Result<(), Error> {
    match db::logout_user(&mut *conn, &user.0).await {
        Ok(true) => Ok(()),
        Ok(false) => Err(Error::permission_denied()),
        Err(e) => Err(Error::Anyhow(e)),
    }
}

async fn whoami(Auth(user): Auth) -> Json<UserId> {
    Json(user)
}

async fn fetch_users(Auth(user): Auth, mut conn: PgConn) -> Result<Json<Vec<User>>, Error> {
    Ok(Json(db::fetch_users(&mut *conn).await.with_context(
        || format!("fetching user list for {:?}", user),
    )?))
}

async fn fetch_tags(
    Auth(user): Auth,
    mut conn: PgConn,
) -> Result<Json<Vec<(Tag, AuthInfo)>>, Error> {
    Ok(Json(
        db::fetch_tags_for_user(&mut *conn, &user)
            .await
            .with_context(|| format!("fetching tag list for {:?}", user))?,
    ))
}

async fn fetch_searches(Auth(user): Auth, mut conn: PgConn) -> Result<Json<Vec<Search>>, Error> {
    Ok(Json(
        db::fetch_searches_for_user(&mut *conn, &user)
            .await
            .with_context(|| format!("fetching saved search list for {:?}", user))?,
    ))
}

async fn search_tasks(
    Auth(user): Auth,
    mut conn: PgConn,
    Json(q): Json<risuto_api::Query>,
) -> Result<Json<(Vec<Task>, Vec<Event>)>, Error> {
    Ok(Json(
        db::search_tasks_for_user(&mut *conn, user, &q)
            .await
            .with_context(|| format!("fetching task list for {:?}", user))?,
    ))
}

async fn submit_action(
    Auth(user): Auth,
    State(feeds): State<UserFeeds>,
    mut conn: PgConn,
    Json(a): Json<Action>,
) -> Result<(), Error> {
    let mut db = db::PostgresDb {
        conn: &mut *conn,
        user,
    };
    match &a {
        Action::NewUser(_) => return Err(Error::permission_denied()),
        Action::NewTask(t, top_comm) => {
            if user != t.owner_id {
                return Err(Error::permission_denied());
            }
            db::submit_task(&mut db, t.clone(), top_comm.clone()).await?;
        }
        Action::NewEvent(e) => {
            if user != e.owner_id {
                return Err(Error::permission_denied());
            }
            db::submit_event(&mut db, e.clone()).await?;
        }
    }
    feeds.relay_action(&mut db.conn, a).await;
    Ok(())
}

async fn action_feed(
    ws: WebSocketUpgrade,
    State(db): State<PgPool>,
    State(feeds): State<UserFeeds>,
) -> Result<axum::response::Response, Error> {
    Ok(ws.on_upgrade(move |sock| {
        let (write, read) = sock.split();
        action_feed_impl(write, read, db, feeds)
    }))
}

async fn action_feed_impl<W, R>(mut write: W, mut read: R, db: PgPool, feeds: UserFeeds)
where
    W: 'static + Send + Unpin + futures::Sink<Message>,
    <W as futures::Sink<Message>>::Error: Send,
    R: 'static + Send + Unpin + futures::Stream<Item = Result<Message, axum::Error>>,
{
    // TODO: handle errors more gracefully
    // TODO: also log ip of other websocket end
    tracing::debug!("event feed websocket connected");
    if let Some(Ok(Message::Text(token))) = read.next().await {
        if let Ok(token) = Uuid::try_from(&token as &str) {
            if let Ok(mut conn) = db.acquire().await {
                if let Ok(user) = db::recover_session(&mut *conn, AuthToken(token)).await {
                    if let Ok(_) = write.send(Message::Text(String::from("ok"))).await {
                        tracing::debug!(?user, "event feed websocket auth success");
                        feeds.add_for_user(user, write, read).await;
                        return;
                    }
                }
            }
        }
        tracing::debug!(?token, "event feed websocket auth failure");
        let _ = write
            .send(Message::Text(String::from("permission denied")))
            .await;
    }
}
