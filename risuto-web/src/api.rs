use futures::{
    channel::{
        mpsc::{self, UnboundedSender},
        oneshot,
    },
    future, select, FutureExt, StreamExt,
};
use gloo_storage::{LocalStorage, Storage};
use risuto_api::*;
use std::{collections::VecDeque, future::Future, pin::Pin};

use crate::LoginInfo;

pub async fn auth(
    client: reqwest::Client,
    host: String,
    session: NewSession,
) -> anyhow::Result<AuthToken> {
    Ok(client
        .post(format!("{}/api/auth", host))
        .json(&session)
        .send()
        .await?
        .json()
        .await?)
}

pub async fn fetch_db_dump(client: reqwest::Client, login: LoginInfo) -> DbDump {
    loop {
        match try_fetch_db_dump(&client, &login).await {
            Ok(db) => return db,
            Err(e) if e.is_timeout() => continue,
            // TODO: at least handle unauthorized error
            _ => panic!("failed to fetch db dump"), // TODO: should eg be a popup
        }
    }
}

async fn try_fetch_db_dump(client: &reqwest::Client, login: &LoginInfo) -> reqwest::Result<DbDump> {
    client
        .get(format!("{}/api/fetch-unarchived", login.host))
        .bearer_auth(login.token.0)
        .send()
        .await?
        .json()
        .await
}

pub async fn start_event_feed(
    login: LoginInfo,
    feed: UnboundedSender<NewEvent>,
    mut cancel: oneshot::Sender<()>,
) {
    // TODO: make a custom error type
    let ws_url = format!(
        "ws{}/api/ws",
        login.host.strip_prefix("http").expect("TODO")
    );
    let mut cancellation = cancel.cancellation().fuse();
    loop {
        select! {
            _ = cancellation => return,
            // todo
        }
    }
}

pub async fn handle_event_submissions(
    client: reqwest::Client,
    login: LoginInfo,
    queue: mpsc::UnboundedReceiver<NewEvent>,
) {
    let mut queue = queue.fuse();
    let mut to_send = LocalStorage::get("queue")
        .ok()
        .unwrap_or(VecDeque::<NewEvent>::new());
    // TODO: to_send should be exposed from the UI
    let mut currently_sending = false;
    let mut current_send =
        (Box::pin(future::pending()) as Pin<Box<dyn Future<Output = ()>>>).fuse();
    loop {
        if !currently_sending && !to_send.is_empty() {
            current_send = (Box::pin(send_event(&client, &login, to_send[0].clone()))
                as Pin<Box<dyn Future<Output = ()>>>)
                .fuse();
            currently_sending = true;
        }
        select! {
            e = queue.next() => {
                match e {
                    None => break,
                    Some(e) => {
                        to_send.push_back(e);
                        LocalStorage::set("queue", &to_send)
                            .expect("failed saving queue to local storage");
                    }
                }
            }
            _ = current_send => {
                to_send.pop_front();
                LocalStorage::set("queue", &to_send)
                    .expect("failed saving queue to local storage");
                currently_sending = false;
            }
        }
    }
}

async fn send_event(client: &reqwest::Client, login: &LoginInfo, event: NewEvent) {
    loop {
        let res = client
            .post(format!("{}/api/submit-event", login.host))
            .bearer_auth(login.token.0)
            .json(&event)
            .send()
            .await;
        match res {
            // TODO: panicking on server message is Bad(tm)
            // TODO: at least handle 403 forbidden answers
            Ok(r) if r.status().is_success() => break,
            Ok(r) => panic!("got non-successful response to event submission: {:?}", r),
            Err(e) if e.is_timeout() => continue,
            Err(e) => panic!("got reqwest error {:?}", e),
        }
    }
}
