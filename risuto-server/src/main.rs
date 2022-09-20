use anyhow::Context;
use axum::{
    http::{self, StatusCode},
    middleware::Next,
    request::Request,
    response::Response,
    routing::{get, post},
    Json, Router,
};
use diesel::{sqlite::SqliteConnection, Connection};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

mod models;
mod schema;

struct CurrentUser {
    id: usize,
}

async fn auth<B>(mut req: Request<B>, next: Next<B>, Extension(connection): Extension<SqliteConnection>) -> Result<Response, StatusCode> {
    let auth_header = req.headers()
        .get(http::header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    if let Some(current_user) = authorize_current_user(auth_header).await {
        req.extensions_mut().insert(current_user);
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

async fn authorize_current_user(auth_token: &str) -> Option<CurrentUser> {
    let split = auth_token.split(" ");
    if split.len() != 2 || split[0] != "Basic" {
        return None;
    }

    let userpass = base64::decode(split[1])?;
    let split = userpass.split(":");
    if split.len() != 2 {
        return None;
    }

    use diesel::prelude::*;
    use self::schema::users::dsl::*;
    let user = users.filter(name.eq(split[0])).load::<models::User>(connection)?;
    Some(user.id)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let db_file = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    let db = SqliteConnection::establish(&db_file)
        .with_context(|| format!("Error opening database {:?}", db_file))?;

    let app = Router::new().route("/", get(root))
    .route_layer(axum::middleware::from_fn(auth));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .context("serving axum webserver")
}

// basic handler that responds with a static string, but only to auth'd users
async fn root(Extension(current_user): Extension<CurrentUser>) -> &'static str {
    "Hello, World!"
}
