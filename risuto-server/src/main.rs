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
use risuto_api::{DbDump, Event, EventId, EventType, Tag, TagId, Task, TaskId, User, UserId, Uuid};
use sqlx::Row;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    net::SocketAddr,
};

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
        .route("/api/fetch-unarchived", get(fetch_unarchived))
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

#[axum_macros::debug_handler]
async fn fetch_unarchived(
    Extension(user): Extension<Auth>,
    Extension(db): Extension<sqlx::PgPool>,
) -> Result<Result<axum::Json<DbDump>, (StatusCode, &'static str)>, AnyhowError> {
    if !user.0.is_some() {
        return Ok(Err((StatusCode::FORBIDDEN, "Permission denied")));
    }
    let user = user.0.unwrap().id;

    let mut conn = db.acquire().await.context("acquiring db connection")?;

    let users = fetch_users(&mut conn).await?;
    let tags = fetch_tags_for_user(&mut conn, user).await?;

    sqlx::query("CREATE TEMPORARY TABLE tmp_tasks (id UUID NOT NULL)")
        .execute(&mut conn)
        .await
        .context("creating temp table")?;
    sqlx::query(
        "
            INSERT INTO tmp_tasks
            SELECT t.id
                FROM tasks t
            LEFT JOIN v_tasks_archived vta
                ON vta.task_id = t.id
            LEFT JOIN v_tasks_users vtu
                ON vtu.task_id = t.id
            WHERE vtu.user_id = $1
            AND vta.archived = false
        ",
    )
    .bind(user)
    .execute(&mut conn)
    .await
    .context("filling temp table with interesting task ids")?;

    let fetched_tasks = fetch_tasks_from_tmp_tasks_table(&mut conn).await;

    sqlx::query("DROP TABLE tmp_tasks")
        .execute(&mut conn)
        .await
        .context("dropping temp table")?;

    let tasks = fetched_tasks?;

    Ok(Ok(axum::Json(DbDump { users, tags, tasks })))
}

async fn fetch_users(conn: &mut sqlx::PgConnection) -> Result<HashMap<UserId, User>, AnyhowError> {
    Ok(sqlx::query!("SELECT id, name FROM users")
        .fetch(conn)
        .map_ok(|u| (UserId(u.id), User { name: u.name }))
        .try_collect::<HashMap<UserId, User>>()
        .await
        .context("querying users table")?)
}

async fn fetch_tags_for_user(
    conn: &mut sqlx::PgConnection,
    user: Uuid,
) -> Result<HashMap<TagId, Tag>, AnyhowError> {
    Ok(sqlx::query!(
        "
            SELECT tags.id, tags.owner_id, tags.name, tags.archived
            FROM tags
            INNER JOIN perms
            ON perms.tag_id = tags.id
            WHERE perms.user_id = $1
            OR tags.owner_id = $1
        ",
        user
    )
    .fetch(conn)
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
    .context("querying tags table")?)
}

async fn fetch_tasks_from_tmp_tasks_table(
    conn: &mut sqlx::PgConnection,
) -> Result<HashMap<TaskId, Task>, AnyhowError> {
    let mut tasks = HashMap::new();
    let mut tasks_query = sqlx::query(
        "
            SELECT t.id, t.owner_id, t.date, t.initial_title
                FROM tmp_tasks interesting_tasks
            INNER JOIN tasks t
                ON t.id = interesting_tasks.id
        ",
    )
    .fetch(&mut *conn);
    while let Some(t) = tasks_query
        .try_next()
        .await
        .context("querying tasks table")?
    {
        tasks.insert(
            TaskId(t.try_get("id").context("retrieving the id field")?),
            Task {
                owner: UserId(
                    t.try_get("owner_id")
                        .context("retrieving the owner_id field")?,
                ),
                date: t
                    .try_get::<chrono::NaiveDateTime, _>("date")
                    .context("retrieving the date field")?
                    .and_local_timezone(Utc)
                    .unwrap(),

                initial_title: t
                    .try_get("initial_title")
                    .context("retrieving the initial_title field")?,
                current_title: String::new(),

                is_done: false,
                is_archived: false,
                scheduled_for: None,
                current_tags: HashMap::new(),

                deps_before_self: HashSet::new(),
                deps_after_self: HashSet::new(),

                current_comments: BTreeMap::new(),

                events: BTreeMap::new(),
            },
        );
    }
    std::mem::drop(tasks_query); // free conn borrow

    macro_rules! query_events {
        (full: $query:expr, $table:expr, $task_id:expr, |$e:ident| $c:expr,) => {{
            let mut query = sqlx::query($query).fetch(&mut *conn);
            while let Some($e) =
                query
                    .try_next()
                    .await
                    .context(concat!("querying ", $table, " table"))?
            {
                let task_id = $e.try_get($task_id).context("retrieving task_id field")?;
                if let Some(t) = tasks.get_mut(&TaskId(task_id)) {
                    let date: chrono::NaiveDateTime =
                        $e.try_get("date").context("retrieving date field")?;
                    let date = date.and_local_timezone(Utc).unwrap();
                    let id = $e.try_get("id").context("retrieving id field")?;
                    let owner = $e
                        .try_get("owner_id")
                        .context("retrieving owner_id field")?;
                    t.events.entry(date).or_insert(Vec::new()).push(Event {
                        id: EventId(id),
                        owner: UserId(owner),
                        date,
                        contents: $c,
                    });
                }
            }
        }};

        (fields: $additional_fields:expr, $table:expr, $task_id:expr, |$e:ident| $c:expr,) => {
            query_events!(
                full: concat!(
                    "SELECT e.id, e.owner_id, e.date, ",
                    $additional_fields,
                    " FROM tmp_tasks t
                    INNER JOIN ",
                    $table,
                    " e ON t.id = e.",
                    $task_id
                ),
                $table,
                $task_id,
                |$e| $c,
            )
        };
    }

    query_events!(
        fields: "e.task_id, e.title",
        "set_title_events",
        "task_id",
        |e| EventType::SetTitle(e.try_get("title").context("retrieving title field")?),
    );

    query_events!(
        fields: "e.task_id",
        "complete_task_events",
        "task_id",
        |e| EventType::Complete,
    );

    query_events!(
        fields: "e.task_id",
        "reopen_task_events",
        "task_id",
        |e| EventType::Reopen,
    );

    query_events!(
        fields: "e.task_id",
        "archive_task_events",
        "task_id",
        |e| EventType::Archive,
    );

    query_events!(
        fields: "e.task_id",
        "unarchive_task_events",
        "task_id",
        |e| EventType::Unarchive,
    );

    query_events!(
        fields: "e.task_id, e.scheduled_date",
        "schedule_events",
        "task_id",
        |e| EventType::Schedule(
            e.try_get::<Option<chrono::NaiveDateTime>, _>("scheduled_date")
                .context("retrieving scheduled_date field")?
                .map(|d| d.and_local_timezone(Utc).unwrap())
        ),
    );

    query_events!(
        fields: "e.first_id, e.then_id",
        "add_dependency_events",
        "first_id",
        |e| EventType::AddDepAfterSelf(TaskId(
            e.try_get("then_id").context("retrieving then_id field")?
        )),
    );

    query_events!(
        fields: "e.first_id, e.then_id",
        "add_dependency_events",
        "then_id",
        |e| EventType::AddDepBeforeSelf(TaskId(
            e.try_get("first_id").context("retrieving first_id field")?
        )),
    );

    query_events!(
        fields: "e.first_id, e.then_id",
        "remove_dependency_events",
        "first_id",
        |e| EventType::RmDepAfterSelf(TaskId(
            e.try_get("then_id").context("retrieving then_id field")?
        )),
    );

    query_events!(
        fields: "e.first_id, e.then_id",
        "remove_dependency_events",
        "then_id",
        |e| EventType::RmDepBeforeSelf(TaskId(
            e.try_get("first_id").context("retrieving first_id field")?
        )),
    );

    query_events!(
        fields: "e.task_id, e.tag_id, e.priority",
        "add_tag_events",
        "task_id",
        |e| EventType::AddTag {
            tag: TagId(e.try_get("tag_id").context("retrieving tag_id field")?),
            prio: e.try_get("priority").context("retrieving prio field")?
        },
    );

    query_events!(
        fields: "e.task_id, e.tag_id",
        "remove_tag_events",
        "task_id",
        |e| EventType::RmTag(TagId(
            e.try_get("tag_id").context("retrieving tag_id field")?
        )),
    );

    query_events!(
        fields: "e.task_id, e.text",
        "add_comment_events",
        "task_id",
        |e| EventType::AddComment(e.try_get("text").context("retrieving text field")?),
    );

    query_events!(
        full: "
            SELECT e.id, e.owner_id, e.date, e.comment_id, e.text, ace.task_id
                FROM tmp_tasks t
            INNER JOIN add_comment_events ace
                ON t.id = ace.task_id
            INNER JOIN edit_comment_events e
                ON ace.id = e.comment_id
        ",
        "edit_comment_events",
        "task_id",
        |e| EventType::EditComment(
            EventId(
                e.try_get("comment_id")
                    .context("retrieving comment_id field")?
            ),
            e.try_get("text").context("retrieving text field")?
        ),
    );

    for t in tasks.values_mut() {
        t.refresh_metadata();
    }

    Ok(tasks)
}
