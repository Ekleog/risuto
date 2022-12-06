use chrono::Utc;
use futures::{channel::oneshot, pin_mut, select, FutureExt, SinkExt, StreamExt};
use risuto_api::*;
use ws_stream_wasm::{WsMessage, WsMeta};

use crate::{ui, LoginInfo};

// TODO: make below chrono::Duration once https://github.com/chronotope/chrono/issues/309 fixeds
// Pings will be sent every PING_INTERVAL
const PING_INTERVAL_SECS: i64 = 10;
// If the interval between two pongs is more than DISCONNECT_INTERVAL, disconnect
const DISCONNECT_INTERVAL_SECS: i64 = 20;
// Space each reconnect attempt by ATTEMPT_SPACING
const ATTEMPT_SPACING_SECS: i64 = 1;

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

pub async fn unauth(client: reqwest::Client, host: String, token: AuthToken) {
    // TODO: make this a loop in case of network issues?
    let resp = client
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

pub async fn fetch_db_dump(client: &reqwest::Client, login: &LoginInfo) -> DbDump {
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

async fn sleep_for(d: chrono::Duration) {
    wasm_timer::Delay::new(d.to_std().unwrap_or(std::time::Duration::from_secs(0)))
        .await
        .expect("failed sleeping")
}

async fn sleep_until(t: Time) {
    sleep_for(t - Utc::now()).await
}

pub async fn start_event_feed(
    client: reqwest::Client,
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
        let db = fetch_db_dump(&client, &login).await;
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
                    let msg: FeedMessage = match msg {
                        None => continue 'reconnect,
                        Some(WsMessage::Text(t)) => serde_json::from_str(&t),
                        Some(WsMessage::Binary(b)) => serde_json::from_slice(&b),
                    }.expect("TODO");
                    match msg {
                        FeedMessage::Pong => last_pong = Utc::now(),
                        FeedMessage::NewEvent(e) => feed_sender.send_message(ui::AppMsg::NewNetworkEvent(e)),
                    }
                }
            }
        }
    }
}

pub async fn send_event(client: &reqwest::Client, login: &LoginInfo, event: NewEvent) {
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
