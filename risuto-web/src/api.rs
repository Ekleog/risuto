use std::{collections::HashMap, sync::Arc};

use chrono::Utc;
use futures::{channel::oneshot, pin_mut, select, FutureExt, SinkExt, StreamExt};
use risuto_client::{
    api::{self, Time, Uuid},
    DbDump,
};
use ws_stream_wasm::{WsMessage, WsMeta};

use crate::{ui, LoginInfo};

// TODO: make below chrono::Duration once https://github.com/chronotope/chrono/issues/309 fixeds
// Pings will be sent every PING_INTERVAL
const PING_INTERVAL_SECS: i64 = 10;
// If the interval between two pongs is more than DISCONNECT_INTERVAL, disconnect
const DISCONNECT_INTERVAL_SECS: i64 = 20;
// Space each reconnect attempt by ATTEMPT_SPACING
const ATTEMPT_SPACING_SECS: i64 = 1;

pub async fn auth(host: String, session: api::NewSession) -> anyhow::Result<api::AuthToken> {
    Ok(crate::CLIENT
        .post(format!("{}/api/auth", host))
        .json(&session)
        .send()
        .await?
        .json()
        .await?)
}

pub async fn unauth(host: String, token: api::AuthToken) {
    let resp = crate::CLIENT
        .post(format!("{}/api/unauth", host))
        .bearer_auth(token.0)
        .send()
        .await;
    match resp {
        Err(e) => tracing::error!("failed to unauth: {:?}", e),
        Ok(resp) if !resp.status().is_success() => {
            tracing::error!("failed to unauth: response is not success {:?}", resp)
        }
        Ok(_) => (),
    }
}

async fn fetch<R>(login: &LoginInfo, fetcher: &str, body: Option<&api::Query>) -> R
where
    R: for<'de> serde::Deserialize<'de>,
{
    // TODO: at least handle unauthorized error
    let req = match body {
        None => crate::CLIENT.get(format!("{}/api/{}", login.host, fetcher)),
        Some(body) => crate::CLIENT
            .post(format!("{}/api/{}", login.host, fetcher))
            .json(body),
    };
    req.bearer_auth(login.token.0)
        .send()
        .await
        .expect("failed to fetch data from server") // TODO: should eg be a popup
        .json()
        .await
        .expect("failed to parse data from server") // TODO: should eg be a popup
}

async fn fetch_db_dump(login: &LoginInfo) -> DbDump {
    let mut db = DbDump {
        owner: fetch(login, "whoami", None).await,
        users: Arc::new(HashMap::new()),
        tags: Arc::new(HashMap::new()),
        perms: Arc::new(HashMap::new()),
        tasks: Arc::new(HashMap::new()),
    };

    db.add_users(fetch(login, "fetch-users", None).await);
    db.add_tags(fetch(login, "fetch-tags", None).await);
    let (tasks, events): (Vec<api::Task>, Vec<api::Event>) =
        fetch(login, "search-tasks", Some(&api::Query::Archived(false))).await;
    db.add_tasks(tasks);
    db.add_events_and_refresh_all(events);

    db
}

async fn sleep_for(d: chrono::Duration) {
    wasm_timer::Delay::new(d.to_std().unwrap_or(std::time::Duration::from_secs(0)))
        .await
        .expect("failed sleeping")
}

async fn sleep_until(t: Time) {
    sleep_for(t - Utc::now()).await
}

pub async fn start_event_feed(
    login: LoginInfo,
    feed_sender: yew::html::Scope<ui::App>,
    mut cancel: oneshot::Sender<()>,
) {
    let mut first_attempt = true;
    'reconnect: loop {
        match first_attempt {
            true => first_attempt = false,
            false => {
                tracing::warn!("lost event feed connection");
                feed_sender.send_message(ui::AppMsg::WebsocketDisconnected);
                sleep_for(chrono::Duration::seconds(ATTEMPT_SPACING_SECS)).await;
            }
        }

        // Connect to websocket
        let ws_url = format!(
            "ws{}/ws/event-feed",
            login.host.strip_prefix("http").expect("TODO")
        );
        let mut sock = match WsMeta::connect(ws_url, None).await {
            Ok((_, s)) => s,
            Err(_) => continue 'reconnect, // TODO: maybe the url is no tthe right one?
        };

        // Authentify
        let mut buf = Uuid::encode_buffer();
        sock.send(WsMessage::Text(
            login.token.0.as_hyphenated().encode_lower(&mut buf).into(),
        ))
        .await
        .expect("TODO");
        let res = match sock.next().await {
            Some(r) => r,
            None => continue 'reconnect,
        };
        assert_eq!(res, WsMessage::Text("ok".into())); // TODO: handle permission denied response
        tracing::info!("successfully authenticated to event feed");
        feed_sender.send_message(ui::AppMsg::WebsocketConnected);

        // Fetch the database
        // TODO: this should happen async from the websocket handling to not risk stalling the connection.
        // ui::App should already be ready to handle it thanks to its connection_state member
        let db = fetch_db_dump(&login).await;
        tracing::info!("successfully fetched database");
        feed_sender.send_message(ui::AppMsg::ReceivedDb(db));

        // Finally, run the event feed
        let mut next_ping = Utc::now();
        let mut last_pong = Utc::now();
        let mut sock = sock.fuse();
        let mut cancellation = cancel.cancellation().fuse();
        loop {
            let delay_pong_reception =
                sleep_until(last_pong + chrono::Duration::seconds(DISCONNECT_INTERVAL_SECS)).fuse();
            let delay_ping_send = sleep_until(next_ping).fuse();
            pin_mut!(delay_ping_send, delay_pong_reception);
            select! {
                _ = cancellation => {
                    sock.into_inner().close().await.expect("TODO");
                    tracing::info!("disconnected from event feed");
                    return;
                }
                _ = delay_pong_reception => continue 'reconnect,
                _ = delay_ping_send => {
                    sock.send(WsMessage::Text("ping".to_string())).await.expect("TODO");
                    next_ping += chrono::Duration::seconds(PING_INTERVAL_SECS);
                }
                msg = sock.next() => {
                    let msg: api::FeedMessage = match msg {
                        None => continue 'reconnect,
                        Some(WsMessage::Text(t)) => serde_json::from_str(&t),
                        Some(WsMessage::Binary(b)) => serde_json::from_slice(&b),
                    }.expect("TODO");
                    match msg {
                        api::FeedMessage::Pong => last_pong = Utc::now(),
                        api::FeedMessage::NewEvent(e) => feed_sender.send_message(ui::AppMsg::NewNetworkEvent(e)),
                    }
                }
            }
        }
    }
}

pub async fn send_event(login: &LoginInfo, event: api::Event) {
    let res = crate::CLIENT
        .post(format!("{}/api/submit-event", login.host))
        .bearer_auth(login.token.0)
        .json(&event)
        .send()
        .await;
    match res {
        // TODO: panicking on server message is Bad(tm)
        // TODO: at least handle 403 forbidden answers
        Ok(r) if r.status().is_success() => (),
        Ok(r) => panic!("got non-successful response to event submission: {:?}", r),
        Err(e) => panic!("got reqwest error {:?}", e),
    }
}
