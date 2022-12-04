use anyhow::Context;
use arrayvec::ArrayVec;
use axum::{
    async_trait,
    extract::{
        ws::{Message, WebSocket},
        FromRequest, RequestParts, WebSocketUpgrade,
    },
    http::StatusCode,
    routing::{get, post},
    Extension, Json, Router,
};
use futures::{stream, StreamExt};
use risuto_api::{AuthToken, DbDump, NewSession, UserId, Uuid};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::sync::{Mutex, RwLock};
use tower_http::trace::TraceLayer;

mod db;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let db_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    let db = sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect(&db_url)
        .await
        .with_context(|| format!("Error opening database {:?}", db_url))?;

    let feeds = UserFeeds(Arc::new(RwLock::new(HashMap::new())));

    let app = Router::new()
        .route("/api/auth", post(auth))
        .route("/api/unauth", post(unauth))
        .route("/api/fetch-unarchived", get(fetch_unarchived))
        .route("/ws/event-feed", get(event_feed))
        .route("/api/submit-event", post(submit_event))
        .layer(Extension(db))
        .layer(Extension(feeds))
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .context("serving axum webserver")
}

struct PreAuth(AuthToken);

#[async_trait]
impl<B: Send + Sync> FromRequest<B> for PreAuth {
    type Rejection = Error;

    async fn from_request(req: &mut RequestParts<B>) -> Result<PreAuth, Error> {
        match req.headers().get(axum::http::header::AUTHORIZATION) {
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
impl<B: Send + Sync> FromRequest<B> for Auth {
    type Rejection = Error;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Auth, Error> {
        let token = req.extract::<PreAuth>().await?.0;
        let db = req
            .extract::<Extension<sqlx::PgPool>>()
            .await
            .context("recovering PgPool extension")?;
        let mut conn = db
            .acquire()
            .await
            .context("getting connection to database")?;
        Ok(Auth(db::recover_session(&mut conn, token).await?))
    }
}

pub enum Error {
    Anyhow(anyhow::Error),
    PermissionDenied,
}

impl From<anyhow::Error> for Error {
    fn from(e: anyhow::Error) -> Error {
        Error::Anyhow(e)
    }
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
        }
    }
}

async fn auth(
    Extension(db): Extension<sqlx::PgPool>,
    Json(data): Json<NewSession>,
) -> Result<Json<AuthToken>, Error> {
    let mut conn = db.acquire().await.context("acquiring db connection")?;
    Ok(Json(
        db::login_user(&mut conn, &data)
            .await
            .context("logging user in")?
            .ok_or(Error::PermissionDenied)?,
    ))
}

async fn unauth(user: PreAuth, Extension(db): Extension<sqlx::PgPool>) -> Result<(), Error> {
    let mut conn = db.acquire().await.context("acquiring db connection")?;
    match db::logout_user(&mut conn, &user.0).await {
        Ok(true) => Ok(()),
        Ok(false) => Err(Error::PermissionDenied),
        Err(e) => Err(Error::Anyhow(e)),
    }
}

async fn fetch_unarchived(
    Auth(user): Auth,
    Extension(db): Extension<sqlx::PgPool>,
) -> Result<Json<DbDump>, Error> {
    let mut conn = db.acquire().await.context("acquiring db connection")?;
    Ok(Json(
        db::fetch_dump_unarchived(&mut conn, user)
            .await
            .with_context(|| format!("fetching db dump for {:?}", user))?,
    ))
}

async fn submit_event(
    Json(e): Json<risuto_api::NewEvent>,
    Auth(user): Auth,
    Extension(db): Extension<sqlx::PgPool>,
    Extension(feeds): Extension<UserFeeds>,
) -> Result<(), Error> {
    if user != e.owner {
        return Err(Error::PermissionDenied);
    }
    let mut conn = db.acquire().await.context("acquiring db connection")?;
    db::submit_event(&mut conn, e.clone()).await?;
    let db = db::PostgresDb {
        conn: &mut conn,
        user,
    };
    feeds.relay_event(db, e).await;
    Ok(())
}

async fn event_feed(
    ws: WebSocketUpgrade,
    Extension(db): Extension<sqlx::PgPool>,
    Extension(feeds): Extension<UserFeeds>,
) -> Result<axum::response::Response, Error> {
    Ok(ws.on_upgrade(move |mut sock| async move {
        // TODO: handle errors more gracefully
        // TODO: also log ip of other websocket end
        tracing::debug!("event feed websocket connected");
        if let Some(Ok(Message::Text(token))) = sock.recv().await {
            if let Ok(token) = Uuid::try_from(&token as &str) {
                if let Ok(mut conn) = db.acquire().await {
                    if let Ok(user) = db::recover_session(&mut conn, AuthToken(token)).await {
                        if let Ok(_) = sock.send(Message::Text(String::from("ok"))).await {
                            tracing::debug!(?user, "event feed websocket auth success");
                            feeds.clone().add_for_user(user, sock).await;
                        }
                    }
                }
            }
        }
    }))
}

#[derive(Clone, Debug)]
struct UserFeeds(Arc<RwLock<HashMap<UserId, Vec<Mutex<WebSocket>>>>>);

impl UserFeeds {
    async fn add_for_user(self, user: UserId, sock: WebSocket) {
        // TODO: limit to some reasonable number of sockets, to avoid starvations
        self.0
            .write()
            .await
            .entry(user)
            .or_insert_with(Vec::new)
            .push(Mutex::new(sock));
    }

    async fn relay_event(&self, mut db: db::PostgresDb<'_>, mut e: risuto_api::NewEvent) {
        if let Err(err) = e.make_untrusted_trusted(&mut db).await {
            tracing::error!(?err, "failed to make untrusted event {:?} trusted", e);
            return;
        }

        let json = match serde_json::to_vec(&e) {
            Ok(json) => json,
            Err(err) => {
                tracing::error!(?err, event=?e, "failed to serialize evetn to json");
                return;
            }
        };
        let json_ref = &json[..];

        let tasks = e
            .untrusted_task_event_list()
            .into_iter()
            .map(|(t, _)| t.0)
            .collect::<ArrayVec<_, 2>>();

        // TODO: magic numbers below should be at least explained
        db::users_interested_by(&mut db.conn, &tasks)
            .for_each_concurrent(Some(16), |u| async move {
                match u {
                    Err(err) => {
                        tracing::error!(?err, "error occurred while listing interested users");
                    }
                    Ok(u) => {
                        let ids_to_rm = if let Some(socks) = self.0.read().await.get(&u) {
                            stream::iter(socks.iter().enumerate())
                                .filter_map(|(i, s)| async move {
                                    if let Err(_) = s
                                        .lock()
                                        .await
                                        .send(Message::Binary(json_ref.to_vec()))
                                        .await
                                    {
                                        // TODO: check error details, using axum-tungstenite, to confirm we need to remove this socket
                                        Some(i)
                                    } else {
                                        None
                                    }
                                })
                                .collect::<Vec<_>>()
                                .await
                        } else {
                            Vec::new()
                        };
                        if !ids_to_rm.is_empty() {
                            let mut users = self.0.write().await;
                            let socks = users
                                .get_mut(&u)
                                .expect("we should never remove UserId's from UserFeeds");
                            for i in ids_to_rm.into_iter().rev() {
                                // iterate from last to remove to first, as we can swap with the last item
                                socks.swap_remove(i);
                            }
                        }
                    }
                }
            })
            .await;
    }
}
