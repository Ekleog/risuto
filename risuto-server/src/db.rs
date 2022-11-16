use anyhow::Context;
use chrono::Utc;
use futures::TryStreamExt;
use risuto_api::{
    AuthCheck, AuthInfo, DbDump, Event, EventId, EventType, NewEvent, Tag, TagId, Task, TaskId,
    User, UserId,
};
use sqlx::Row;
use std::collections::{BTreeMap, HashMap, HashSet};

use crate::PermissionDenied;

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

pub async fn auth_info_for(
    conn: &mut sqlx::PgConnection,
    user: UserId,
    task: TaskId,
) -> anyhow::Result<AuthInfo> {
    let auth = sqlx::query!(
        r#"
            SELECT
                can_edit AS "can_edit!",
                can_triage AS "can_triage!",
                can_relabel_to_any AS "can_relabel_to_any!",
                can_comment AS "can_comment!"
            FROM v_tasks_users
            WHERE task_id = $1
            AND user_id = $2
        "#,
        task.0,
        user.0
    )
    .fetch_all(&mut *conn)
    .await
    .with_context(|| {
        format!(
            "checking permissions for user {:?} on task {:?}",
            user, task
        )
    })?;
    match &auth[..] {
        [] => Ok(AuthInfo {
            can_read: false,
            can_edit: false,
            can_triage: false,
            can_relabel_to_any: false,
            can_comment: false,
        }),
        [r] => Ok(AuthInfo {
            can_read: true,
            can_edit: r.can_edit,
            can_triage: r.can_triage,
            can_relabel_to_any: r.can_relabel_to_any,
            can_comment: r.can_comment,
        }),
        _ => Err(anyhow::anyhow!(
            "v_tasks_users had multiple lines for task {:?} and user {:?}",
            task,
            user
        )),
    }
}

pub async fn list_tags_on(
    conn: &mut sqlx::PgConnection,
    task: TaskId,
) -> anyhow::Result<Vec<TagId>> {
    Ok(sqlx::query!(
        r#"SELECT tag_id AS "tag_id!" FROM v_tasks_tags WHERE task_id = $1"#,
        task.0
    )
    .map(|r| TagId(r.tag_id))
    .fetch_all(conn)
    .await?)
}

pub async fn get_comment_owner(
    conn: &mut sqlx::PgConnection,
    event: EventId,
) -> anyhow::Result<UserId> {
    Ok(UserId(
        sqlx::query!(
            "SELECT owner_id FROM add_comment_events WHERE id = $1",
            event.0
        )
        .fetch_one(conn)
        .await?
        .owner_id,
    ))
}

pub async fn is_comment_first(
    conn: &mut sqlx::PgConnection,
    task: TaskId,
    event: EventId,
) -> anyhow::Result<bool> {
    Ok(sqlx::query!(
        "SELECT id FROM add_comment_events WHERE task_id = $1 ORDER BY date LIMIT 1",
        task.0
    )
    .fetch_one(conn)
    .await?
    .id == event.0)
}

pub async fn submit_event(
    conn: &mut sqlx::PgConnection,
    e: NewEvent,
) -> anyhow::Result<Result<(), PermissionDenied>> {
    // Check authorization
    let mut auth = e.is_authorized(
        &auth_info_for(&mut *conn, e.event.owner, e.task)
            .await
            .with_context(|| format!("fetching auth info for {:?} {:?}", e.event.owner, e.task))?,
    );
    loop {
        match auth.clone() {
            AuthCheck::Done(true) => break,
            AuthCheck::Done(false) => return Ok(Err(PermissionDenied)),
            AuthCheck::IfCanTriage(task) => auth.feed_auth_info_for(
                task,
                &auth_info_for(&mut *conn, e.event.owner, task)
                    .await
                    .with_context(|| {
                        format!("fetching auth info for {:?} {:?}", e.event.owner, e.task)
                    })?,
            ),
            AuthCheck::IfTagInTagsFor(tag, task) => auth.feed_tag_in_tags_for(
                tag,
                task,
                list_tags_on(&mut *conn, task)
                    .await
                    .with_context(|| format!("fetching tags on {:?}", task))?
                    .contains(&tag),
            ),
            AuthCheck::IfIsCommentOwner(comm) => auth.feed_is_comment_owner(
                comm,
                get_comment_owner(&mut *conn, comm)
                    .await
                    .with_context(|| format!("getting comment owner of {:?}", comm))?
                    == e.event.owner,
            ),
            AuthCheck::IfIsCommentFirstOr(task, comm, _) => auth.feed_is_comment_first(
                task,
                comm,
                is_comment_first(&mut *conn, task, comm)
                    .await
                    .with_context(|| {
                        format!(
                            "checking if comment {:?} is the first of task {:?}",
                            comm, task
                        )
                    })?,
            ),
        }
    }

    let event_id = e.event.id;

    macro_rules! insert_event {
        ($table:expr, $repeat:expr, $( $v:expr ),*) => {{
            sqlx::query(
                concat!(
                    "INSERT INTO ",
                    $table,
                    " VALUES ($1, $2, $3, ",
                    $repeat,
                    ")",
                )
            )
            $(.bind($v))*
            .execute(&mut *conn)
            .await
            .with_context(|| format!("inserting {} {:?}", $table, event_id))?
        }}
    }

    let res = match e.event.contents {
        EventType::SetTitle(t) => insert_event!("set_title_events", "$4, $5", e.task.0, t),
        EventType::SetDone(d) => insert_event!("set_task_done_events", "$4, $5", e.task.0, d),
        EventType::SetArchived(a) => {
            insert_event!("set_task_archived_events", "$4, $5", e.task.0, a)
        }
        EventType::Schedule(d) => insert_event!(
            "schedule_events",
            "$4, $5",
            e.task.0,
            d.map(|d| d.naive_utc())
        ),
        EventType::AddDepBeforeSelf(o) => {
            insert_event!("add_dependency_events", "$4, $5", o.0, e.task.0)
        }
        EventType::AddDepAfterSelf(o) => {
            insert_event!("add_dependency_events", "$4, $5", e.task.0, o.0)
        }
        EventType::RmDepBeforeSelf(o) => {
            insert_event!("remove_dependency_events", "$4, $5", o.0, e.task.0)
        }
        EventType::RmDepAfterSelf(o) => {
            insert_event!("remove_dependency_events", "$4, $5", e.task.0, o.0)
        }
        EventType::AddTag { tag, prio, backlog } => insert_event!(
            "add_tag_events",
            "$4, $5, $6, $7",
            e.task.0,
            tag.0,
            prio,
            backlog
        ),
        EventType::RmTag(tag) => insert_event!("remove_tag_events", "$4, $5", e.task.0, tag.0),
        EventType::AddComment(txt) => insert_event!("add_comment_events", "$4, $5", e.task.0, txt),
        EventType::EditComment(comm, txt) => {
            insert_event!("edit_comment_events", "$4, $5", comm.0, txt)
        }
    };

    anyhow::ensure!(
        res.rows_affected() == 1,
        "insertion of event {:?} affected {} rows",
        event_id,
        res.rows_affected()
    );
    // TODO: give a specific error if the event id is already taken

    Ok(Ok(()))
}
