use diesel::result::Error as DieselError;
use ethers_providers::ProviderError;
use r2d2::Error as R2d2Error;
use redis::RedisError;
use thiserror::Error;
use tokio::task::JoinError;

#[derive(Error, Debug)]
pub enum AppError {
    // 捕获所有 SQL 执行、ORM 映射错误、NotFound 错误等。
    #[error("Database query error: {0}")]
    DatabaseQuery(#[from] DieselError),

    // 处理从连接池获取连接失败的情况（通常包含底层的 ConnectionError）。
    #[error("Database connection pool error: {0}")]
    ConnectionPool(#[from] R2d2Error),

    #[error("Redis error: {0}")]
    Redis(#[from] RedisError),

    #[error("Join error: {0}")]
    JoinError(#[from] JoinError),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Invalid token: {0}")]
    InvalidToken(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("类型转换错误: {0}")]
    ConversionError(String),

    /// 数据库层错误（连接/查询/写入）
    #[error("数据库错误: {0}")]
    DatabaseError(String),

    /// 类型转换错误（i64→BigDecimal、时间转换等）
    #[error("类型转换错误: {0}")]
    Conversion(String),

    /// 业务逻辑冲突（重复插入、状态异常）
    #[error("业务冲突错误: {0}")]
    Conflict(String),

    /// 资源未找到
    #[error("资源未找到: {0}")]
    NotFound(String),

    /// 异步任务错误（spawn_blocking/JoinError）
    #[error("异步任务错误: {0}")]
    Task(String),

    /// 内部不可预期错误（兜底）
    #[error("内部错误: {0}")]
    Internal(String),

    #[error("无效的tx_hash: {0}")]
    InvalidTxHash(String),

    #[error("无效的provider: {0}")]
    ProviderError(String),

    #[error("无效的区块号: {0}")]
    InvalidBlockNumber(String),

    #[error("无效的数字: {0}")]
    InvalidNumber(String),

    #[error("解析错误: {0}")]
    ParserError(String),

    #[error("无效的URL: {0}")]
    InvalidUrl(String),

    #[error("区块链RPC错误: {0}")]
    BlockchainError(String),

    #[error("无效的地址: {0}")]
    InvalidAddress(String),

    #[error("链分叉检测: 在区块 {block} 发现分叉，本地父哈希: {local}, 网络父哈希: {network}")]
    ChainReorg {
        block: u64,
        local: String,
        network: String,
    },
}

impl AppError {

}

#[derive(Error, Debug)]
pub enum SyncError {
    #[error("同步中断: {0}")]
    Interrupted(String),

    #[error("落后太多区块: 本地 {local}, 网络 {network}")]
    TooFarBehind { local: u64, network: u64 },

    #[error("连续失败次数过多: {count}")]
    TooManyFailures { count: u32 },
}

impl AppError {
    pub fn new(message: &str) -> Self {
        AppError::Internal(message.to_string())
    }
}


impl From<ProviderError> for AppError {
    fn from(err: ProviderError) -> Self {
        AppError::ProviderError(err.to_string())
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::Internal(err.to_string())
    }
}
impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Internal(err.to_string())
    }
}
impl From<std::string::FromUtf8Error> for AppError {
    fn from(err: std::string::FromUtf8Error) -> Self {
        AppError::Internal(err.to_string())
    }
}

impl From<std::string::FromUtf16Error> for AppError {
    fn from(err: std::string::FromUtf16Error) -> Self {
        AppError::Internal(err.to_string())
    }
}

impl From<std::num::ParseIntError> for AppError {
    fn from(err: std::num::ParseIntError) -> Self {
        AppError::Internal(err.to_string())
    }
}

impl From<std::num::ParseFloatError> for AppError {
    fn from(err: std::num::ParseFloatError) -> Self {
        AppError::Internal(err.to_string())
    }
}
