use anyhow::Context;
use chrono::Utc;
use futures::TryStreamExt;
use risuto_api::{DbDump, Event, EventId, EventType, Tag, TagId, Task, TaskId, User, UserId};
use sqlx::Row;
use std::collections::{BTreeMap, HashMap, HashSet};

pub async fn fetch_dump_unarchived(
    conn: &mut sqlx::PgConnection,
    owner: UserId,
) -> anyhow::Result<DbDump> {
    let users = fetch_users(&mut *conn).await.context("fetching users")?;
    let tags = fetch_tags_for_user(&mut *conn, owner)
        .await
        .with_context(|| format!("fetching tags for user {:?}", owner))?;

    sqlx::query("CREATE TEMPORARY TABLE tmp_tasks (id UUID NOT NULL)")
        .execute(&mut *conn)
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
    .bind(owner.0)
    .execute(&mut *conn)
    .await
    .context("filling temp table with interesting task ids")?;

    let fetched_tasks = fetch_tasks_from_tmp_tasks_table(&mut *conn).await;

    sqlx::query("DROP TABLE tmp_tasks")
        .execute(conn)
        .await
        .context("dropping temp table")?;

    let tasks = fetched_tasks?;

    Ok(DbDump {
        owner,
        users,
        tags,
        tasks,
    })
}

async fn fetch_users(conn: &mut sqlx::PgConnection) -> anyhow::Result<HashMap<UserId, User>> {
    Ok(sqlx::query!("SELECT id, name FROM users")
        .fetch(conn)
        .map_ok(|u| (UserId(u.id), User { name: u.name }))
        .try_collect::<HashMap<UserId, User>>()
        .await
        .context("querying users table")?)
}

async fn fetch_tags_for_user(
    conn: &mut sqlx::PgConnection,
    user: UserId,
) -> anyhow::Result<HashMap<TagId, Tag>> {
    // TODO: also report which permissions are available to the user
    Ok(sqlx::query!(
        "
            SELECT
                tags.id,
                tags.owner_id,
                tags.name,
                tags.archived,
                users.name AS owner_name
            FROM tags
            INNER JOIN users
                ON users.id = tags.owner_id
            LEFT JOIN perms
                ON perms.tag_id = tags.id
            WHERE perms.user_id = $1
                OR tags.owner_id = $1
        ",
        user.0
    )
    .fetch(conn)
    .map_ok(|t| {
        (
            TagId(t.id),
            Tag {
                owner: UserId(t.owner_id),
                name: if t.owner_id == user.0 {
                    t.name
                } else {
                    format!("{}:{}", t.owner_name, t.name)
                },
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
) -> anyhow::Result<HashMap<TaskId, Task>> {
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
                    t.add_event(Event {
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
        fields: "e.task_id, e.now_done",
        "set_task_done_events",
        "task_id",
        |e| EventType::SetDone(e.try_get("now_done").context("retrieving now_done field")?),
    );

    query_events!(
        fields: "e.task_id, e.now_archived",
        "set_task_archived_events",
        "task_id",
        |e| EventType::SetArchived(e.try_get("now_archived").context("retrieving now_archived field")?),
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
        fields: "e.task_id, e.tag_id, e.priority, e.backlog",
        "add_tag_events",
        "task_id",
        |e| EventType::AddTag {
            tag: TagId(e.try_get("tag_id").context("retrieving tag_id field")?),
            prio: e.try_get("priority").context("retrieving prio field")?,
            backlog: e.try_get("backlog").context("retrieving backlog field")?,
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
