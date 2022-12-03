use futures::{channel::oneshot, select, FutureExt, SinkExt, StreamExt};
use risuto_api::*;
use ws_stream_wasm::{WsMessage, WsMeta};

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
    feed_sender: yew::html::Scope<crate::App>,
    mut cancel: oneshot::Sender<()>,
) {
    // Connect to websocket
    // TODO: split connect & auth into another function that returns an error on perm-denied
    let ws_url = format!(
        "ws{}/ws/event-feed",
        login.host.strip_prefix("http").expect("TODO")
    );
    let (_, mut sock) = WsMeta::connect(ws_url, None).await.expect("TODO");

    // Authentify
    let mut buf = Uuid::encode_buffer();
    sock.send(WsMessage::Text(
        login.token.0.as_hyphenated().encode_lower(&mut buf).into(),
    ))
    .await
    .expect("TODO");
    let res = sock.next().await.expect("TODO");
    assert_eq!(res, WsMessage::Text("ok".into()));

    // Finally, run the event feed
    let mut sock = sock.fuse();
    let mut cancellation = cancel.cancellation().fuse();
    loop {
        // TODO: ping-pong to detect disconnection
        select! {
            _ = cancellation => {
                sock.into_inner().close().await.expect("TODO");
                tracing::info!("disconnected from event feed");
                return;
            }
            msg = sock.next() => {
                let msg = match msg {
                    None => panic!("TODO"),
                    Some(WsMessage::Text(t)) => serde_json::from_str(&t),
                    Some(WsMessage::Binary(b)) => serde_json::from_slice(&b),
                }.expect("TODO");
                feed_sender.send_message(crate::AppMsg::NewNetworkEvent(msg));
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
