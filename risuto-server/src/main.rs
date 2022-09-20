use anyhow::Context;
use axum::{
    http::{self, Request, StatusCode},
    middleware::Next,
    response::Response,
    routing::get,
    Extension, Router,
};
use std::net::SocketAddr;

#[derive(sqlx::FromRow, serde::Deserialize, serde::Serialize)]
pub struct User {
    pub id: usize,
    pub name: String,
    pub password: String,
}

#[derive(Clone, Debug)]
struct Auth(Option<CurrentUser>);

#[derive(Clone, Debug)]
struct CurrentUser {
    id: usize,
}

async fn auth<B: std::fmt::Debug>(mut req: Request<B>, next: Next<B>) -> Result<Response, StatusCode> {
    if let Some(auth) = req.headers().get(http::header::AUTHORIZATION) {
        let auth = auth.to_str().map_err(|_| StatusCode::UNAUTHORIZED)?;

        let db = req.extensions().get::<sqlx::SqlitePool>().expect("No sqlite pool extension");
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

async fn authorize_current_user(db: &sqlx::SqlitePool, auth: &str) -> Option<CurrentUser> {
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

    Some(CurrentUser { id: 42 })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let db_file = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    let db = sqlx::sqlite::SqlitePoolOptions::new()
        .connect(&db_file)
        .await
        .with_context(|| format!("Error opening database {:?}", db_file))?;

    let app = Router::new()
        .route("/", get(root))
        .route_layer(axum::middleware::from_fn(auth))
        .layer(Extension(db));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .context("serving axum webserver")
}

// basic handler that responds with a static string, but only to auth'd users
async fn root(Extension(user): Extension<Auth>) -> String {
    format!("Hello user {:?}", user)
}
