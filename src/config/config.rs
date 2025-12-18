use config;
use config::{ConfigError, File};
use ethers::prelude::U64;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub server: ServerConfig,
    pub ethereum: EthereumConfig,
}

/// PostgreSQL 连接配置（结构化管理）
#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub database_name: String,
    pub username: String,
    pub password: String,
    // 连接池优化参数
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout_seconds: u64,
    pub idle_timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RedisConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub db: i64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct EthereumConfig {
    pub rpc_url: String,
    pub chain_id: u64,
    pub api_keys: String,
    pub init_height: u64,
    pub delay: i16,
    pub max_retries: usize,
    pub base_delay_secs: u64,
}
impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        let environment = std::env::var("APP_ENVIRONMENT").unwrap_or_else(|_| "development".into());

        config::Config::builder()
            .add_source(File::with_name("config/default"))
            .add_source(File::with_name(&format!("config/{}", environment)).required(false))
            .build()?
            .try_deserialize()
    }
}
