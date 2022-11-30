use anyhow::{anyhow, Context};
use axum::async_trait;
use chrono::Utc;
use futures::{Stream, StreamExt, TryStreamExt};
use risuto_api::{
    AuthInfo, AuthToken, DbDump, Event, EventId, EventType, NewEvent, NewEventContents, NewSession,
    Tag, TagId, Task, TaskId, User, UserId, Uuid,
};
use sqlx::Row;
use std::collections::{BTreeMap, HashMap, HashSet};

use crate::Error;

pub struct PostgresDb<'a> {
    pub conn: &'a mut sqlx::PgConnection,
    pub user: UserId,
}

#[async_trait]
impl<'a> risuto_api::Db for PostgresDb<'a> {
    async fn auth_info_for(&mut self, task: TaskId) -> anyhow::Result<AuthInfo> {
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
            self.user.0
        )
        .fetch_all(&mut *self.conn)
        .await
        .with_context(|| {
            format!(
                "checking permissions for user {:?} on task {:?}",
                self.user, task
            )
        })?;
        let auth = match &auth[..] {
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
                self.user
            )),
        }?;
        tracing::trace!(?auth, ?task, "retrieved auth info");
        Ok(auth)
    }

    async fn list_tags_for(&mut self, task: TaskId) -> anyhow::Result<Vec<TagId>> {
        Ok(sqlx::query!(
            r#"SELECT tag_id AS "tag_id!" FROM v_tasks_tags WHERE task_id = $1"#,
            task.0
        )
        .map(|r| TagId(r.tag_id))
        .fetch_all(&mut *self.conn)
        .await?)
    }

    async fn get_comment_owner(&mut self, event: EventId) -> anyhow::Result<UserId> {
        Ok(UserId(
            sqlx::query!(
                "SELECT owner_id FROM add_comment_events WHERE id = $1",
                event.0
            )
            .fetch_one(&mut *self.conn)
            .await?
            .owner_id,
        ))
    }

    async fn get_task_for_comment(&mut self, comment: EventId) -> anyhow::Result<TaskId> {
        Ok(TaskId(
            sqlx::query!(
                "SELECT task_id FROM add_comment_events WHERE id = $1",
                comment.0
            )
            .fetch_one(&mut *self.conn)
            .await?
            .task_id,
        ))
    }

    async fn is_first_comment(&mut self, task: TaskId, comment: EventId) -> anyhow::Result<bool> {
        Ok(sqlx::query!(
            "SELECT id FROM add_comment_events WHERE task_id = $1 ORDER BY date LIMIT 1",
            task.0
        )
        .fetch_one(&mut *self.conn)
        .await?
        .id == comment.0)
    }
}

pub async fn login_user(
    db: &mut sqlx::PgConnection,
    s: &NewSession,
) -> anyhow::Result<Option<AuthToken>> {
    let session_id = Uuid::new_v4();
    let now = Utc::now();
    let rows_inserted = sqlx::query!(
        "
            INSERT INTO sessions
            SELECT $1, id, $2, $3, $3
            FROM users
            WHERE name = $4 AND password = $5
        ",
        session_id,
        s.device,
        now.naive_utc(),
        s.user,
        s.password, // TODO: password should be salted (eg. user + "risuto" + password)
    )
    .execute(db)
    .await
    .with_context(|| format!("authenticating user {:?}", s.user))?
    .rows_affected();
    assert!(
        rows_inserted <= 1,
        "inserted more than 1 row: {}",
        rows_inserted
    );
    Ok((rows_inserted == 1).then(|| AuthToken(session_id)))
}

/// Returns true iff a user was actually logged out
pub async fn logout_user(db: &mut sqlx::PgConnection, user: &AuthToken) -> anyhow::Result<bool> {
    let rows_deleted = sqlx::query!(
        "
            DELETE FROM sessions
            WHERE id = $1
        ",
        user.0,
    )
    .execute(db)
    .await
    .with_context(|| format!("deauthenticating session with token {:?}", user))?
    .rows_affected();
    assert!(
        rows_deleted <= 1,
        "deleted more than 1 row: {}",
        rows_deleted
    );
    Ok(rows_deleted == 1)
}

pub async fn recover_session(
    db: &mut sqlx::PgConnection,
    token: AuthToken,
) -> Result<UserId, Error> {
    let res = sqlx::query!(
        "
            UPDATE sessions
            SET last_active = $1
            WHERE id=$2
            RETURNING user_id
        ",
        Utc::now().naive_utc(),
        token.0,
    )
    .fetch_all(db)
    .await
    .with_context(|| format!("getting user id for session {:?}", token))?;
    assert!(
        res.len() <= 1,
        "got multiple results for primary key request"
    );
    if res.is_empty() {
        Err(Error::PermissionDenied)
    } else {
        Ok(UserId(res[0].user_id))
    }
}

pub fn users_interested_by<'conn>(
    conn: &'conn mut sqlx::PgConnection,
    tasks: &[Uuid], // TODO: when safe-transmute happens we can just take &[TaskId]
) -> impl 'conn + Stream<Item = anyhow::Result<UserId>> {
    sqlx::query!(
        r#"
            SELECT DISTINCT
                user_id AS "user_id!"
            FROM v_tasks_users
            WHERE task_id = ANY($1)
        "#,
        tasks
    )
    .fetch(conn)
    .map(|r| r.map(|u| UserId(u.user_id)).map_err(anyhow::Error::from))
}

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
) -> anyhow::Result<HashMap<TagId, (Tag, AuthInfo)>> {
    Ok(sqlx::query!(
        r#"
            SELECT
                t.id,
                t.owner_id,
                t.name,
                t.archived,
                u.name AS owner_name,
                vtu.can_edit AS "can_edit!",
                vtu.can_triage AS "can_triage!",
                vtu.can_relabel_to_any AS "can_relabel_to_any!",
                vtu.can_comment AS "can_comment!"
            FROM tags t
            INNER JOIN v_tags_users vtu
                ON vtu.tag_id = t.id
            INNER JOIN users u
                ON u.id = vtu.user_id
            WHERE vtu.user_id = $1
        "#,
        user.0
    )
    .fetch(conn)
    .map_ok(|t| {
        (
            TagId(t.id),
            (
                Tag {
                    owner: UserId(t.owner_id),
                    name: if t.owner_id == user.0 {
                        t.name
                    } else {
                        format!("{}:{}", t.owner_name, t.name)
                    },
                    archived: t.archived,
                },
                AuthInfo {
                    can_read: true,
                    can_edit: t.can_edit,
                    can_triage: t.can_triage,
                    can_relabel_to_any: t.can_relabel_to_any,
                    can_comment: t.can_comment,
                },
            ),
        )
    })
    .try_collect::<HashMap<TagId, (Tag, AuthInfo)>>()
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

pub async fn submit_event(conn: &mut sqlx::PgConnection, e: NewEvent) -> Result<(), Error> {
    let event_id = e.id;

    // Check authorization
    let mut db = PostgresDb {
        conn,
        user: e.owner,
    };
    let auth = e
        .is_authorized(&mut db)
        .await
        .with_context(|| format!("checking if user is authorized to add event {:?}", event_id))?;
    if !auth {
        tracing::info!("rejected permission for event {:?}", e);
        return Err(Error::PermissionDenied);
    }

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
            .bind(e.id.0)
            .bind(e.owner.0)
            .bind(e.date)
            $(.bind($v))*
            .execute(&mut *db.conn)
            .await
            .with_context(|| format!("inserting {} {:?}", $table, event_id))?
        }}
    }

    let res = match e.contents {
        NewEventContents::SetTitle { task, title } => {
            insert_event!("set_title_events", "$4, $5", task.0, title)
        }
        NewEventContents::SetDone { task, now_done } => {
            insert_event!("set_task_done_events", "$4, $5", task.0, now_done)
        }
        NewEventContents::SetArchived { task, now_archived } => {
            insert_event!("set_task_archived_events", "$4, $5", task.0, now_archived)
        }
        NewEventContents::Schedule {
            task,
            scheduled_date,
        } => insert_event!(
            "schedule_events",
            "$4, $5",
            task.0,
            scheduled_date.map(|d| d.naive_utc())
        ),
        NewEventContents::AddDep { first, then } => {
            insert_event!("add_dependency_events", "$4, $5", first.0, then.0)
        }
        NewEventContents::RmDep { first, then } => {
            insert_event!("remove_dependency_events", "$4, $5", first.0, then.0)
        }
        NewEventContents::AddTag {
            task,
            tag,
            prio,
            backlog,
        } => insert_event!(
            "add_tag_events",
            "$4, $5, $6, $7",
            task.0,
            tag.0,
            prio,
            backlog
        ),
        NewEventContents::RmTag { task, tag } => {
            insert_event!("remove_tag_events", "$4, $5", task.0, tag.0)
        }
        NewEventContents::AddComment { task, text } => {
            insert_event!("add_comment_events", "$4, $5", task.0, text)
        }
        NewEventContents::EditComment { comment, text, .. } => {
            insert_event!("edit_comment_events", "$4, $5", comment.0, text)
        }
    };

    if res.rows_affected() != 1 {
        Err(anyhow!(
            "insertion of event {:?} affected {} rows",
            event_id,
            res.rows_affected(),
        ))?;
    }
    // TODO: give a specific error if the event id is already taken

    Ok(())
}
