#![cfg(test)]

use async_recursion::async_recursion;
use axum::{
    extract::FromRequestParts,
    http::{self, request},
};
use risuto_api::{Error as ApiError, NewSession, NewUser, UserId};
use risuto_mock_server::MockServer;
use sqlx::testing::TestSupport;
use std::{fmt::Debug, panic::AssertUnwindSafe};
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

macro_rules! do_sqlx_test {
        ( $name:ident, $gen:expr, $fn:expr ) => {
            #[test]
            fn $name() {
                if std::env::var("RUST_LOG").is_ok() {
                    tracing_subscriber::fmt::init();
                }
                let runtime = AssertUnwindSafe(
                    tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("failed initializing tokio runtime"),
                );
                // create test db
                let pool = AssertUnwindSafe(runtime.block_on(async move {
                    let test_context = sqlx::Postgres::test_context(&sqlx::testing::TestArgs::new(
                        concat!(module_path!(), "::", stringify!($name)),
                    ))
                    .await
                    .expect("failed connecting to setup test db");
                    let pool = test_context
                        .pool_opts
                        .connect_with(test_context.connect_opts)
                        .await
                        .expect("failed connecting test pool");
                    MIGRATOR
                        .run(&mut pool.acquire().await.expect("getting migrator connection"))
                        .await
                        .expect("failed applying migrations");
                    PgPool::new(pool)
                }));
                let cleanup_queries = include_str!("../reset-test-db.sql")
                    .split(";")
                    .collect::<Vec<_>>();
                let cleanup_queries: &[&str] = &cleanup_queries;
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
                            for q in cleanup_queries {
                                sqlx::query(&q)
                                    .execute(&mut *conn)
                                    .await
                                    .expect("failed cleaning up database");
                            }
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
// TODO: re-enable all
enum FuzzOp {
    CreateUser(NewUser),
    Auth {
        uid: usize,
        #[generator(bolero::generator::gen_with::<String>().len(1..100usize))]
        device: String,
    },
    /* TODO:
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
    CloseActionFeed {
        feed_id: usize,
    },
    */
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
    Err(ApiError::parse(&body)
        .unwrap_or_else(|err| panic!("parsing error response body {err}, body is {body:?}")))
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

struct ComparativeFuzzer {
    admin_token: Uuid,
    app: Router,
    mock: MockServer,
    sessions: Vec<(AuthToken, AuthToken)>,
}

impl ComparativeFuzzer {
    async fn new(pool: PgPool) -> ComparativeFuzzer {
        let admin_token = Uuid::new_v4();
        let app = app(pool, Some(AuthToken(admin_token))).await;
        let mock = MockServer::new();
        let sessions = Vec::new();
        ComparativeFuzzer {
            admin_token,
            app,
            mock,
            sessions,
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
                    self.mock.admin_create_user(new_user, pass),
                )
            }
            FuzzOp::Auth { uid, device } => {
                if let Some((user, password)) = self.mock.test_get_user_info(uid) {
                    let session = NewSession {
                        user: String::from(user),
                        password: String::from(password),
                        device,
                        pow: String::new(),
                    };
                    let app_tok =
                        run_on_app(&mut self.app, "POST", "/api/auth", None, &session).await;
                    let mock_tok = self.mock.auth(session);
                    if let (Ok(app), Ok(mock)) = (&app_tok, &mock_tok) {
                        self.sessions.push((*app, *mock));
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
        }
    }
}

do_sqlx_test!(
    compare_with_mock,
    bolero::generator::gen_with::<Vec<FuzzOp>>().len(1..100usize),
    |pool, test: Vec<FuzzOp>| async move {
        let mut fuzzer = ComparativeFuzzer::new(pool).await;
        for op in test {
            fuzzer.execute_fuzz_op(op).await;
        }
    }
);
