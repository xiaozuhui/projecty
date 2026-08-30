use anyhow::{Context, Result};
use std::{env, net::SocketAddr};

#[derive(Debug, Clone)]
pub struct Config {
    pub environment: String,
    pub bind_host: String,
    pub bind_port: u16,
    pub database_url: String,
    pub jwt_secret: String,
    pub jwt_issuer: String,
    pub access_token_ttl_seconds: u64,
    pub refresh_token_ttl_seconds: u64,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            environment: env::var("PROJECTY_ENV").unwrap_or_else(|_| "development".to_owned()),
            bind_host: env::var("PROJECTY_BIND_HOST").unwrap_or_else(|_| "127.0.0.1".to_owned()),
            bind_port: env::var("PROJECTY_BIND_PORT")
                .unwrap_or_else(|_| "8080".to_owned())
                .parse()
                .context("PROJECTY_BIND_PORT must be a valid u16")?,
            database_url: env::var("DATABASE_URL").context("DATABASE_URL is required")?,
            jwt_secret: env::var("JWT_SECRET").context("JWT_SECRET is required")?,
            jwt_issuer: env::var("JWT_ISSUER").unwrap_or_else(|_| "projecty".to_owned()),
            access_token_ttl_seconds: env::var("ACCESS_TOKEN_TTL_SECONDS")
                .unwrap_or_else(|_| "1800".to_owned())
                .parse()
                .context("ACCESS_TOKEN_TTL_SECONDS must be a valid integer")?,
            refresh_token_ttl_seconds: env::var("REFRESH_TOKEN_TTL_SECONDS")
                .unwrap_or_else(|_| "1209600".to_owned())
                .parse()
                .context("REFRESH_TOKEN_TTL_SECONDS must be a valid integer")?,
        })
    }
    pub fn bind_addr(&self) -> Result<SocketAddr> {
        format!("{}:{}", self.bind_host, self.bind_port)
            .parse()
            .context("PROJECTY_BIND_HOST and PROJECTY_BIND_PORT must form a valid socket address")
    }
}
