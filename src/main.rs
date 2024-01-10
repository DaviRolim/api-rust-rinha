mod config;
mod controller;
mod error;

pub use self::error::{Error, Result};
use axum::{routing::get, Router};
pub use config::config;

use self::controller::*;
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    db: Pool<Postgres>,
}

#[tokio::main]
async fn main() {
    let pool = PgPoolOptions::new()
        .min_connections(5)
        .max_connections(100)
        .connect(&config().database_url)
        .await
        .unwrap_or_else(|ex| panic!("Failed to connect to Postgres: {ex:?}"));

    let state = Arc::new(AppState { db: pool });

    let app = Router::new()
        .route("/pessoas/:id", get(get_user))
        .route(
            "/pessoas",
            get(get_pessoas_by_search_term).post(create_user),
        )
        .with_state(state.clone());
    // Start Server
    let app_port = &config().app_port;
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{app_port}"))
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}
