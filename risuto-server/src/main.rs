use anyhow::Context;
use arrayvec::ArrayVec;
use axum::{
    extract::{
        ws::{Message, WebSocket},
        WebSocketUpgrade,
    },
    http::{self, Request, StatusCode},
    middleware::Next,
    response::Response,
    routing::{get, post},
    Extension, Json, Router,
};
use futures::{stream, StreamExt};
use risuto_api::{DbDump, UserId};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::sync::{Mutex, RwLock};

mod db;

#[derive(Clone, Debug)]
struct Auth(Option<CurrentUser>);

#[derive(Clone, Debug)]
struct CurrentUser {
    id: UserId,
}

async fn auth<B: std::fmt::Debug>(
    mut req: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    if let Some(auth) = req.headers().get(http::header::AUTHORIZATION) {
        let auth = auth.to_str().map_err(|_| StatusCode::UNAUTHORIZED)?;

        let db = req
            .extensions()
            .get::<sqlx::PgPool>()
            .expect("No sqlite pool extension");
        if let Some(current_user) = authorize_current_user(db, auth).await {
            req.extensions_mut().insert(Auth(Some(current_user)));
            Ok(next.run(req).await)
        } else {
            Err(StatusCode::UNAUTHORIZED)
        }
    } else {
        req.extensions_mut().insert(Auth(None));
        Ok(next.run(req).await)
    }
}

async fn authorize_current_user(db: &sqlx::PgPool, auth: &str) -> Option<CurrentUser> {
    let split = auth.split(' ').collect::<Vec<_>>();
    if split.len() != 2 || split[0] != "Basic" {
        return None;
    }

    let userpass = base64::decode(split[1]).ok()?;
    let userpass = std::str::from_utf8(&userpass).ok()?;
    let split = userpass.split(':').collect::<Vec<_>>();
    if split.len() != 2 {
        return None;
    }

    let user = sqlx::query!(
        "SELECT id FROM users WHERE name = $1 AND password = $2",
        split[0],
        split[1]
    )
    .fetch_one(db)
    .await
    .ok()?;
    Some(CurrentUser {
        id: UserId(user.id),
    })
}

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
        .route("/api/fetch-unarchived", get(fetch_unarchived))
        .route("/api/event-feed", get(event_feed))
        .route("/api/submit-event", post(submit_event))
        .route_layer(axum::middleware::from_fn(auth))
        .layer(Extension(db))
        .layer(Extension(feeds));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .context("serving axum webserver")
}

struct AnyhowError(());

impl From<anyhow::Error> for AnyhowError {
    fn from(e: anyhow::Error) -> AnyhowError {
        tracing::error!(err=?e, "got an error");
        AnyhowError(())
    }
}

impl axum::response::IntoResponse for AnyhowError {
    fn into_response(self) -> axum::response::Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal server error, see logs for details",
        )
            .into_response()
    }
}

pub struct PermissionDenied;

impl axum::response::IntoResponse for PermissionDenied {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::FORBIDDEN, "Permission denied").into_response()
    }
}

async fn fetch_unarchived(
    Extension(user): Extension<Auth>,
    Extension(db): Extension<sqlx::PgPool>,
) -> Result<Result<Json<DbDump>, PermissionDenied>, AnyhowError> {
    match user.0 {
        None => Ok(Err(PermissionDenied)),
        Some(user) => {
            let mut conn = db.acquire().await.context("acquiring db connection")?;
            Ok(Ok(Json(
                db::fetch_dump_unarchived(&mut conn, user.id)
                    .await
                    .with_context(|| format!("fetching db dump for {:?}", user))?,
            )))
        }
    }
}

async fn submit_event(
    Json(e): Json<risuto_api::NewEvent>,
    Extension(user): Extension<Auth>,
    Extension(db): Extension<sqlx::PgPool>,
    Extension(feeds): Extension<UserFeeds>,
) -> Result<Result<(), PermissionDenied>, AnyhowError> {
    match user.0 {
        None => Ok(Err(PermissionDenied)),
        Some(user) if user.id != e.owner => Ok(Err(PermissionDenied)),
        Some(user) => {
            let mut conn = db.acquire().await.context("acquiring db connection")?;
            let res = db::submit_event(&mut conn, e.clone())
                .await
                .context("submitting event to db")?;
            if res.is_err() {
                return Ok(res);
            }
            let db = db::PostgresDb {
                conn: &mut conn,
                user: user.id,
            };
            feeds.relay_event(db, e).await;
            Ok(Ok(()))
        }
    }
}

async fn event_feed(
    ws: WebSocketUpgrade,
    Extension(user): Extension<Auth>,
    Extension(feeds): Extension<UserFeeds>,
) -> Result<axum::response::Response, PermissionDenied> {
    if let Some(user) = user.0 {
        Ok(ws.on_upgrade(move |sock| feeds.clone().add_for_user(user.id, sock)))
    } else {
        Err(PermissionDenied)
    }
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
                            for i in ids_to_rm {
                                socks.swap_remove(i);
                            }
                        }
                    }
                }
            })
            .await;
    }
}
