use anyhow::Context;
use axum::{
    extract::{ws::Message, State, WebSocketUpgrade},
    Json,
};
use futures::{SinkExt, StreamExt};
use risuto_api::{
    Action, AuthInfo, AuthToken, Event, NewSession, NewUser, Search, Tag, Task, User, UserId, Uuid,
};

use crate::{db, extractors::*, Error, UserFeeds};

pub async fn admin_create_user(
    AdminAuth: AdminAuth,
    State(feeds): State<UserFeeds>,
    mut conn: PgConn,
    Json(data): Json<NewUser>,
) -> Result<(), Error> {
    data.validate()?;
    db::create_user(&mut *conn, data.clone()).await?;
    feeds
        .relay_action(
            &mut *conn,
            Action::NewUser(User {
                id: data.id,
                name: data.name,
            }),
        )
        .await;
    Ok(())
}

pub async fn auth(
    mut conn: PgConn,
    Json(data): Json<NewSession>,
) -> Result<Json<AuthToken>, Error> {
    data.validate_except_pow()?;
    // in test setup, also allow the "empty" pow to work
    #[cfg(test)]
    if !data.verify_pow() && !data.pow.is_empty() {
        return Err(Error::invalid_pow());
    }
    #[cfg(not(test))]
    if !data.verify_pow() {
        return Err(Error::invalid_pow());
    }
    Ok(Json(
        db::login_user(&mut *conn, &data)
            .await
            .context("logging user in")?
            .ok_or(Error::permission_denied())?,
    ))
}

pub async fn unauth(user: PreAuth, mut conn: PgConn) -> Result<(), Error> {
    match db::logout_user(&mut *conn, &user.0).await {
        Ok(true) => Ok(()),
        Ok(false) => Err(Error::permission_denied()),
        Err(e) => Err(Error::Anyhow(e)),
    }
}

pub async fn whoami(Auth(user): Auth) -> Json<UserId> {
    Json(user)
}

pub async fn fetch_users(Auth(user): Auth, mut conn: PgConn) -> Result<Json<Vec<User>>, Error> {
    Ok(Json(db::fetch_users(&mut *conn).await.with_context(
        || format!("fetching user list for {:?}", user),
    )?))
}

pub async fn fetch_tags(
    Auth(user): Auth,
    mut conn: PgConn,
) -> Result<Json<Vec<(Tag, AuthInfo)>>, Error> {
    Ok(Json(
        db::fetch_tags_for_user(&mut *conn, &user)
            .await
            .with_context(|| format!("fetching tag list for {:?}", user))?,
    ))
}

pub async fn fetch_searches(
    Auth(user): Auth,
    mut conn: PgConn,
) -> Result<Json<Vec<Search>>, Error> {
    Ok(Json(
        db::fetch_searches_for_user(&mut *conn, &user)
            .await
            .with_context(|| format!("fetching saved search list for {:?}", user))?,
    ))
}

pub async fn search_tasks(
    Auth(user): Auth,
    mut conn: PgConn,
    Json(q): Json<risuto_api::Query>,
) -> Result<Json<(Vec<Task>, Vec<Event>)>, Error> {
    Ok(Json(
        db::search_tasks_for_user(&mut *conn, user, &q)
            .await
            .with_context(|| format!("fetching task list for {:?}", user))?,
    ))
}

pub async fn submit_action(
    Auth(user): Auth,
    State(feeds): State<UserFeeds>,
    mut conn: PgConn,
    Json(a): Json<Action>,
) -> Result<(), Error> {
    let mut db = db::PostgresDb {
        conn: &mut *conn,
        user,
    };
    match &a {
        Action::NewUser(_) => return Err(Error::permission_denied()),
        Action::NewTask(t, top_comm) => {
            if user != t.owner_id {
                return Err(Error::permission_denied());
            }
            db::submit_task(&mut db, t.clone(), top_comm.clone()).await?;
        }
        Action::NewEvent(e) => {
            if user != e.owner_id {
                return Err(Error::permission_denied());
            }
            db::submit_event(&mut db, e.clone()).await?;
        }
    }
    feeds.relay_action(&mut db.conn, a).await;
    Ok(())
}

pub async fn action_feed(
    ws: WebSocketUpgrade,
    State(db): State<PgPool>,
    State(feeds): State<UserFeeds>,
) -> Result<axum::response::Response, Error> {
    Ok(ws.on_upgrade(move |sock| {
        let (write, read) = sock.split();
        action_feed_impl(write, read, db, feeds)
    }))
}

pub async fn action_feed_impl<W, R>(mut write: W, mut read: R, db: PgPool, feeds: UserFeeds)
where
    W: 'static + Send + Unpin + futures::Sink<Message>,
    <W as futures::Sink<Message>>::Error: Send,
    R: 'static + Send + Unpin + futures::Stream<Item = Result<Message, axum::Error>>,
{
    // TODO: handle errors more gracefully
    // TODO: also log ip of other websocket end
    tracing::debug!("event feed websocket connected");
    if let Some(Ok(Message::Text(token))) = read.next().await {
        if let Ok(token) = Uuid::try_from(&token as &str) {
            if let Ok(mut conn) = db.acquire().await {
                if let Ok(user) = db::recover_session(&mut *conn, AuthToken(token)).await {
                    if let Ok(_) = write.send(Message::Text(String::from("ok"))).await {
                        tracing::debug!(?user, "event feed websocket auth success");
                        feeds.add_for_user(user, write, read).await;
                        return;
                    }
                }
            }
        }
        tracing::debug!(?token, "event feed websocket auth failure");
        let _ = write
            .send(Message::Text(String::from("permission denied")))
            .await;
    }
}
