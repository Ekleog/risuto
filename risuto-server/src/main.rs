use anyhow::Context;
use axum::{
    http::{self, Request, StatusCode},
    middleware::Next,
    response::Response,
    routing::get,
    Extension, Router,
};
use chrono::Utc;
use futures::{StreamExt, TryStreamExt};
use std::{collections::HashMap, net::SocketAddr};

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
    owner: UserId,
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
async fn fetch_unarchived(
    Extension(user): Extension<Auth>,
    Extension(db): Extension<sqlx::SqlitePool>,
) -> Result<Result<axum::Json<DbDump>, (StatusCode, &'static str)>, AnyhowError> {
    if !user.0.is_some() {
        return Ok(Err((StatusCode::FORBIDDEN, "Permission denied")));
    }
    let user = user.0.unwrap().id;

    let users = sqlx::query!("SELECT id, name FROM users")
        .fetch(&db)
        .map_ok(|u| (UserId(u.id), User { name: u.name }))
        .try_collect::<HashMap<UserId, User>>()
        .await
        .context("querying users table")?;

    let tags = sqlx::query!(
        "
            SELECT tags.id, tags.owner_id, tags.name, tags.archived
            FROM tags
            INNER JOIN perms
            ON perms.tag_id = tags.id
            WHERE perms.user_id = ?
        ",
        user
    )
    .fetch(&db)
    .map_ok(|t| {
        (
            TagId(t.id),
            Tag {
                owner: UserId(t.owner_id),
                name: t.name,
                archived: t.archived,
            },
        )
    })
    .try_collect::<HashMap<TagId, Tag>>()
    .await
    .context("querying tags table")?;

    let mut tasks = HashMap::new();
    while let Some(t) = sqlx::query!(
        "
            SELECT t.id, t.owner_id, t.date, t.initial_title
            FROM tasks t
            WHERE (
                owner_id = ?
                OR EXISTS (
                    SELECT NULL
                        FROM add_tag_events ate
                    INNER JOIN perms p
                        ON p.tag_id = ate.tag_id
                    LEFT JOIN remove_tag_events rte
                        ON rte.add_tag_id = ate.id
                    WHERE ate.task_id = t.id
                        AND rte.id IS NULL
                        AND p.user_id = ?
                )
            )
            AND (
                (
                    SELECT MAX(ate.date)
                    FROM archive_task_events ate
                    WHERE ate.task_id = t.id
                ) < (
                    SELECT MAX(ute.date)
                    FROM unarchive_task_events ute
                    WHERE ute.task_id = t.id
                )
            )
        ",
        user, user
    )
        .fetch(&db)
        .try_next()
        .await
        .context("querying tasks table")?
    {
        todo!();
    }

    Ok(Ok(axum::Json(DbDump { users, tags, tasks })))
}
