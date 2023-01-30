#![cfg(test)]

use async_recursion::async_recursion;
use axum::{
    extract::{ws::Message, FromRequestParts},
    http::{self, request},
};
use futures::{channel::mpsc, StreamExt};
use risuto_api::{
    Action, Error as ApiError, FeedMessage, NewSession, NewUser, Query, User, UserId,
};
use risuto_mock_server::MockServer;
use std::{
    cmp, collections::VecDeque, fmt::Debug, ops::RangeTo, panic::AssertUnwindSafe, path::Path,
};
use tower::{Service, ServiceExt};

use crate::{extractors::*, *};

macro_rules! do_tokio_test {
    ( $name:ident, $typ:ty, $fn:expr ) => {
        #[test]
        fn $name() {
            let runtime = AssertUnwindSafe(
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("failed initializing tokio runtime"),
            );
            bolero::check!()
                .with_type::<$typ>()
                .cloned()
                .for_each(move |v| {
                    let () = runtime.block_on($fn(v));
                })
        }
    };
}

fn build_pg_cluster(data: &Path) -> postgresfixture::cluster::Cluster {
    let mut runtime = None;
    let mut best_version = None;
    for r in postgresfixture::runtime::Runtime::find_on_path() {
        if let Ok(v) = r.version() {
            match (&mut runtime, &mut best_version) {
                (None, None) => {
                    runtime = Some(r);
                    best_version = Some(v);
                }
                (Some(runtime), Some(best_version)) => {
                    if *best_version < v {
                        *runtime = r;
                        *best_version = v;
                    }
                }
                _ => unreachable!(),
            }
        }
    }
    postgresfixture::cluster::Cluster::new(
        data,
        runtime.expect("postgresql seems to not be installed in path"),
    )
}

macro_rules! do_sqlx_test {
    ( $name:ident, $gen:expr, $fn:expr ) => {
        #[test]
        fn $name() {
            if std::env::var("RUST_LOG").is_ok() {
                tracing_subscriber::fmt::init();
            }
            let tmpdir = tempfile::Builder::new().prefix("risuto-fuzz-db").tempdir().expect("creating tempdir");
            let lockfile = std::fs::File::create(tmpdir.path().join("lockfile")).expect("creating lockfile");
            let datadir = tmpdir.path().join("db");
            let cluster = build_pg_cluster(&datadir);
            let datadir_path: &str = datadir.to_str().expect("tempdir is not valid utf8");
            postgresfixture::coordinate::run_and_destroy(&cluster, lockfile.into(), || {
                cluster.createdb("test_db").expect("creating test_db database");
                let runtime = AssertUnwindSafe(
                    tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("failed initializing tokio runtime"),
                );
                // create test db
                let pool = AssertUnwindSafe(runtime.block_on(async move {
                    let pool = create_sqlx_pool(&format!("postgresql://?host={}&dbname=test_db", datadir_path)).await.expect("creating sqlx pool");
                    MIGRATOR
                        .run(&mut *pool.acquire().await.expect("getting migrator connection"))
                        .await
                        .expect("failed applying migrations");
                    pool
                }));
                bolero::check!()
                    .with_generator($gen)
                    .cloned()
                    .for_each(move |v| {
                        let pool = pool.clone();
                        // run the test
                        let idle_before = pool.num_idle();
                        let v_str = format!("{v:?}");
                        let idle_after_res: Result<usize, _> = {
                            let pool = pool.clone();
                            std::panic::catch_unwind(AssertUnwindSafe(|| {
                                runtime.block_on(async move {
                                    let () = $fn(pool.clone(), v).await;
                                    let mut idle_after = pool.num_idle();
                                    let wait_release_since = std::time::Instant::now();
                                    while idle_after < idle_before
                                        && wait_release_since.elapsed()
                                            <= std::time::Duration::from_secs(1)
                                    {
                                        tokio::task::yield_now().await;
                                        idle_after = pool.num_idle();
                                    }
                                    idle_after
                                })
                            }))
                        };
                        runtime.block_on(async move {
                            // cleanup
                            let mut conn =
                                pool.acquire().await.expect("getting db cleanup connection");
                            sqlx::query(include_str!("../reset-test-db.sql"))
                                .execute(&mut *conn)
                                .await
                                .expect("failed cleaning up database");
                        });
                        // resume the panics
                        match idle_after_res {
                            Err(e) => std::panic::resume_unwind(e),
                            Ok(idle_after) => assert!(
                                idle_after >= idle_before,
                                "test {} held onto pool after exiting test: before there were {idle_before} connections, and after there were {idle_after} with value {v_str}",
                                stringify!($name)
                            ),
                        }
                    });
            })
            .expect("coordinating spinup and shutdown of the pg cluster");
        }
    };
}

do_tokio_test!(fuzz_preauth_extractor, String, |token| async move {
    if let Ok(req) = http::Request::builder()
        .method(http::Method::GET)
        .uri("/")
        .header(http::header::AUTHORIZATION, token)
        .body(())
    {
        let mut req = req.into_parts().0;
        let res = PreAuth::from_request_parts(&mut req, &()).await;
        match res {
            Ok(_) => (),
            Err(Error::Api(ApiError::PermissionDenied)) => (),
            Err(e) => panic!("got unexpected error: {e}"),
        }
    }
});

// TODO: also allow generating invalid requests?
#[derive(Clone, Debug, bolero::generator::TypeGenerator)]
enum FuzzOp {
    CreateUser(NewUser),
    Auth {
        uid: usize,
        #[generator(bolero::gen_with::<String>().len(1..100usize))]
        device: String,
    },
    Unauth {
        sid: usize,
    },
    Whoami {
        sid: usize,
    },
    FetchUsers {
        sid: usize,
    },
    FetchTags {
        sid: usize,
    },
    FetchSearches {
        sid: usize,
    },
    SearchTasks {
        sid: usize,
        query: risuto_api::Query,
    },
    SubmitAction {
        sid: usize,
        evt: risuto_api::Action,
    },
    OpenActionFeed {
        sid: usize,
    },
    PingActionFeed {
        feed_id: usize,
    },
    CloseActionFeed {
        feed_id: usize,
    },
}

async fn call<Req, Resp>(
    app: &mut Router,
    req: request::Request<axum::body::Body>,
    req_body: &Req,
) -> Result<Resp, ApiError>
where
    Req: Debug,
    Resp: 'static + for<'de> serde::Deserialize<'de>,
{
    app.ready().await.expect("waiting for app to be ready");
    let resp = app.call(req).await.expect("running request");
    let status = resp.status();
    let body = hyper::body::to_bytes(resp.into_body())
        .await
        .expect("recovering resp bytes");
    if status == http::StatusCode::OK {
        if std::any::TypeId::of::<Resp>() == std::any::TypeId::of::<()>() {
            // the server returns an empty string in this situation, which does not parse properly with serde_json
            return Ok(serde_json::from_slice(b"null").unwrap());
        } else {
            return Ok(serde_json::from_slice(&body).unwrap_or_else(|err| {
                panic!(
                    r#"
                        Failed parsing resp body!

                        The error is the following:
                        ---
                        {err}
                        ---

                        Response body is:
                        ---
                        {body:?}
                        ---

                        Request was:
                        ---
                        {req_body:?}
                        ---
                    "#
                )
            }));
        }
    }
    Err(ApiError::parse(&body).unwrap_or_else(|err| {
        panic!(
            r#"
                Failed parsing resp error body!

                The error is the following:
                ---
                {err}
                ---

                Response body is:
                ---
                {body:?}
                ---

                Request was:
                ---
                {req_body:?}
                ---
            "#
        )
    }))
}

async fn run_on_app<Req, Resp>(
    app: &mut Router,
    method: &str,
    uri: &str,
    token: Option<Uuid>,
    body: &Req,
) -> Result<Resp, ApiError>
where
    Req: Debug + serde::Serialize,
    Resp: 'static + for<'de> serde::Deserialize<'de>,
{
    let req = request::Builder::new()
        .method(method)
        .uri(uri)
        .header(http::header::CONTENT_TYPE, "application/json");
    let req = match token {
        Some(token) => req.header(http::header::AUTHORIZATION, format!("bearer {token}")),
        None => req,
    };
    let req = req
        .body(axum::body::Body::from(
            serde_json::to_vec(body).expect("serializing request body to json"),
        ))
        .expect("building request");
    call(app, req, body).await
}

fn compare<T>(name: &str, app_res: Result<T, ApiError>, mock_res: Result<T, ApiError>)
where
    T: Debug + PartialEq,
{
    assert_eq!(
        app_res, mock_res,
        "app and mock did not return the same result for {name}"
    );
}

fn resize_int(fuzz_id: usize, RangeTo { end }: RangeTo<usize>) -> Option<usize> {
    if end == 0 {
        return None;
    }
    let bucket_size = cmp::max(1, usize::MAX / end); // in case we rounded to 0
    let id = fuzz_id / bucket_size;
    Some(cmp::min(id, end - 1)) // in case id was actually over end - 1 due to rounding
}

fn check_json_roundtrip_is_identity<T>(t: T) -> Option<T>
where
    T: Eq + serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    match serde_json::from_slice::<T>(&serde_json::to_vec(&t).expect("serializing to json")) {
        // recursion limit hit or similar issue
        Err(_) => None,

        // eg. chrono leap seconds are do not round-trip nicely through json
        // (https://github.com/chronotope/chrono/issues/944)
        Ok(deserialized) if t != deserialized => None,

        // All went fine
        Ok(_) => Some(t),
    }
}

fn sanitize_query(q: Query) -> Option<Query> {
    // TODO: make Query::Tag.tag be likely to actually be a valid tag (once CreateTag will be implemented)
    check_json_roundtrip_is_identity(q)
}

fn sanitize_action(action: Action) -> Option<Action> {
    // TODO: make Action::*Id likely to actually be valid IDs
    check_json_roundtrip_is_identity(action)
}

#[derive(Clone, Copy)]
struct Session {
    app: AuthToken,
    mock: AuthToken,
}

struct Feed {
    app_receiver: mpsc::UnboundedReceiver<Message>,
    app_sender: mpsc::UnboundedSender<Result<Message, axum::Error>>,
    mock_receiver: mpsc::UnboundedReceiver<Action>,
}

struct ComparativeFuzzer {
    admin_token: Uuid,
    app: Router,
    mock: MockServer,
    app_db: PgPool,
    app_feeds: UserFeeds,
    sessions: Vec<Session>,
    feeds: Vec<Option<Feed>>,
}

impl ComparativeFuzzer {
    async fn new(pool: PgPool) -> ComparativeFuzzer {
        let admin_token = Uuid::new_v4();
        let feeds = UserFeeds::new();
        let app = app(pool.clone(), feeds.clone(), Some(AuthToken(admin_token))).await;
        ComparativeFuzzer {
            admin_token,
            app,
            mock: MockServer::new(),
            app_db: pool,
            app_feeds: feeds.clone(),
            sessions: Vec::new(),
            feeds: Vec::new(),
        }
    }

    async fn get_session(&mut self, sid: usize) -> Session {
        match resize_int(sid, ..self.sessions.len()) {
            Some(sid) => self.sessions[sid],
            None => {
                self.execute_fuzz_op(FuzzOp::Auth {
                    uid: sid,
                    device: String::from("device"),
                })
                .await;
                self.sessions[0]
            }
        }
    }

    #[async_recursion]
    async fn execute_fuzz_op(&mut self, op: FuzzOp) {
        match op {
            FuzzOp::CreateUser(new_user) => {
                // no hashing for tests
                let pass = new_user.initial_password_hash.clone();
                compare(
                    "CreateUser",
                    run_on_app(
                        &mut self.app,
                        "POST",
                        "/api/admin/create-user",
                        Some(self.admin_token),
                        &new_user,
                    )
                    .await,
                    self.mock.admin_create_user(new_user, pass).await,
                )
            }
            FuzzOp::Auth { uid, device } => {
                if let Some(uid) = resize_int(uid, ..self.mock.test_num_users()) {
                    let (user, password) = self.mock.test_get_user_info(uid);
                    let session = NewSession {
                        user: String::from(user),
                        password: String::from(password),
                        device,
                        pow: String::new(),
                    };
                    let app_tok =
                        run_on_app(&mut self.app, "POST", "/api/auth", None, &session).await;
                    let mock_tok = self.mock.auth(session);
                    if let (&Ok(app), &Ok(mock)) = (&app_tok, &mock_tok) {
                        self.sessions.push(Session { app, mock });
                    }
                    compare("Auth", app_tok.map(|_| ()), mock_tok.map(|_| ()));
                } else {
                    self.execute_fuzz_op(FuzzOp::CreateUser(NewUser {
                        id: UserId::stub(),
                        name: String::from("user"),
                        initial_password_hash: String::from("password"),
                    }))
                    .await;
                    self.execute_fuzz_op(FuzzOp::Auth { uid, device }).await;
                }
            }
            FuzzOp::Unauth { sid } => {
                let sess = self.get_session(sid).await;
                compare(
                    "Unauth",
                    run_on_app(&mut self.app, "POST", "/api/unauth", Some(sess.app.0), &()).await,
                    self.mock.unauth(sess.mock),
                );
            }
            FuzzOp::Whoami { sid } => {
                let sess = self.get_session(sid).await;
                compare(
                    "Whoami",
                    run_on_app(&mut self.app, "GET", "/api/whoami", Some(sess.app.0), &()).await,
                    self.mock.whoami(sess.mock),
                );
            }
            FuzzOp::FetchUsers { sid } => {
                let sess = self.get_session(sid).await;
                let mut app_res: Result<Vec<User>, _> = run_on_app(
                    &mut self.app,
                    "GET",
                    "/api/fetch-users",
                    Some(sess.app.0),
                    &(),
                )
                .await;
                let _ = app_res.as_mut().map(|v| v.sort_by_key(|u| u.id));
                let mut mock_res = self.mock.fetch_users(sess.mock);
                let _ = mock_res.as_mut().map(|v| v.sort_by_key(|u| u.id));
                compare("FetchUsers", app_res, mock_res);
            }
            FuzzOp::FetchTags { sid } => {
                let sess = self.get_session(sid).await;
                compare(
                    "FetchTags",
                    run_on_app(
                        &mut self.app,
                        "GET",
                        "/api/fetch-tags",
                        Some(sess.app.0),
                        &(),
                    )
                    .await,
                    self.mock.fetch_tags(sess.mock),
                );
            }
            FuzzOp::FetchSearches { sid } => {
                let sess = self.get_session(sid).await;
                compare(
                    "FetchSearches",
                    run_on_app(
                        &mut self.app,
                        "GET",
                        "/api/fetch-searches",
                        Some(sess.app.0),
                        &(),
                    )
                    .await,
                    self.mock.fetch_searches(sess.mock),
                );
            }
            FuzzOp::SearchTasks { sid, query } => {
                let sess = self.get_session(sid).await;
                if let Some(query) = sanitize_query(query) {
                    compare(
                        "SearchTasks",
                        run_on_app(
                            &mut self.app,
                            "POST",
                            "/api/search-tasks",
                            Some(sess.app.0),
                            &query,
                        )
                        .await,
                        self.mock.search_tasks(sess.mock, query),
                    );
                }
            }
            FuzzOp::SubmitAction { sid, evt } => {
                let sess = self.get_session(sid).await;
                if let Some(evt) = sanitize_action(evt) {
                    compare(
                        "SubmitAction",
                        run_on_app(
                            &mut self.app,
                            "POST",
                            "/api/submit-action",
                            Some(sess.app.0),
                            &evt,
                        )
                        .await,
                        self.mock.submit_action(sess.mock, evt).await,
                    );
                }
            }
            FuzzOp::OpenActionFeed { sid } => {
                let sess = self.get_session(sid).await;
                let (app_sender, serv_receiver) = mpsc::unbounded();
                let (serv_sender, mut app_receiver) = mpsc::unbounded();
                let (_, app_res) = futures::join!(
                    async {
                        crate::handlers::action_feed_impl(
                            serv_sender,
                            serv_receiver,
                            self.app_db.clone(),
                            self.app_feeds.clone(),
                        )
                        .await;
                    },
                    async {
                        // TODO: also fuzz protocol violations here; but this should probably be a
                        // separate fuzzer
                        app_sender
                            .unbounded_send(Ok(Message::Text(format!("{}", sess.app.0))))
                            .expect("sending auth token to feed");
                        match app_receiver.next().await {
                            Some(Message::Text(t)) if t == "ok" => Ok(()),
                            Some(Message::Text(t)) if t == "permission denied" => {
                                Err(ApiError::PermissionDenied)
                            }
                            o => panic!("unexpected reply to auth request {o:?}"),
                        }
                    }
                );
                let (mock_res, mock_receiver) = match self.mock.action_feed(sess.mock).await {
                    Ok(receiver) => (Ok(()), Some(receiver)),
                    Err(e) => (Err(e), None),
                };
                compare("OpenActionFeed", app_res, mock_res);
                if let Some(mock_receiver) = mock_receiver {
                    self.feeds.push(Some(Feed {
                        app_sender,
                        app_receiver,
                        mock_receiver,
                    }));
                }
            }
            FuzzOp::PingActionFeed { feed_id } => {
                let feed_id = match resize_int(feed_id, ..self.feeds.len()) {
                    None => return,
                    Some(feed_id) => feed_id,
                };
                if let Some(f) = &mut self.feeds[feed_id] {
                    f.app_sender
                        .unbounded_send(Ok(Message::Text(String::from("ping"))))
                        .expect("sending ping");
                    for _attempt in 0..1000 {
                        match f.app_receiver.try_next() {
                            Ok(Some(Message::Binary(m))) => {
                                let m: FeedMessage = serde_json::from_slice(&m)
                                    .expect("failed parsing ping response from json");
                                match m {
                                    FeedMessage::Pong => return,
                                    m => panic!("received unexpected ping response: {m:?}"),
                                }
                            }
                            Err(_) => tokio::task::yield_now().await, // waiting for response
                            m => panic!("received unexpected answer to ping: {m:?}"),
                        }
                    }
                    panic!("did not receive ping response within allocated time");
                }
            }
            FuzzOp::CloseActionFeed { feed_id } => {
                let feed_id = match resize_int(feed_id, ..self.feeds.len()) {
                    None => return,
                    Some(feed_id) => feed_id,
                };
                std::mem::drop(self.feeds[feed_id].take());
            }
        }
    }

    async fn check_feeds(&mut self) {
        for f in self.feeds.iter_mut().flat_map(|f| f.iter_mut()) {
            let mut expected = VecDeque::new();
            while let Ok(Some(a)) = f.mock_receiver.try_next() {
                expected.push_back(a);
            }
            'next_action: while !expected.is_empty() {
                for _attempt in 0..1000 {
                    match f.app_receiver.try_next() {
                        Err(_) => tokio::task::yield_now().await, // waiting for data
                        Ok(None) => panic!("app receiver closed while still expecting messages!\n---\n{expected:#?}\n---"),
                        Ok(Some(m)) => {
                            match m {
                                Message::Binary(m) => {
                                    let m: FeedMessage = serde_json::from_slice(&m).expect("failed deserializing feed message");
                                    match m {
                                        FeedMessage::Action(a) => {
                                            assert_eq!(a, expected[0], "got unexpected feed message:\n---\n{a:#?}\n---\nExpected messages:\n---\n{expected:#?}\n---");
                                            expected.pop_front();
                                            continue 'next_action;
                                        }
                                        m => panic!("unexpected FeedMessage: {m:?}"),
                                    }
                                }
                                m => panic!("unexpected ws::Message: {m:?}"),
                            }
                        }
                    }
                }
                panic!("did not receive expected message within allocated time. Expected message:\n---\n{:#?}\n---", expected[0]);
            }
            match f.app_receiver.try_next() {
                Ok(Some(m)) => {
                    if let Message::Binary(m) = &m {
                        if let Ok(m) = serde_json::from_slice::<FeedMessage>(m) {
                            panic!("expected no more messages, but got:\n---\n{m:#?}\n---");
                        }
                    }
                    panic!(
                        "expected no more messages, but got impossible-to-parse:\n---\n{m:#?}\n---"
                    );
                }
                _ => (),
            }
        }
    }
}

do_sqlx_test!(
    compare_with_mock,
    bolero::gen_with::<Vec<FuzzOp>>().len(1..100usize),
    |pool, test: Vec<FuzzOp>| async move {
        let mut fuzzer = ComparativeFuzzer::new(pool).await;
        for op in test {
            fuzzer.execute_fuzz_op(op).await;
            fuzzer.check_feeds().await;
        }
    }
);
