use anyhow::{anyhow, Context};
use axum::async_trait;
use chrono::Utc;
use futures::{Future, Stream, StreamExt, TryStreamExt};
use risuto_api::{
    AuthInfo, AuthToken, DbDump, Event, EventId, EventType, NewSession, Tag, TagId, Task, TaskId,
    Time, User, UserId, Uuid,
};
use sqlx::Row;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    pin::Pin,
};

use crate::Error;

pub struct PostgresDb<'a> {
    pub conn: &'a mut sqlx::PgConnection,
    pub user: UserId,
}

#[derive(sqlx::Type)]
#[sqlx(type_name = "event_type", rename_all = "snake_case")]
enum DbType {
    SetTitle,
    SetDone,
    SetArchived,
    BlockedUntil,
    ScheduleFor,
    AddTag,
    RemoveTag,
    AddComment,
    EditComment,
    SetEventRead,
}

#[derive(sqlx::FromRow)]
struct DbEvent {
    id: Uuid,
    owner_id: Uuid,
    date: chrono::NaiveDateTime,

    #[sqlx(rename = "type")]
    type_: DbType,
    task_id: Uuid,

    title: Option<String>,
    new_val_bool: Option<bool>,
    time: Option<chrono::NaiveDateTime>,
    tag_id: Option<Uuid>,
    new_val_int: Option<i64>,
    comment: Option<String>,
    parent_id: Option<Uuid>,
}

impl DbEvent {
    fn type_(mut self, t: DbType) -> DbEvent {
        self.type_ = t;
        self
    }
    fn title(mut self, t: String) -> DbEvent {
        self.title = Some(t);
        self
    }
    fn new_val_bool(mut self, b: bool) -> DbEvent {
        self.new_val_bool = Some(b);
        self
    }
    fn time(mut self, t: Option<Time>) -> DbEvent {
        self.time = t.map(|t| t.naive_utc());
        self
    }
    fn tag_id(mut self, t: TagId) -> DbEvent {
        self.tag_id = Some(t.0);
        self
    }
    fn new_val_int(mut self, i: i64) -> DbEvent {
        self.new_val_int = Some(i);
        self
    }
    fn comment(mut self, c: String) -> DbEvent {
        self.comment = Some(c);
        self
    }
    fn parent_id(mut self, p: Option<EventId>) -> DbEvent {
        self.parent_id = p.map(|p| p.0);
        self
    }
}

impl From<Event> for DbEvent {
    fn from(e: Event) -> DbEvent {
        let res = DbEvent {
            id: e.id.0,
            owner_id: e.owner.0,
            date: e.date.naive_utc(),
            task_id: e.task.0,
            type_: DbType::SetTitle, // will be overwritten below
            title: None,
            new_val_bool: None,
            time: None,
            tag_id: None,
            new_val_int: None,
            comment: None,
            parent_id: None,
        };
        use EventType::*;
        match e.contents {
            SetTitle(t) => res.type_(DbType::SetTitle).title(t),
            SetDone(b) => res.type_(DbType::SetDone).new_val_bool(b),
            SetArchived(b) => res.type_(DbType::SetArchived).new_val_bool(b),
            BlockedUntil(t) => res.type_(DbType::BlockedUntil).time(t),
            ScheduleFor(t) => res.type_(DbType::ScheduleFor).time(t),
            AddTag { tag, prio, backlog } => res
                .type_(DbType::AddTag)
                .tag_id(tag)
                .new_val_int(prio)
                .new_val_bool(backlog),
            RmTag(t) => res.type_(DbType::RemoveTag).tag_id(t),
            AddComment { text, parent_id } => res
                .type_(DbType::AddComment)
                .comment(text)
                .parent_id(parent_id),
            EditComment { text, comment_id } => res
                .type_(DbType::EditComment)
                .comment(text)
                .parent_id(Some(comment_id)),
            SetEventRead { event_id, now_read } => res
                .type_(DbType::SetEventRead)
                .new_val_bool(now_read)
                .parent_id(Some(event_id)),
        }
    }
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

    async fn get_event_info(&mut self, event: EventId) -> anyhow::Result<(UserId, Time, TaskId)> {
        let res = sqlx::query!(
            "SELECT owner_id, date, task_id FROM events WHERE id = $1",
            event.0
        )
        .fetch_one(&mut *self.conn)
        .await?;
        Ok((
            UserId(res.owner_id),
            res.date.and_local_timezone(Utc).unwrap(),
            TaskId(res.task_id),
        ))
    }

    async fn is_first_comment(&mut self, task: TaskId, comment: EventId) -> anyhow::Result<bool> {
        Ok(sqlx::query!(
            "SELECT id FROM events
            WHERE task_id = $1
                AND type = 'add_comment'
                AND parent_id IS NULL
            ORDER BY date LIMIT 1",
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

async fn with_tmp_tasks_table<R, F>(conn: &mut sqlx::PgConnection, f: F) -> anyhow::Result<R>
where
    F: for<'a> FnOnce(
        &'a mut sqlx::PgConnection,
    ) -> Pin<Box<dyn 'a + Send + Future<Output = anyhow::Result<R>>>>,
{
    sqlx::query("CREATE TEMPORARY TABLE tmp_tasks (id UUID NOT NULL)")
        .execute(&mut *conn)
        .await
        .context("creating temp table")?;

    let res = f(&mut *conn).await;

    let drop_res = sqlx::query("DROP TABLE tmp_tasks")
        .execute(&mut *conn)
        .await
        .context("dropping temp table");
    if let Err(err) = drop_res {
        tracing::error!(?err, "failed dropping temp table");
    }

    res
}

pub async fn fetch_dump_unarchived(
    conn: &mut sqlx::PgConnection,
    owner: UserId,
) -> anyhow::Result<DbDump> {
    let users = fetch_users(&mut *conn).await.context("fetching users")?;
    let tags = fetch_tags_for_user(&mut *conn, owner)
        .await
        .with_context(|| format!("fetching tags for user {:?}", owner))?;

    let tasks = with_tmp_tasks_table(&mut *conn, |conn| {
        Box::pin(async move {
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

            fetch_tasks_from_tmp_tasks_table(&mut *conn).await
        })
    })
    .await?;

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
                ON u.id = t.owner_id
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
                blocked_until: None,
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

    let mut events_query = sqlx::query_as::<_, DbEvent>(
        "
            SELECT e.*
            FROM tmp_tasks t
            INNER JOIN events e
            ON t.id = e.task_id
        ",
    )
    .fetch(&mut *conn);
    while let Some(e) = events_query
        .try_next()
        .await
        .context("querying events table")?
    {
        if let Some(t) = tasks.get_mut(&TaskId(e.task_id)) {
            t.add_event(Event {
                id: EventId(e.id),
                owner: UserId(e.owner_id),
                date: e.date.and_local_timezone(chrono::Utc).unwrap(),
                task: TaskId(e.task_id),
                contents: match e.type_ {
                    DbType::SetTitle => {
                        EventType::SetTitle(e.title.expect("set_title event without title"))
                    }
                    DbType::SetDone => EventType::SetDone(
                        e.new_val_bool.expect("set_done event without new_val_bool"),
                    ),
                    DbType::SetArchived => EventType::SetArchived(
                        e.new_val_bool
                            .expect("set_archived event without new_val_bool"),
                    ),
                    DbType::BlockedUntil => EventType::BlockedUntil(
                        e.time.map(|t| t.and_local_timezone(chrono::Utc).unwrap()),
                    ),
                    DbType::ScheduleFor => EventType::ScheduleFor(
                        e.time.map(|t| t.and_local_timezone(chrono::Utc).unwrap()),
                    ),
                    DbType::AddTag => EventType::AddTag {
                        tag: TagId(e.tag_id.expect("add_tag event without tag_id")),
                        prio: e.new_val_int.expect("add_tag event without new_val_int"),
                        backlog: e.new_val_bool.expect("add_tag event without new_val_bool"),
                    },
                    DbType::RemoveTag => {
                        EventType::RmTag(TagId(e.tag_id.expect("remove_tag event without tag_id")))
                    }
                    DbType::AddComment => EventType::AddComment {
                        text: e.comment.expect("add_comment event without text"),
                        parent_id: e.parent_id.map(EventId),
                    },
                    DbType::EditComment => EventType::EditComment {
                        text: e.comment.expect("edit_comment event without text"),
                        comment_id: EventId(
                            e.parent_id.expect("edit_comment event without parent_id"),
                        ),
                    },
                    DbType::SetEventRead => EventType::SetEventRead {
                        event_id: EventId(
                            e.parent_id.expect("set_event_read event without parent_id"),
                        ),
                        now_read: e
                            .new_val_bool
                            .expect("set_event_read event without new_val_bool"),
                    },
                },
            })
        }
    }

    for t in tasks.values_mut() {
        t.refresh_metadata();
    }

    Ok(tasks)
}

pub async fn submit_event(conn: &mut sqlx::PgConnection, e: Event) -> Result<(), Error> {
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

    let e = DbEvent::from(e);
    let res = sqlx::query!(
        "INSERT INTO events VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
        e.id,
        e.owner_id,
        e.date,
        e.type_ as DbType,
        e.task_id,
        e.title,
        e.new_val_bool,
        e.time,
        e.tag_id,
        e.new_val_int,
        e.comment,
        e.parent_id,
    )
    .execute(&mut *db.conn)
    .await
    .with_context(|| format!("inserting event {:?}", event_id))?;

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
