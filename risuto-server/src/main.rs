use anyhow::Context;
use axum::{
    routing::{get, post},
    Router,
};
use risuto_api::{AuthToken, Uuid};
use std::net::SocketAddr;
use tower_http::trace::TraceLayer;

mod db;
mod error;
mod extractors;
mod feeds;
mod fuzz;
mod handlers;
mod query;

use crate::extractors::PgPool;
use crate::feeds::UserFeeds;
use crate::{error::Error, extractors::AppState};

#[derive(Debug, structopt::StructOpt)]
struct Opt {
    /// Enable the admin interface. This will print the admin token to risuto-server's stdout.
    ///
    /// Note that the admin token changes on each server start.
    #[structopt(long)]
    enable_admin: bool,
}

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = <Opt as structopt::StructOpt>::from_args();

    tracing_subscriber::fmt::init();

    let db_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    let db = create_sqlx_pool(&db_url).await?;
    MIGRATOR
        .run(
            &mut *db
                .acquire()
                .await
                .context("acquiring conn for migration running")?,
        )
        .await
        .context("running pending migrations")?;

    let admin_token = match opt.enable_admin {
        false => None,
        true => {
            let t = Uuid::new_v4();
            // Do NOT go through tracing, as it could end up in various metrics collection things
            println!("admin interface enabled; admin token is {t:?}");
            Some(AuthToken(t))
        }
    };

    let app = app(db, admin_token).await;

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .context("serving axum webserver")
}

async fn create_sqlx_pool(db_url: &str) -> anyhow::Result<PgPool> {
    Ok(PgPool::new(
        sqlx::postgres::PgPoolOptions::new()
            .max_connections(8)
            .connect(&db_url)
            .await
            .with_context(|| format!("Error opening database {:?}", db_url))?,
    ))
}

async fn app(db: PgPool, admin_token: Option<AuthToken>) -> Router {
    use handlers::*;

    let feeds = UserFeeds::new();

    let state = AppState {
        db,
        feeds,
        admin_token,
    };

    Router::new()
        .route("/api/admin/create-user", post(admin_create_user))
        .route("/api/auth", post(auth))
        .route("/api/unauth", post(unauth))
        .route("/api/whoami", get(whoami))
        .route("/api/fetch-users", get(fetch_users))
        .route("/api/fetch-tags", get(fetch_tags))
        .route("/api/fetch-searches", get(fetch_searches))
        .route("/api/search-tasks", post(search_tasks))
        .route("/ws/action-feed", get(action_feed))
        .route("/api/submit-action", post(submit_action))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
