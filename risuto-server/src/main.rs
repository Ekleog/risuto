use anyhow::Context;
use axum::{
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use diesel::{sqlite::SqliteConnection, Connection};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let db_file = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    let db = SqliteConnection::establish(&db_file)
        .with_context(|| format!("Error opening database {:?}", db_file))?;

    let app = Router::new().route("/", get(root));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .context("serving axum webserver")
}

// basic handler that responds with a static string
async fn root() -> &'static str {
    "Hello, World!"
}
