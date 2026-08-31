#![allow(dead_code)]

mod app;
mod application;
mod config;
mod domain;
mod http;
mod infrastructure;
mod modules;
mod state;

use clap::{Parser, Subcommand};
use projecty_migration::{Migrator, MigratorTrait};

use crate::infrastructure::db::connect_database;
use crate::{config::Config, state::AppState};
use anyhow::Context;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, Parser)]
#[command(name = "projecty-api", version, about = "Projecty API 服务与运维命令")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// 账号运维命令(可在容器内通过 docker exec 执行)
    Admin {
        #[command(subcommand)]
        action: AdminAction,
    },
}

#[derive(Debug, Subcommand)]
enum AdminAction {
    /// 创建超级管理员账号,账号已存在时报错退出
    Create {
        /// 登录账号,2-64 个字符
        #[arg(long)]
        account: String,
        /// 初始密码,8-128 个字符
        #[arg(long)]
        password: String,
        /// 显示名称
        #[arg(long, default_value = "超级管理员")]
        display_name: String,
        /// 数据库连接串,缺省读取 DATABASE_URL 环境变量
        #[arg(long)]
        database_url: Option<String>,
    },
}

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

    match Cli::parse().command {
        None => serve().await,
        Some(Command::Admin { action }) => admin(action).await,
    }
}

async fn serve() -> anyhow::Result<()> {
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

async fn admin(action: AdminAction) -> anyhow::Result<()> {
    let AdminAction::Create {
        account,
        password,
        display_name,
        database_url,
    } = action;
    let database_url = database_url
        .or_else(|| std::env::var("DATABASE_URL").ok())
        .context("需要通过 --database-url 或 DATABASE_URL 提供数据库连接串")?;
    let db = connect_database(&database_url)
        .await
        .context("failed to connect postgres database")?;
    Migrator::up(&db, None)
        .await
        .context("failed to apply pending database migrations")?;

    let user = modules::users::service::create_user(
        &db,
        None,
        modules::users::service::NewUser {
            account,
            password,
            display_name,
            system_role: domain::permissions::SystemRole::SuperAdmin,
            department_ids: vec![],
        },
    )
    .await
    .map_err(|error| anyhow::anyhow!("创建超级管理员失败:{error}"))?;
    println!("超级管理员已创建:账号 {},显示名 {}", user.account, user.display_name);
    Ok(())
}
