use crate::config::RedisConfig;
use crate::errors::error::AppError;
use redis::{Client as RedisClient, aio::ConnectionManager}; // 用 ConnectionManager 替代直接连接

/// 初始化 Redis 异步连接管理器（支持自动重连）
pub async fn create_redis_pool(config: &RedisConfig) -> Result<ConnectionManager, AppError> {
    // 1. 构造 Redis 连接 URL
    let redis_url = if config.username.is_empty() {
        format!(
            "redis://:{}@{}:{}/{}",
            config.password, config.host, config.port, config.db
        )
    } else {
        format!(
            "redis://{}:{}@{}:{}/{}",
            config.username, config.password, config.host, config.port, config.db
        )
    };

    // 2. 创建 Redis 客户端
    let client = RedisClient::open(redis_url)
        .map_err(|e| AppError::Redis(e))?;

    // 3. 创建异步连接管理器（自动重连，替代 get_async_connection）
    let manager = ConnectionManager::new(client)
        .await // 异步初始化
        .map_err(|e| AppError::Redis(e))?;

    // 4. 验证连接（通过 ConnectionManager 执行 PING）
    let mut conn = manager.clone(); // 从管理器获取连接
    let pong: String = redis::cmd("PING")
        .query_async(&mut conn)
        .await
        .map_err(|e| AppError::Redis(e))?;

    if pong == "PONG" {
        tracing::info!("✅ Redis ConnectionManager initialized successfully");
    } else {
        return Err(AppError::Validation("Redis PING response is not PONG".to_string()));
    }

    Ok(manager)
}