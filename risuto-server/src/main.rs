use anyhow::Context;
use axum::{
    http::{self, Request, StatusCode},
    middleware::Next,
    response::Response,
    routing::get,
    Extension, Router,
};
use chrono::Utc;
use futures::TryStreamExt;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    net::SocketAddr,
};
use uuid::Uuid;

#[derive(Clone, Debug)]
struct Auth(Option<CurrentUser>);

#[derive(Clone, Debug)]
struct CurrentUser {
    id: Uuid,
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

    let user = sqlx::query_as!(
        CurrentUser,
        "SELECT id FROM users WHERE name = $1 AND password = $2",
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

    let db_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    let db = sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect(&db_url)
        .await
        .with_context(|| format!("Error opening database {:?}", db_url))?;

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

#[derive(Clone, Copy, Eq, Hash, PartialEq, serde::Serialize)]
struct UserId(Uuid);

#[derive(serde::Serialize)]
struct User {
    name: String,
}

#[derive(Clone, Copy, Eq, Hash, PartialEq, serde::Serialize)]
struct TagId(Uuid);

#[derive(serde::Serialize)]
struct Tag {
    owner: UserId,
    name: String,
    archived: bool,
}

#[derive(Clone, Copy, Eq, Hash, PartialEq, serde::Serialize)]
struct TaskId(Uuid);

#[derive(serde::Serialize)]
struct Task {
    owner: UserId,
    date: Time,

    initial_title: String,
    current_title: String,

    is_done: bool,
    is_archived: bool,
    scheduled_for: Option<Time>,
    current_tags: HashSet<(TagId, usize)>,

    deps_before_self: HashSet<TaskId>,
    deps_after_self: HashSet<TaskId>,

    /// List of comments in chronological order, with for each comment each edit in chronological order
    current_comments: BTreeMap<Time, BTreeMap<Time, String>>,

    events: BTreeMap<Time, Event>,
}

#[derive(Clone, Copy, Eq, PartialEq, serde::Serialize)]
struct EventId(Uuid);

#[derive(serde::Serialize)]
struct Event {
    id: EventId,
    owner: UserId,
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
    Schedule(Option<Time>),
    AddDepBeforeSelf(TaskId),
    AddDepAfterSelf(TaskId),
    RmDep(EventId),
    AddTag { tag: TagId, prio: usize },
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
    Extension(db): Extension<sqlx::PgPool>,
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
            WHERE perms.user_id = $1
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
    let mut tasks_query = sqlx::query!(
        "
            SELECT t.id, t.owner_id, t.date, t.initial_title
                FROM tasks t
            LEFT JOIN v_tasks_archived vta
                ON vta.task_id = t.id
            LEFT JOIN v_tasks_users vtu
                ON vtu.task_id = t.id
            WHERE vtu.user_id = $1
            AND vta.archived = false
        ",
        user
    )
    .fetch(&db);
    while let Some(t) = tasks_query
        .try_next()
        .await
        .context("querying tasks table")?
    {
        tasks.insert(
            TaskId(t.id),
            Task {
                owner: UserId(t.owner_id),
                date: t.date.and_local_timezone(Utc).unwrap(),

                initial_title: t.initial_title,
                current_title: String::new(),

                is_done: false,
                is_archived: false,
                scheduled_for: None,
                current_tags: HashSet::new(),

                deps_before_self: HashSet::new(),
                deps_after_self: HashSet::new(),

                current_comments: BTreeMap::new(),

                events: BTreeMap::new(),
            },
        );
    }

    macro_rules! query_events {
        ($query:expr, $table:expr, $task_id:ident, |$e:ident| $c:expr,) => {{
            let mut query = sqlx::query!($query, user).fetch(&db);
            while let Some($e) =
                query
                    .try_next()
                    .await
                    .context(concat!("querying ", $table, " table"))?
            {
                if let Some(t) = tasks.get_mut(&TaskId($e.$task_id)) {
                    let date = $e.date.and_local_timezone(Utc).unwrap();
                    t.events.insert(
                        date,
                        Event {
                            id: EventId($e.id),
                            owner: UserId($e.owner_id),
                            date,
                            contents: $c,
                        },
                    );
                }
            }
        }};
    }

    query_events!(
        "
            SELECT e.id, e.owner_id, e.date, e.task_id, e.title
                FROM set_title_events e
            LEFT JOIN v_tasks_archived vta
                ON vta.task_id = e.task_id
            LEFT JOIN v_tasks_users vtu
                ON vtu.task_id = e.task_id
            WHERE vtu.user_id = $1
            AND vta.archived = false
        ",
        "set_title_events",
        task_id,
        |e| EventType::SetTitle(e.title),
    );

    query_events!(
        "
            SELECT e.id, e.owner_id, e.date, e.task_id
                FROM complete_task_events e
            LEFT JOIN v_tasks_archived vta
                ON vta.task_id = e.task_id
            LEFT JOIN v_tasks_users vtu
                ON vtu.task_id = e.task_id
            WHERE vtu.user_id = $1
            AND vta.archived = false
        ",
        "complete_task_events",
        task_id,
        |e| EventType::Complete,
    );

    query_events!(
        "
            SELECT e.id, e.owner_id, e.date, e.task_id
                FROM reopen_task_events e
            LEFT JOIN v_tasks_archived vta
                ON vta.task_id = e.task_id
            LEFT JOIN v_tasks_users vtu
                ON vtu.task_id = e.task_id
            WHERE vtu.user_id = $1
            AND vta.archived = false
        ",
        "reopen_task_events",
        task_id,
        |e| EventType::Reopen,
    );

    query_events!(
        "
            SELECT e.id, e.owner_id, e.date, e.task_id
                FROM archive_task_events e
            LEFT JOIN v_tasks_archived vta
                ON vta.task_id = e.task_id
            LEFT JOIN v_tasks_users vtu
                ON vtu.task_id = e.task_id
            WHERE vtu.user_id = $1
            AND vta.archived = false
        ",
        "archive_task_events",
        task_id,
        |e| EventType::Archive,
    );

    query_events!(
        "
            SELECT e.id, e.owner_id, e.date, e.task_id
                FROM unarchive_task_events e
            LEFT JOIN v_tasks_archived vta
                ON vta.task_id = e.task_id
            LEFT JOIN v_tasks_users vtu
                ON vtu.task_id = e.task_id
            WHERE vtu.user_id = $1
            AND vta.archived = false
        ",
        "unarchive_task_events",
        task_id,
        |e| EventType::Unarchive,
    );

    query_events!(
        "
            SELECT e.id, e.owner_id, e.date, e.task_id, e.scheduled_date
                FROM schedule_events e
            LEFT JOIN v_tasks_archived vta
                ON vta.task_id = e.task_id
            LEFT JOIN v_tasks_users vtu
                ON vtu.task_id = e.task_id
            WHERE vtu.user_id = $1
            AND vta.archived = false
        ",
        "schedule_events",
        task_id,
        |e| EventType::Schedule(e.scheduled_date.map(|d| d.and_local_timezone(Utc).unwrap())),
    );

    query_events!(
        "
            SELECT e.id, e.owner_id, e.date, e.first_id, e.then_id
                FROM add_dependency_events e
            LEFT JOIN v_tasks_archived vta
                ON vta.task_id = e.first_id
            LEFT JOIN v_tasks_users vtu
                ON vtu.task_id = e.first_id
            WHERE vtu.user_id = $1
            AND vta.archived = false
        ",
        "add_dependency_events",
        first_id,
        |e| EventType::AddDepAfterSelf(TaskId(e.then_id)),
    );

    query_events!(
        "
            SELECT e.id, e.owner_id, e.date, e.first_id, e.then_id
                FROM add_dependency_events e
            LEFT JOIN v_tasks_archived vta
                ON vta.task_id = e.then_id
            LEFT JOIN v_tasks_users vtu
                ON vtu.task_id = e.then_id
            WHERE vtu.user_id = $1
            AND vta.archived = false
        ",
        "add_dependency_events",
        then_id,
        |e| EventType::AddDepBeforeSelf(TaskId(e.first_id)),
    );

    query_events!(
        "
            SELECT e.id, e.owner_id, e.date, e.dep_id, ade.first_id
                FROM remove_dependency_events e
            LEFT JOIN add_dependency_events ade
                ON ade.id = e.dep_id
            LEFT JOIN v_tasks_archived vta
                ON vta.task_id = ade.first_id
            LEFT JOIN v_tasks_users vtu
                ON vtu.task_id = ade.first_id
            WHERE vtu.user_id = $1
            AND vta.archived = false
        ",
        "remove_dependency_events",
        first_id,
        |e| EventType::RmDep(EventId(e.dep_id)),
    );

    query_events!(
        "
            SELECT e.id, e.owner_id, e.date, e.dep_id, ade.then_id
                FROM remove_dependency_events e
            LEFT JOIN add_dependency_events ade
                ON ade.id = e.dep_id
            LEFT JOIN v_tasks_archived vta
                ON vta.task_id = ade.then_id
            LEFT JOIN v_tasks_users vtu
                ON vtu.task_id = ade.then_id
            WHERE vtu.user_id = $1
            AND vta.archived = false
        ",
        "remove_dependency_events",
        then_id,
        |e| EventType::RmDep(EventId(e.dep_id)),
    );

    for t in tasks.values_mut() {
        for e in t.events.values() {
            match &e.contents {
                EventType::SetTitle(title) => t.current_title = title.clone(),
                EventType::Complete => t.is_done = true,
                EventType::Reopen => t.is_done = false,
                EventType::Archive => t.is_archived = true,
                EventType::Unarchive => t.is_archived = false,
                EventType::Schedule(time) => t.scheduled_for = *time,
                EventType::AddDepBeforeSelf(task) => {
                    t.deps_before_self.insert(*task);
                }
                EventType::AddDepAfterSelf(task) => {
                    t.deps_after_self.insert(*task);
                }
                EventType::RmDep(evt) => {
                    if let Some(evt) = t.events.values().find(|e| &e.id == evt) {
                        // ignore if no matching event
                        match evt.contents {
                            EventType::AddDepBeforeSelf(task) => {
                                t.deps_before_self.remove(&task);
                            }
                            EventType::AddDepAfterSelf(task) => {
                                t.deps_after_self.remove(&task);
                            }
                            _ => panic!("RmDep refering to wrong event"),
                        }
                    }
                }
                EventType::AddTag { tag, prio } => {
                    t.current_tags.insert((*tag, *prio));
                }
                EventType::RmTag(evt) => {
                    if let Some(evt) = t.events.values().find(|e| &e.id == evt) {
                        // ignore if no matching event
                        match evt.contents {
                            EventType::AddTag { tag, prio } => {
                                t.current_tags.remove(&(tag, prio));
                            }
                            _ => panic!("RmTag refering to wrong event"),
                        }
                    }
                }
                EventType::AddComment(txt) => {
                    let mut edits = BTreeMap::new();
                    edits.insert(e.date, txt.clone());
                    t.current_comments.insert(e.date, edits);
                }
                EventType::EditComment(evt, txt) => {
                    if let Some(evt) = t.events.values().find(|e| &e.id == evt) {
                        // ignore if no matching event
                        match evt.contents {
                            EventType::AddComment(_) => {
                                t.current_comments
                                    .get_mut(&evt.date)
                                    .unwrap()
                                    .insert(e.date, txt.clone());
                            }
                            _ => panic!("RmTag refering to wrong event"),
                        }
                    }
                }
            }
        }
    }

    Ok(Ok(axum::Json(DbDump { users, tags, tasks })))
}
