use std::{collections::HashMap, iter, pin::Pin, sync::Arc};

use axum::extract::ws::Message;
use futures::{channel::mpsc, select, stream, SinkExt, Stream, StreamExt};
use risuto_api::{Action, FeedMessage, UserId, Uuid};
use tokio::sync::RwLock;

use crate::db;

#[derive(Clone, Debug)]
pub struct UserFeeds(
    Arc<RwLock<HashMap<UserId, HashMap<Uuid, mpsc::UnboundedSender<FeedMessage>>>>>,
);

impl UserFeeds {
    pub fn new() -> UserFeeds {
        UserFeeds(Arc::new(RwLock::new(HashMap::new())))
    }

    pub async fn add_for_user<W, R>(self, user: UserId, mut write: W, read: R)
    where
        W: 'static + Send + Unpin + futures::Sink<Message>,
        <W as futures::Sink<Message>>::Error: Send,
        R: 'static + Send + Unpin + futures::Stream<Item = Result<Message, axum::Error>>,
    {
        // Create relayer channel
        // Note: if this were bounded, there would be a deadlock between the write-lock to remove a channel and the read-lock to send an event to all interested sockets
        let (sender, mut receiver) = mpsc::unbounded();
        let sender_id = Uuid::new_v4();

        // Add relayer endpoint to hashmap
        // TODO: limit to some reasonable number of sockets, to avoid starvations
        self.0
            .write()
            .await
            .entry(user)
            .or_insert_with(HashMap::new)
            .insert(sender_id, sender);

        // Start relayer queue
        let this = self.clone();
        let user = user.clone();
        let mut read = read.fuse();
        tokio::spawn(async move {
            macro_rules! remove_self {
                () => {{
                    this.0
                        .write()
                        .await
                        .get_mut(&user)
                        .expect("user {user:?} disappeared")
                        .remove(&sender_id);
                    return;
                }};
            }
            macro_rules! send_message {
                ( $msg:expr ) => {{
                    let msg: FeedMessage = $msg;
                    let json = match serde_json::to_vec(&msg) {
                        Ok(json) => json,
                        Err(err) => {
                            tracing::error!(?err, ?msg, "failed serializing message to json");
                            continue;
                        }
                    };
                    if let Err(_) = write.send(Message::Binary(json)).await {
                        // TODO: check error details, using axum-tungstenite, to confirm we need to remove this socket
                        remove_self!();
                    }
                }};
            }
            loop {
                select! {
                    msg = receiver.next() => match msg {
                        None => remove_self!(),
                        Some(msg) => send_message!(msg),
                    },
                    msg = read.next() => match msg {
                        None => remove_self!(),
                        Some(Ok(Message::Close(_))) => remove_self!(),
                        Some(Ok(Message::Text(msg))) => {
                            if msg != "ping" {
                                tracing::warn!("received unexpected message from client: {msg:?}");
                                remove_self!();
                            }
                            send_message!(FeedMessage::Pong);
                        }
                        Some(msg) => {
                            tracing::warn!("received unexpected message from client: {msg:?}");
                            remove_self!();
                        }
                    },
                }
            }
        });
    }

    pub async fn relay_action(&self, conn: &mut sqlx::PgConnection, a: Action) {
        match &a {
            Action::NewUser(_) => match db::fetch_users(conn).await {
                Err(e) => Box::pin(stream::iter(iter::once(Err(e))))
                    as Pin<Box<dyn Send + Stream<Item = anyhow::Result<UserId>>>>,
                Ok(u) => Box::pin(stream::iter(u.into_iter().map(|u| Ok(u.id)))),
            },
            Action::NewTask(t, _) => Box::pin(stream::iter(iter::once(Ok(t.owner_id)))),
            Action::NewEvent(e) => Box::pin(db::users_interested_by(conn, &[e.task_id.0])),
            // TODO: make sure we actually send the whole task if a user gets access to this task it didn't have before
        }
        // TODO: magic numbers below should be at least explained
        .for_each_concurrent(Some(16), |u| {
            let a = a.clone();
            async move {
                match u {
                    Err(err) => {
                        tracing::error!(?err, "error occurred while listing interested users");
                    }
                    Ok(u) => {
                        if let Some(socks) = self.0.read().await.get(&u) {
                            for s in socks.values() {
                                let _ = s.unbounded_send(FeedMessage::Action(a.clone()));
                            }
                        }
                    }
                }
            }
        })
        .await;
    }
}
