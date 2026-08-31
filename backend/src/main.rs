#![allow(dead_code)]

mod app;
mod application;
mod config;
mod domain;
mod http;
mod infrastructure;
mod modules;
mod state;

use crate::infrastructure::db::connect_database;
use crate::{config::Config, state::AppState};
use anyhow::Context;
use projecty_migration::{Migrator, MigratorTrait};
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "projecty_api=debug,tower_http=info,axum::rejection=trace".into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env()?;
    let db = connect_database(&config.database_url)
        .await
        .context("failed to connect postgres database")?;
    Migrator::up(&db, None)
        .await
        .context("failed to apply pending database migrations")?;
    let bind_addr = config.bind_addr()?;
    let listener = TcpListener::bind(bind_addr)
        .await
        .context("failed to bind api listener")?;

    tracing::info!(addr = %bind_addr, "Projecty API listening");
    axum::serve(listener, app::build_router(AppState::new(config, db)))
        .await
        .context("api server failed")?;
    Ok(())
}
