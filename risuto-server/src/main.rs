use anyhow::Context;
use axum::{
    async_trait,
    extract::{ws::Message, FromRef, FromRequestParts, State, WebSocketUpgrade},
    http::{self, request, StatusCode},
    routing::{get, post},
    Json, Router,
};
use futures::{channel::mpsc, select, stream, SinkExt, Stream, StreamExt};
use risuto_api::{
    Action, AuthInfo, AuthToken, Event, FeedMessage, NewSession, NewUser, Search, Tag, Task, User,
    UserId, Uuid,
};
use serde_json::json;
use std::{collections::HashMap, iter, net::SocketAddr, pin::Pin, sync::Arc};
use tokio::sync::RwLock;
use tower_http::trace::TraceLayer;

mod db;
mod query;

#[derive(Debug, structopt::StructOpt)]
struct Opt {
    /// Enable the admin interface. This will print the admin token to risuto-server's stdout.
    ///
    /// Note that the admin token changes on each server start.
    #[structopt(long)]
    enable_admin: bool,
}

#[derive(Clone, FromRef)]
struct AppState {
    db: sqlx::PgPool,
    feeds: UserFeeds,
    admin_token: Option<AuthToken>,
}

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = <Opt as structopt::StructOpt>::from_args();

    tracing_subscriber::fmt::init();

    let db_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    let db = sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect(&db_url)
        .await
        .with_context(|| format!("Error opening database {:?}", db_url))?;
    MIGRATOR
        .run(&db)
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

async fn app(db: sqlx::PgPool, admin_token: Option<AuthToken>) -> Router {
    let feeds = UserFeeds(Arc::new(RwLock::new(HashMap::new())));

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

struct PreAuth(AuthToken);

#[async_trait]
impl<S: Sync> FromRequestParts<S> for PreAuth {
    type Rejection = Error;

    async fn from_request_parts(req: &mut request::Parts, _state: &S) -> Result<PreAuth, Error> {
        match req.headers.get(http::header::AUTHORIZATION) {
            None => Err(Error::PermissionDenied),
            Some(auth) => {
                let auth = auth.to_str().map_err(|_| Error::PermissionDenied)?;
                let mut auth = auth.split(' ');
                if !auth
                    .next()
                    .ok_or(Error::PermissionDenied)?
                    .eq_ignore_ascii_case("bearer")
                {
                    return Err(Error::PermissionDenied);
                }
                let token = auth.next().ok_or(Error::PermissionDenied)?;
                if !auth.next().is_none() {
                    return Err(Error::PermissionDenied);
                }
                let token = Uuid::try_from(token).map_err(|_| Error::PermissionDenied)?;
                Ok(PreAuth(AuthToken(token)))
            }
        }
    }
}

struct Auth(UserId);

#[async_trait]
impl FromRequestParts<AppState> for Auth {
    type Rejection = Error;

    async fn from_request_parts(req: &mut request::Parts, state: &AppState) -> Result<Auth, Error> {
        let token = PreAuth::from_request_parts(req, state).await?.0;
        let mut conn = state
            .db
            .acquire()
            .await
            .context("getting connection to database")?;
        Ok(Auth(db::recover_session(&mut conn, token).await?))
    }
}

struct AdminAuth;

#[async_trait]
impl FromRequestParts<AppState> for AdminAuth {
    type Rejection = Error;

    async fn from_request_parts(
        req: &mut request::Parts,
        state: &AppState,
    ) -> Result<AdminAuth, Error> {
        let token = PreAuth::from_request_parts(req, state).await?.0;
        if Some(token) == state.admin_token {
            Ok(AdminAuth)
        } else {
            Err(Error::PermissionDenied)
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),

    #[error("Permission denied")]
    PermissionDenied,

    #[error("Uuid already used {0}")]
    UuidAlreadyUsed(Uuid),

    #[error("Invalid Proof of Work")]
    InvalidPow,
}

impl axum::response::IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        match self {
            Error::Anyhow(err) => {
                tracing::error!(?err, "got an error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error, see logs for details",
                )
                    .into_response()
            }
            Error::PermissionDenied => {
                tracing::info!("returning permission denied to client");
                (StatusCode::FORBIDDEN, "Permission denied").into_response()
            }
            Error::UuidAlreadyUsed(id) => {
                tracing::info!("Conflicting uuid sent by client");
                (
                    StatusCode::CONFLICT,
                    Json(json!({ "error": "uuid already used", "uuid": id })),
                )
                    .into_response()
            }
            Error::InvalidPow => {
                tracing::info!("client sent an invalid proof of work");
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": "invalid proof of work" })),
                )
                    .into_response()
            }
        }
    }
}

async fn admin_create_user(
    AdminAuth: AdminAuth,
    State(db): State<sqlx::PgPool>,
    State(feeds): State<UserFeeds>,
    Json(data): Json<NewUser>,
) -> Result<(), Error> {
    let mut conn = db.acquire().await.context("acquiring db connection")?;
    db::create_user(&mut conn, data.clone()).await?;
    feeds
        .relay_action(
            &mut conn,
            Action::NewUser(User {
                id: data.id,
                name: data.name,
            }),
        )
        .await;
    Ok(())
}

async fn auth(
    State(db): State<sqlx::PgPool>,
    Json(data): Json<NewSession>,
) -> Result<Json<AuthToken>, Error> {
    // in test setup, also allow the "empty" pow to work
    #[cfg(test)]
    if !data.verify_pow() && !data.pow.is_empty() {
        return Err(Error::InvalidPow);
    }
    #[cfg(not(test))]
    if !data.verify_pow() {
        return Err(Error::InvalidPow);
    }
    let mut conn = db.acquire().await.context("acquiring db connection")?;
    Ok(Json(
        db::login_user(&mut conn, &data)
            .await
            .context("logging user in")?
            .ok_or(Error::PermissionDenied)?,
    ))
}

async fn unauth(user: PreAuth, State(db): State<sqlx::PgPool>) -> Result<(), Error> {
    let mut conn = db.acquire().await.context("acquiring db connection")?;
    match db::logout_user(&mut conn, &user.0).await {
        Ok(true) => Ok(()),
        Ok(false) => Err(Error::PermissionDenied),
        Err(e) => Err(Error::Anyhow(e)),
    }
}

async fn whoami(Auth(user): Auth) -> Json<UserId> {
    Json(user)
}

async fn fetch_users(
    Auth(user): Auth,
    State(db): State<sqlx::PgPool>,
) -> Result<Json<Vec<User>>, Error> {
    let mut conn = db.acquire().await.context("acquiring db connection")?;
    Ok(Json(db::fetch_users(&mut conn).await.with_context(
        || format!("fetching user list for {:?}", user),
    )?))
}

async fn fetch_tags(
    Auth(user): Auth,
    State(db): State<sqlx::PgPool>,
) -> Result<Json<Vec<(Tag, AuthInfo)>>, Error> {
    let mut conn = db.acquire().await.context("acquiring db connection")?;
    Ok(Json(
        db::fetch_tags_for_user(&mut conn, &user)
            .await
            .with_context(|| format!("fetching tag list for {:?}", user))?,
    ))
}

async fn fetch_searches(
    Auth(user): Auth,
    State(db): State<sqlx::PgPool>,
) -> Result<Json<Vec<Search>>, Error> {
    let mut conn = db.acquire().await.context("acquiring db connection")?;
    Ok(Json(
        db::fetch_searches_for_user(&mut conn, &user)
            .await
            .with_context(|| format!("fetching saved search list for {:?}", user))?,
    ))
}

async fn search_tasks(
    Auth(user): Auth,
    State(db): State<sqlx::PgPool>,
    Json(q): Json<risuto_api::Query>,
) -> Result<Json<(Vec<Task>, Vec<Event>)>, Error> {
    let mut conn = db.acquire().await.context("acquiring db connection")?;
    Ok(Json(
        db::search_tasks_for_user(&mut conn, user, &q)
            .await
            .with_context(|| format!("fetching task list for {:?}", user))?,
    ))
}

async fn submit_action(
    Auth(user): Auth,
    State(db): State<sqlx::PgPool>,
    State(feeds): State<UserFeeds>,
    Json(a): Json<Action>,
) -> Result<(), Error> {
    let mut conn = db.acquire().await.context("acquiring db connection")?;
    let mut db = db::PostgresDb {
        conn: &mut conn,
        user,
    };
    match &a {
        Action::NewUser(_) => return Err(Error::PermissionDenied),
        Action::NewTask(t, top_comm) => {
            if user != t.owner_id {
                return Err(Error::PermissionDenied);
            }
            db::submit_task(&mut db, t.clone(), top_comm.clone()).await?;
        }
        Action::NewEvent(e) => {
            if user != e.owner_id {
                return Err(Error::PermissionDenied);
            }
            db::submit_event(&mut db, e.clone()).await?;
        }
    }
    feeds.relay_action(&mut db.conn, a).await;
    Ok(())
}

async fn action_feed(
    ws: WebSocketUpgrade,
    State(db): State<sqlx::PgPool>,
    State(feeds): State<UserFeeds>,
) -> Result<axum::response::Response, Error> {
    Ok(ws.on_upgrade(move |sock| {
        let (write, read) = sock.split();
        action_feed_impl(write, read, db, feeds)
    }))
}

async fn action_feed_impl<W, R>(mut write: W, mut read: R, db: sqlx::PgPool, feeds: UserFeeds)
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
                if let Ok(user) = db::recover_session(&mut conn, AuthToken(token)).await {
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

#[derive(Clone, Debug)]
struct UserFeeds(Arc<RwLock<HashMap<UserId, HashMap<Uuid, mpsc::UnboundedSender<FeedMessage>>>>>);

impl UserFeeds {
    async fn add_for_user<W, R>(self, user: UserId, mut write: W, read: R)
    where
        W: 'static + Send + Unpin + futures::Sink<Message>,
        <W as futures::Sink<Message>>::Error: Send,
        R: 'static + Send + Unpin + futures::Stream<Item = Result<Message, axum::Error>>,
    {
        // Create relayer channel
        // Note: if this were bounded, there would be a deadlock between the write-lock to remove a channel and the read-lock to send an event to all interested sockets
        let (sender, mut receiver) = mpsc::unbounded();
        let sender_id = Uuid::new_v4();

        // Add relayer endpoint to hashmap
        // TODO: limit to some reasonable number of sockets, to avoid starvations
        self.0
            .write()
            .await
            .entry(user)
            .or_insert_with(HashMap::new)
            .insert(sender_id, sender);

        // Start relayer queue
        let this = self.clone();
        let user = user.clone();
        let mut read = read.fuse();
        tokio::spawn(async move {
            macro_rules! remove_self {
                () => {{
                    this.0
                        .write()
                        .await
                        .get_mut(&user)
                        .expect("user {user:?} disappeared")
                        .remove(&sender_id);
                    return;
                }};
            }
            macro_rules! send_message {
                ( $msg:expr ) => {{
                    let msg: FeedMessage = $msg;
                    let json = match serde_json::to_vec(&msg) {
                        Ok(json) => json,
                        Err(err) => {
                            tracing::error!(?err, ?msg, "failed serializing message to json");
                            continue;
                        }
                    };
                    if let Err(_) = write.send(Message::Binary(json)).await {
                        // TODO: check error details, using axum-tungstenite, to confirm we need to remove this socket
                        remove_self!();
                    }
                }};
            }
            loop {
                select! {
                    msg = receiver.next() => match msg {
                        None => remove_self!(),
                        Some(msg) => send_message!(msg),
                    },
                    msg = read.next() => match msg {
                        None => remove_self!(),
                        Some(Ok(Message::Text(msg))) => {
                            if msg != "ping" {
                                tracing::warn!("received unexpected message from client: {msg:?}");
                                remove_self!();
                            }
                            send_message!(FeedMessage::Pong);
                        }
                        Some(Ok(Message::Close(_))) => remove_self!(),
                        Some(msg) => {
                            tracing::warn!("received unexpected message from client: {msg:?}");
                            remove_self!();
                        }
                    },
                }
            }
        });
    }

    async fn relay_action(&self, conn: &mut sqlx::PgConnection, a: Action) {
        match &a {
            Action::NewUser(_) => match db::fetch_users(conn).await {
                Err(e) => Box::pin(stream::iter(iter::once(Err(e))))
                    as Pin<Box<dyn Send + Stream<Item = anyhow::Result<UserId>>>>,
                Ok(u) => Box::pin(stream::iter(u.into_iter().map(|u| Ok(u.id)))),
            },
            Action::NewTask(t, _) => Box::pin(stream::iter(iter::once(Ok(t.owner_id)))),
            Action::NewEvent(e) => Box::pin(db::users_interested_by(conn, &[e.task_id.0])),
            // TODO: make sure we actually send the whole task if a user gets access to this task it didn't have before
        }
        // TODO: magic numbers below should be at least explained
        .for_each_concurrent(Some(16), |u| {
            let a = a.clone();
            async move {
                match u {
                    Err(err) => {
                        tracing::error!(?err, "error occurred while listing interested users");
                    }
                    Ok(u) => {
                        if let Some(socks) = self.0.read().await.get(&u) {
                            for s in socks.values() {
                                let _ = s.unbounded_send(FeedMessage::Action(a.clone()));
                            }
                        }
                    }
                }
            }
        })
        .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http;
    use sqlx::testing::TestSupport;
    use std::panic::AssertUnwindSafe;
    use tower::{Service, ServiceExt};

    macro_rules! do_tokio_test {
        ( $name:ident, $typ:ty, $fn:expr ) => {
            #[test]
            fn $name() {
                let runtime = AssertUnwindSafe(
                    tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("failed initializing tokio runtime"),
                );
                bolero::check!()
                    .with_type::<$typ>()
                    .cloned()
                    .for_each(move |v| {
                        let () = runtime.block_on($fn(v));
                    })
            }
        };
    }

    macro_rules! do_sqlx_test {
        ( $name:ident, $typ:ty, $fn:expr ) => {
            #[test]
            fn $name() {
                let runtime = AssertUnwindSafe(
                    tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("failed initializing tokio runtime"),
                );
                // create test db
                let pool = AssertUnwindSafe(runtime.block_on(async move {
                    let test_context = sqlx::Postgres::test_context(&sqlx::testing::TestArgs::new(
                        concat!(module_path!(), "::", stringify!($name)),
                    ))
                    .await
                    .expect("failed connecting to setup test db");
                    let pool = test_context
                        .pool_opts
                        .connect_with(test_context.connect_opts)
                        .await
                        .expect("failed connecting test pool");
                    MIGRATOR
                        .run(&mut pool.acquire().await.expect("getting migrator connection"))
                        .await
                        .expect("failed applying migrations");
                    pool
                }));
                let cleanup_queries = include_str!("../reset-test-db.sql")
                    .split(";")
                    .collect::<Vec<_>>();
                let cleanup_queries: &[&str] = &cleanup_queries;
                bolero::check!()
                    .with_type::<$typ>()
                    .cloned()
                    .for_each(move |v| {
                        let pool = pool.clone();
                        runtime.block_on(async move {
                            // run the test
                            let idle_before = pool.num_idle();
                            let () = $fn(pool.clone(), v).await;
                            // cleanup
                            assert_eq!(
                                pool.num_idle(),
                                idle_before,
                                "test {} held onto pool after exiting",
                                stringify!($name)
                            );
                            let mut conn =
                                pool.acquire().await.expect("getting db cleanup connection");
                            for q in cleanup_queries {
                                sqlx::query(&q)
                                    .execute(&mut conn)
                                    .await
                                    .expect("failed cleaning up database");
                            }
                        });
                    });
            }
        };
    }

    do_tokio_test!(fuzz_preauth_extractor, String, |token| async move {
        if let Ok(req) = http::Request::builder()
            .method(http::Method::GET)
            .uri("/")
            .header(http::header::AUTHORIZATION, token)
            .body(())
        {
            let mut req = req.into_parts().0;
            let res = PreAuth::from_request_parts(&mut req, &()).await;
            match res {
                Ok(_) => (),
                Err(Error::PermissionDenied) => (),
                Err(e) => panic!("got unexpected error: {e}"),
            }
        }
    });

    // TODO: also allow generating invalid requests?
    #[derive(Clone, Debug, bolero::generator::TypeGenerator)]
    enum FuzzOp {
        CreateUser {
            id: UserId,
            name: String,
            pass: String,
        },
        Auth {
            uid: usize,
            device: String,
        },
        Unauth {
            sid: usize,
        },
        Whoami {
            sid: usize,
        },
        FetchUsers {
            sid: usize,
        },
        FetchTags {
            sid: usize,
        },
        FetchSearches {
            sid: usize,
        },
        SearchTasks {
            sid: usize,
            query: risuto_api::Query,
        },
        SubmitAction {
            sid: usize,
            evt: risuto_api::Action,
        },
        OpenActionFeed {
            sid: usize,
        },
        CloseActionFeed {
            feed_id: usize,
        },
    }

    do_sqlx_test!(
        auth_extractor_no_500,
        Vec<FuzzOp>,
        |pool, test| async move {
            let admin_token = Uuid::new_v4();
            let app = app(pool, Some(AuthToken(admin_token))).await;
            let mock = risuto_mock_server::MockServer::new();
            for op in test {
                match op {
                    FuzzOp::CreateUser { id, name, pass } => {
                        let new_user = NewUser {
                            id,
                            name,
                            initial_password_hash: pass,
                        };
                        app.ready().await;
                        let app_res = app
                            .call(
                                request::Builder::new()
                                    .method("POST")
                                    .uri("/api/admin/create-user")
                                    .header(
                                        http::header::AUTHORIZATION,
                                        format!("bearer {admin_token}"),
                                    )
                                    .body(axum::body::Body::from(
                                        serde_json::to_vec(&new_user)
                                            .expect("serializing request to json"),
                                    ))
                                    .expect("building test request"),
                            )
                            .await
                            .expect("running request");
                        let mock_res = mock.admin_create_user(new_user);
                        assert_eq!(
                            app_res, mock_res,
                            "app and mock did not return the same result for CreateUser"
                        );
                    }
                }
            }
        }
    );
}
