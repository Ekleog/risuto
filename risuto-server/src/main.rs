use anyhow::Context;
use axum::{
    http::{self, Request, StatusCode},
    middleware::Next,
    response::Response,
    routing::get,
    Extension, Router,
};
use chrono::Utc;
use std::{net::SocketAddr, collections::HashMap};

#[derive(Clone, Debug)]
struct Auth(Option<CurrentUser>);

#[derive(Clone, Debug)]
struct CurrentUser {
    id: String,
}

async fn auth<B: std::fmt::Debug>(
    mut req: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    if let Some(auth) = req.headers().get(http::header::AUTHORIZATION) {
        let auth = auth.to_str().map_err(|_| StatusCode::UNAUTHORIZED)?;

        let db = req
            .extensions()
            .get::<sqlx::SqlitePool>()
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

async fn authorize_current_user(db: &sqlx::SqlitePool, auth: &str) -> Option<CurrentUser> {
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

    let user = sqlx::query_as!(
        CurrentUser,
        "SELECT id FROM users WHERE name = ? AND password = ?",
        split[0],
        split[1]
    )
    .fetch_one(db)
    .await
    .ok()?;
    Some(user)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let db_file = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    let db = sqlx::sqlite::SqlitePoolOptions::new()
        .connect(&db_file)
        .await
        .with_context(|| format!("Error opening database {:?}", db_file))?;

    let app = Router::new()
        .route("/fetch-unarchived", get(fetch_unarchived))
        .route_layer(axum::middleware::from_fn(auth))
        .layer(Extension(db));

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
        tracing::error!(err=%e, "got an error");
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

type Time = chrono::DateTime<Utc>;

#[derive(Eq, Hash, PartialEq, serde::Serialize)]
struct UserId(String);

#[derive(serde::Serialize)]
struct User {
    name: String,
}

#[derive(Eq, Hash, PartialEq, serde::Serialize)]
struct TagId(String);

#[derive(serde::Serialize)]
struct Tag {
    owner: User,
    name: String,
    archived: bool,
}

#[derive(Eq, Hash, PartialEq, serde::Serialize)]
struct TaskId(String);

#[derive(serde::Serialize)]
struct Task {
    owner: UserId,
    date: Time,

    initial_title: String,
    current_title: String,

    scheduled_for: Option<Time>,

    current_tags: Vec<(TagId, usize)>,

    /// List of comments in chronological order, with for each comment each edit in chronological order
    current_comments: Vec<Vec<String>>,

    events: Vec<Event>,
}

#[derive(serde::Serialize)]
struct EventId(String);

#[derive(serde::Serialize)]
struct Event {
    owner: User,
    date: Time,

    contents: EventType,
}

#[derive(serde::Serialize)]
enum EventType {
    SetTitle(String),
    Complete,
    Reopen,
    Archive,
    Unarchive,
    Schedule(Time),
    AddDepBeforeSelf(TaskId),
    AddDepAfterSelf(TaskId),
    RmDep(EventId),
    AddTag(TagId, usize),
    RmTag(EventId),
    AddComment(String),
    EditComment(EventId, String),
}

#[derive(serde::Serialize)]
struct DbDump {
    users: HashMap<UserId, User>,
    tags: HashMap<TagId, Tag>,
    tasks: HashMap<TaskId, Task>,
}

#[axum_macros::debug_handler]
async fn fetch_unarchived(Extension(user): Extension<Auth>, Extension(db): Extension<sqlx::SqlitePool>) -> Result<axum::Json<DbDump>, AnyhowError> {
    let users = sqlx::query!("SELECT id, name FROM users")
        .fetch_all(&db)
        .await
        .context("querying users table");
    let tags = sqlx::query!("SELECT id, owner_id, name, archived FROM tags")
        .fetch_all(&db)
        .await
        .context("querying tags table");
    Ok(axum::Json(todo!()))
}
