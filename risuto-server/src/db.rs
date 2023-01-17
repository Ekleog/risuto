use anyhow::{anyhow, Context};
use axum::async_trait;
use chrono::Utc;
use futures::{Future, Stream, StreamExt, TryStreamExt};
use risuto_api::{
    AuthInfo, AuthToken, Event, EventData, EventId, NewSession, Order, OrderId, OrderType, Query,
    Search, SearchId, Tag, TagId, Task, TaskId, Time, User, UserId, Uuid,
};
use std::pin::Pin;

use crate::{query, Error};

pub struct PostgresDb<'a> {
    pub conn: &'a mut sqlx::PgConnection,
    pub user: UserId,
}

#[derive(Debug, Eq, PartialEq, sqlx::Type)]
#[sqlx(type_name = "event_type", rename_all = "snake_case")]
enum DbType {
    SetTitle,
    SetDone,
    SetArchived,
    BlockedUntil,
    ScheduleFor,
    SetOrder,
    AddTag,
    RemoveTag,
    AddComment,
    EditComment,
    SetEventRead,
}

#[derive(Debug, Eq, PartialEq, sqlx::Type)]
#[sqlx(type_name = "order_type", rename_all = "snake_case")]
enum DbOrderType {
    Custom,
    Tag,
    CreationDateAsc,
    CreationDateDesc,
    LastEventDateAsc,
    LastEventDateDesc,
    ScheduledForAsc,
    ScheduledForDesc,
    BlockedUntilAsc,
    BlockedUntilDesc,
}

impl DbOrderType {
    fn into_api(self, id: Uuid, tag_id: Option<Uuid>) -> Order {
        match self {
            DbOrderType::Custom => Order::Custom(OrderId(id)),
            DbOrderType::Tag => Order::Tag(TagId(tag_id.expect("ill-formed db entry"))),
            DbOrderType::CreationDateAsc => Order::CreationDate(OrderType::Asc),
            DbOrderType::CreationDateDesc => Order::CreationDate(OrderType::Desc),
            DbOrderType::LastEventDateAsc => Order::LastEventDate(OrderType::Asc),
            DbOrderType::LastEventDateDesc => Order::LastEventDate(OrderType::Desc),
            DbOrderType::ScheduledForAsc => Order::ScheduledFor(OrderType::Asc),
            DbOrderType::ScheduledForDesc => Order::ScheduledFor(OrderType::Desc),
            DbOrderType::BlockedUntilAsc => Order::BlockedUntil(OrderType::Asc),
            DbOrderType::BlockedUntilDesc => Order::BlockedUntil(OrderType::Desc),
        }
    }
}

#[derive(sqlx::FromRow)]
struct DbTask {
    id: Uuid,
    owner_id: Uuid,
    date: chrono::NaiveDateTime,

    initial_title: String,
}

impl From<Task> for DbTask {
    fn from(t: Task) -> DbTask {
        DbTask {
            id: t.id.0,
            owner_id: t.owner_id.0,
            date: t.date.naive_utc(),
            initial_title: t.initial_title,
        }
    }
}

impl From<DbTask> for Task {
    fn from(t: DbTask) -> Task {
        Task {
            id: TaskId(t.id),
            owner_id: UserId(t.owner_id),
            date: t.date.and_local_timezone(chrono::Utc).unwrap(),
            initial_title: t.initial_title,
        }
    }
}

#[derive(Debug, Eq, PartialEq, sqlx::FromRow)]
struct DbEvent {
    id: Uuid,
    owner_id: Uuid,
    date: chrono::NaiveDateTime,
    task_id: Uuid,

    d_type: DbType,
    d_text: Option<String>,
    d_bool: Option<bool>,
    d_int: Option<i64>,
    d_time: Option<chrono::NaiveDateTime>,
    d_tag_id: Option<Uuid>,
    d_parent_id: Option<Uuid>,
    d_order_id: Option<Uuid>,
}

impl DbEvent {
    fn d_type(mut self, t: DbType) -> DbEvent {
        self.d_type = t;
        self
    }
    fn d_text(mut self, t: String) -> DbEvent {
        self.d_text = Some(t);
        self
    }
    fn d_bool(mut self, b: bool) -> DbEvent {
        self.d_bool = Some(b);
        self
    }
    fn d_time(mut self, t: Option<Time>) -> DbEvent {
        self.d_time = t.map(|t| t.naive_utc());
        self
    }
    fn d_tag_id(mut self, t: TagId) -> DbEvent {
        self.d_tag_id = Some(t.0);
        self
    }
    fn d_int(mut self, i: i64) -> DbEvent {
        self.d_int = Some(i);
        self
    }
    fn d_parent_id(mut self, p: Option<EventId>) -> DbEvent {
        self.d_parent_id = p.map(|p| p.0);
        self
    }
    fn d_order_id(mut self, o: OrderId) -> DbEvent {
        self.d_order_id = Some(o.0);
        self
    }
}

impl From<Event> for DbEvent {
    fn from(e: Event) -> DbEvent {
        let res = DbEvent {
            id: e.id.0,
            owner_id: e.owner_id.0,
            date: e.date.naive_utc(),
            task_id: e.task_id.0,
            d_type: DbType::SetTitle, // will be overwritten below
            d_text: None,
            d_bool: None,
            d_time: None,
            d_tag_id: None,
            d_int: None,
            d_parent_id: None,
            d_order_id: None,
        };
        use EventData::*;
        match e.data {
            SetTitle(t) => res.d_type(DbType::SetTitle).d_text(t),
            SetDone(b) => res.d_type(DbType::SetDone).d_bool(b),
            SetArchived(b) => res.d_type(DbType::SetArchived).d_bool(b),
            BlockedUntil(t) => res.d_type(DbType::BlockedUntil).d_time(t),
            ScheduleFor(t) => res.d_type(DbType::ScheduleFor).d_time(t),
            SetOrder { order, prio } => res.d_order_id(order).d_int(prio),
            AddTag { tag, prio, backlog } => res
                .d_type(DbType::AddTag)
                .d_tag_id(tag)
                .d_int(prio)
                .d_bool(backlog),
            RmTag(t) => res.d_type(DbType::RemoveTag).d_tag_id(t),
            AddComment { text, parent_id } => res
                .d_type(DbType::AddComment)
                .d_text(text)
                .d_parent_id(parent_id),
            EditComment { text, comment_id } => res
                .d_type(DbType::EditComment)
                .d_text(text)
                .d_parent_id(Some(comment_id)),
            SetEventRead { event_id, now_read } => res
                .d_type(DbType::SetEventRead)
                .d_bool(now_read)
                .d_parent_id(Some(event_id)),
        }
    }
}

impl From<DbEvent> for Event {
    fn from(e: DbEvent) -> Event {
        Event {
            id: EventId(e.id),
            owner_id: UserId(e.owner_id),
            date: e.date.and_local_timezone(chrono::Utc).unwrap(),
            task_id: TaskId(e.task_id),
            data: match e.d_type {
                DbType::SetTitle => {
                    EventData::SetTitle(e.d_text.expect("set_title event without title"))
                }
                DbType::SetDone => {
                    EventData::SetDone(e.d_bool.expect("set_done event without new_val_bool"))
                }
                DbType::SetArchived => EventData::SetArchived(
                    e.d_bool.expect("set_archived event without new_val_bool"),
                ),
                DbType::BlockedUntil => EventData::BlockedUntil(
                    e.d_time.map(|t| t.and_local_timezone(chrono::Utc).unwrap()),
                ),
                DbType::ScheduleFor => EventData::ScheduleFor(
                    e.d_time.map(|t| t.and_local_timezone(chrono::Utc).unwrap()),
                ),
                DbType::SetOrder => EventData::SetOrder {
                    order: OrderId(e.d_order_id.expect("set_order event without order_id")),
                    prio: e.d_int.expect("set_order event without prio"),
                },
                DbType::AddTag => EventData::AddTag {
                    tag: TagId(e.d_tag_id.expect("add_tag event without tag_id")),
                    prio: e.d_int.expect("add_tag event without new_val_int"),
                    backlog: e.d_bool.expect("add_tag event without new_val_bool"),
                },
                DbType::RemoveTag => {
                    EventData::RmTag(TagId(e.d_tag_id.expect("remove_tag event without tag_id")))
                }
                DbType::AddComment => EventData::AddComment {
                    text: e.d_text.expect("add_comment event without text"),
                    parent_id: e.d_parent_id.map(EventId),
                },
                DbType::EditComment => EventData::EditComment {
                    text: e.d_text.expect("edit_comment event without text"),
                    comment_id: EventId(
                        e.d_parent_id.expect("edit_comment event without parent_id"),
                    ),
                },
                DbType::SetEventRead => EventData::SetEventRead {
                    event_id: EventId(
                        e.d_parent_id
                            .expect("set_event_read event without parent_id"),
                    ),
                    now_read: e.d_bool.expect("set_event_read event without new_val_bool"),
                },
            },
        }
    }
}

#[async_trait]
impl<'a> risuto_api::Db for PostgresDb<'a> {
    fn current_user(&self) -> UserId {
        self.user
    }

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
            r#"SELECT tag_id AS "tag_id!" FROM v_tasks_tags WHERE task_id = $1 AND is_in = true"#,
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
                AND d_type = 'add_comment'
                AND d_parent_id IS NULL
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

pub async fn fetch_users(conn: &mut sqlx::PgConnection) -> anyhow::Result<Vec<User>> {
    Ok(sqlx::query!("SELECT id, name FROM users")
        .fetch(conn)
        .map_ok(|u| User {
            id: UserId(u.id),
            name: u.name,
        })
        .try_collect()
        .await
        .context("querying users table")?)
}

pub async fn fetch_tags_for_user(
    conn: &mut sqlx::PgConnection,
    user: &UserId,
) -> anyhow::Result<Vec<(Tag, AuthInfo)>> {
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
            Tag {
                id: TagId(t.id),
                owner_id: UserId(t.owner_id),
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
        )
    })
    .try_collect()
    .await
    .context("querying tags table")?)
}

pub async fn fetch_searches_for_user(
    conn: &mut sqlx::PgConnection,
    user: &UserId,
) -> anyhow::Result<Vec<Search>> {
    Ok(sqlx::query!(
        r#"
            SELECT
                id,
                name,
                filter AS "filter: sqlx::types::Json<Query>",
                order_type AS "order_type: DbOrderType",
                priority,
                tag_id
            FROM searches
            WHERE owner_id = $1
        "#,
        user.0
    )
    .fetch(conn)
    .map_ok(|s| Search {
        id: SearchId(s.id),
        name: s.name,
        filter: s.filter.0,
        priority: s.priority,
        order: s.order_type.into_api(s.id, s.tag_id),
    })
    .try_collect()
    .await
    .context("querying tags table")?)
}

pub async fn search_tasks_for_user(
    conn: &mut sqlx::PgConnection,
    owner: UserId,
    query: &Query,
) -> anyhow::Result<(Vec<Task>, Vec<Event>)> {
    let query::Sql {
        where_clause,
        binds,
    } = query::to_postgres(&query, 2);
    with_tmp_tasks_table(&mut *conn, |conn| {
        Box::pin(async move {
            let query = format!(
                "
                    INSERT INTO tmp_tasks
                    SELECT DISTINCT t.id
                        FROM tasks t
                    LEFT JOIN v_tasks_users vtu
                        ON vtu.task_id = t.id
                    LEFT JOIN v_tasks_archived vta
                        ON vta.task_id = t.id
                    LEFT JOIN v_tasks_done vtd
                        ON vtd.task_id = t.id
                    LEFT JOIN v_tasks_tags vtt
                        ON vtt.task_id = t.id
                    LEFT JOIN v_tasks_is_tagged vtit
                        ON vtit.task_id = t.id
                    LEFT JOIN v_tasks_scheduled vts
                        ON vts.task_id = t.id AND vts.owner_id = $1
                    LEFT JOIN v_tasks_blocked vtb
                        ON vtb.task_id = t.id
                    LEFT JOIN v_tasks_comments vtc
                        ON vtc.task_id = t.id
                    WHERE vtu.user_id = $1
                    AND {where_clause}
                "
            );
            let mut q = sqlx::query(&query).bind(owner.0);
            for b in binds {
                match b {
                    query::Bind::Bool(b) => q = q.bind(b),
                    query::Bind::Uuid(u) => q = q.bind(u),
                    query::Bind::String(s) => q = q.bind(s),
                    query::Bind::Time(t) => q = q.bind(t.naive_utc()),
                };
            }
            q.execute(&mut *conn)
                .await
                .context("filling temp table with interesting task ids")?;

            fetch_tasks_from_tmp_tasks_table(&mut *conn).await
        })
    })
    .await
}

async fn fetch_tasks_from_tmp_tasks_table(
    conn: &mut sqlx::PgConnection,
) -> anyhow::Result<(Vec<Task>, Vec<Event>)> {
    let tasks = sqlx::query_as::<_, DbTask>(
        "
            SELECT t.id, t.owner_id, t.date, t.initial_title
                FROM tmp_tasks interesting_tasks
            INNER JOIN tasks t
                ON t.id = interesting_tasks.id
        ",
    )
    .fetch(&mut *conn)
    .map_ok(Task::from)
    .try_collect()
    .await
    .context("fetching relevant tasks")?;

    let events = sqlx::query_as::<_, DbEvent>(
        "
            SELECT e.*
            FROM tmp_tasks t
            INNER JOIN events e
            ON t.id = e.task_id
        ",
    )
    .fetch(&mut *conn)
    .map_ok(Event::from)
    .try_collect()
    .await
    .context("fetching relevant events")?;

    Ok((tasks, events))
}

pub async fn submit_event(db: &mut PostgresDb<'_>, e: Event) -> Result<(), Error> {
    let event_id = e.id;

    // Check authorization
    let auth = e
        .is_authorized(&mut *db)
        .await
        .with_context(|| format!("checking if user is authorized to add event {:?}", event_id))?;
    if !auth {
        tracing::info!("rejected permission for event {:?}", e);
        return Err(Error::PermissionDenied);
    }

    let e = DbEvent::from(e);
    let res = sqlx::query!(
        "INSERT INTO events VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        &e.id,
        &e.owner_id,
        &e.date,
        &e.task_id,
        &e.d_type as &DbType,
        e.d_text.as_ref(),
        e.d_bool.as_ref(),
        e.d_int.as_ref(),
        e.d_time.as_ref(),
        e.d_tag_id.as_ref(),
        e.d_parent_id.as_ref(),
    )
    .execute(&mut *db.conn)
    .await
    .with_context(|| format!("inserting event {:?}", event_id))?;

    match res.rows_affected() {
        1 => Ok(()),
        0 => {
            let already_present = sqlx::query_as::<_, DbEvent>("SELECT * FROM events WHERE id=$1")
                .bind(e.id)
                .fetch_optional(&mut *db.conn)
                .await
                .context("sanity-checking the already-present event")?;
            match already_present {
                Some(p) if p == e => Ok(()),
                Some(p) if p.id == e.id => Err(Error::UuidAlreadyUsed(e.id)),
                _ => Err(Error::Anyhow(anyhow!("unknown event insertion conflict: trying to insert {e:?}, already had {already_present:?}")))
            }
        }
        rows => panic!("insertion of single event {event_id:?} affected multiple ({rows}) rows"),
    }
}

pub async fn submit_task(db: &mut PostgresDb<'_>, t: Task) -> Result<(), Error> {
    let task_id = t.id.0;

    let res = sqlx::query!(
        "INSERT INTO tasks VALUES ($1, $2, $3, $4)",
        &t.id.0,
        &t.owner_id.0,
        &t.date.naive_utc(),
        &t.initial_title,
    )
    .execute(&mut *db.conn)
    .await
    .with_context(|| format!("creating task {:?}", t.id))?;

    match res.rows_affected() {
        1 => Ok(()),
        0 => {
            let already_present = sqlx::query!("SELECT * FROM tasks WHERE id=$1", t.id.0)
                .fetch_optional(&mut *db.conn)
                .await
                .context("sanity-checking the already-present event")?;
            match already_present {
                Some(p) if p.id == t.id.0 && p.owner_id == t.owner_id.0 && p.date == t.date.naive_utc() && p.initial_title == t.initial_title => Ok(()),
                Some(p) if p.id == t.id.0 => Err(Error::UuidAlreadyUsed(p.id)),
                _ => Err(Error::Anyhow(anyhow!("unknown event insertion conflict: trying to insert {t:?}, already had {already_present:?}")))
            }
        }
        rows => panic!("insertion of single event {task_id:?} affected multiple ({rows}) rows"),
    }
}
