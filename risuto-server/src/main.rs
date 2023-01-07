use anyhow::Context;
use axum::{
    async_trait,
    extract::{
        ws::{Message, WebSocket},
        FromRef, FromRequestParts, State, WebSocketUpgrade,
    },
    http::{request, StatusCode},
    routing::{get, post},
    Json, Router,
};
use futures::{channel::mpsc, select, SinkExt, StreamExt};
use risuto_api::{
    AuthInfo, AuthToken, Event, FeedMessage, NewSession, Tag, Task, User, UserId, Uuid,
};
use serde_json::json;
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::sync::RwLock;
use tower_http::trace::TraceLayer;

mod db;
mod query;

#[derive(Clone, FromRef)]
struct AppState {
    db: sqlx::PgPool,
    feeds: UserFeeds,
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

    let state = AppState { db, feeds };

    let app = Router::new()
        .route("/api/auth", post(auth))
        .route("/api/unauth", post(unauth))
        .route("/api/whoami", get(whoami))
        .route("/api/fetch-users", get(fetch_users))
        .route("/api/fetch-tags", get(fetch_tags))
        .route("/api/search-tasks", post(search_tasks))
        .route("/ws/event-feed", get(event_feed))
        .route("/api/submit-event", post(submit_event))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .context("serving axum webserver")
}

struct PreAuth(AuthToken);

#[async_trait]
impl FromRequestParts<AppState> for PreAuth {
    type Rejection = Error;

    async fn from_request_parts(
        req: &mut request::Parts,
        _state: &AppState,
    ) -> Result<PreAuth, Error> {
        match req.headers.get(axum::http::header::AUTHORIZATION) {
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

pub enum Error {
    Anyhow(anyhow::Error),
    PermissionDenied,
    UuidAlreadyUsed(Uuid),
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
            Error::UuidAlreadyUsed(id) => (
                StatusCode::CONFLICT,
                Json(json!({ "error": "uuid already used", "uuid": id })),
            )
                .into_response(),
        }
    }
}

async fn auth(
    State(db): State<sqlx::PgPool>,
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

async fn submit_event(
    Auth(user): Auth,
    State(db): State<sqlx::PgPool>,
    State(feeds): State<UserFeeds>,
    Json(e): Json<risuto_api::Event>,
) -> Result<(), Error> {
    if user != e.owner_id {
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
    State(db): State<sqlx::PgPool>,
    State(feeds): State<UserFeeds>,
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
                            feeds.add_for_user(user, sock).await;
                            return;
                        }
                    }
                }
            }
            tracing::debug!(?token, "event feed websocket auth failure");
            let _ = sock
                .send(Message::Text(String::from("permission denied")))
                .await;
        }
    }))
}

#[derive(Clone, Debug)]
struct UserFeeds(Arc<RwLock<HashMap<UserId, HashMap<Uuid, mpsc::UnboundedSender<FeedMessage>>>>>);

impl UserFeeds {
    async fn add_for_user(self, user: UserId, sock: WebSocket) {
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
        let mut sock = sock.fuse();
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
                    if let Err(_) = sock.send(Message::Binary(json)).await {
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
                    msg = sock.next() => match msg {
                        None => remove_self!(),
                        Some(Ok(Message::Text(msg))) => {
                            if msg != "ping" {
                                tracing::warn!("received unexpected message from client: {msg:?}");
                                remove_self!();
                            }
                            send_message!(FeedMessage::Pong);
                        }
                        Some(msg) => {
                            tracing::warn!("received unexpected message from client: {msg:?}");
                            remove_self!();
                        }
                    },
                }
            }
        });
    }

    async fn relay_event(&self, mut db: db::PostgresDb<'_>, e: risuto_api::Event) {
        // TODO: magic numbers below should be at least explained
        db::users_interested_by(&mut db.conn, &[e.task_id.0])
            .for_each_concurrent(Some(16), |u| {
                let e = e.clone();
                async move {
                    match u {
                        Err(err) => {
                            tracing::error!(?err, "error occurred while listing interested users");
                        }
                        Ok(u) => {
                            if let Some(socks) = self.0.read().await.get(&u) {
                                for s in socks.values() {
                                    let _ = s.unbounded_send(FeedMessage::NewEvent(e.clone()));
                                }
                            }
                        }
                    }
                }
            })
            .await;
    }
}
