use anyhow::Context;
use axum::{
    http::{self, Request, StatusCode},
    middleware::Next,
    response::Response,
    routing::{get, post},
    Extension, Json, Router,
};
use risuto_api::{DbDump, UserId};
use std::net::SocketAddr;

mod db;

#[derive(Clone, Debug)]
struct Auth(Option<CurrentUser>);

#[derive(Clone, Debug)]
struct CurrentUser {
    id: UserId,
}

async fn auth<B: std::fmt::Debug>(
    mut req: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    if let Some(auth) = req.headers().get(http::header::AUTHORIZATION) {
        let auth = auth.to_str().map_err(|_| StatusCode::UNAUTHORIZED)?;

        let db = req
            .extensions()
            .get::<sqlx::PgPool>()
            .expect("No sqlite pool extension");
        if let Some(current_user) = authorize_current_user(db, auth).await {
            req.extensions_mut().insert(Auth(Some(current_user)));
            Ok(next.run(req).await)
        } else {
            Err(StatusCode::UNAUTHORIZED)
        }
    } else {
        req.extensions_mut().insert(Auth(None));
        Ok(next.run(req).await)
    }
}

async fn authorize_current_user(db: &sqlx::PgPool, auth: &str) -> Option<CurrentUser> {
    let split = auth.split(' ').collect::<Vec<_>>();
    if split.len() != 2 || split[0] != "Basic" {
        return None;
    }

    let userpass = base64::decode(split[1]).ok()?;
    let userpass = std::str::from_utf8(&userpass).ok()?;
    let split = userpass.split(':').collect::<Vec<_>>();
    if split.len() != 2 {
        return None;
    }

    let user = sqlx::query!(
        "SELECT id FROM users WHERE name = $1 AND password = $2",
        split[0],
        split[1]
    )
    .fetch_one(db)
    .await
    .ok()?;
    Some(CurrentUser {
        id: UserId(user.id),
    })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let db_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    let db = sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect(&db_url)
        .await
        .with_context(|| format!("Error opening database {:?}", db_url))?;

    let app = Router::new()
        .route("/api/fetch-unarchived", get(fetch_unarchived))
        .route("/api/submit-event", post(submit_event))
        .route_layer(axum::middleware::from_fn(auth))
        .layer(Extension(db));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .context("serving axum webserver")
}

struct AnyhowError(());

impl From<anyhow::Error> for AnyhowError {
    fn from(e: anyhow::Error) -> AnyhowError {
        tracing::error!(err=?e, "got an error");
        AnyhowError(())
    }
}

impl axum::response::IntoResponse for AnyhowError {
    fn into_response(self) -> axum::response::Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal server error, see logs for details",
        )
            .into_response()
    }
}

pub struct PermissionDenied;

impl axum::response::IntoResponse for PermissionDenied {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::FORBIDDEN, "Permission denied").into_response()
    }
}

async fn fetch_unarchived(
    Extension(user): Extension<Auth>,
    Extension(db): Extension<sqlx::PgPool>,
) -> Result<Result<Json<DbDump>, PermissionDenied>, AnyhowError> {
    match user.0 {
        None => Ok(Err(PermissionDenied)),
        Some(user) => {
            let mut conn = db.acquire().await.context("acquiring db connection")?;
            Ok(Ok(Json(
                db::fetch_dump_unarchived(&mut conn, user.id)
                    .await
                    .with_context(|| format!("fetching db dump for {:?}", user))?,
            )))
        }
    }
}

async fn submit_event(
    Json(e): Json<risuto_api::NewEvent>,
    Extension(user): Extension<Auth>,
    Extension(db): Extension<sqlx::PgPool>,
) -> Result<Result<(), PermissionDenied>, AnyhowError> {
    match user.0 {
        None => Ok(Err(PermissionDenied)),
        Some(user) if user.id != e.owner => Ok(Err(PermissionDenied)),
        Some(_) => {
            let mut conn = db.acquire().await.context("acquiring db connection")?;
            // TODO: websocket stuff
            Ok(db::submit_event(&mut conn, e)
                .await
                .context("submitting event to db")?)
        }
    }
}
