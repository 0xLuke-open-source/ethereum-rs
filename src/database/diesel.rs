use crate::config::DatabaseConfig;
use crate::errors::error::AppError;
use diesel_async::AsyncConnection;
use diesel_async::pg::AsyncPgConnection;
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::pooled_connection::bb8::Pool;
use diesel_async::scoped_futures::ScopedFutureExt;
use futures_util::future::BoxFuture;

// 定义异步池类型
pub type AsyncDbPool = Pool<AsyncPgConnection>;

pub async fn create_async_db_pool(config: &DatabaseConfig) -> Result<AsyncDbPool, AppError> {
    let database_url = format!(
        "postgresql://{}:{}@{}:{}/{}",
        config.username, config.password, config.host, config.port, config.database_name
    );

    let manager = AsyncDieselConnectionManager::<AsyncPgConnection>::new(database_url);
    let pool = Pool::builder()
        .max_size(config.max_connections as u32)
        .build(manager)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(pool)
}

#[async_trait::async_trait]
pub trait TransactionExecutor: Send + Sync {
    /// 执行异步事务的闭包接口
    /// 使用自定义的 F 约束，允许闭包返回一个带有生命周期的 Future
    async fn execute_tx<F, T>(&self, f: F) -> Result<T, AppError>
    where
        T: Send,
        F: for<'a> FnOnce(&'a mut AsyncPgConnection) -> BoxFuture<'a, Result<T, AppError>> + Send;
}

pub struct DbService {
    pub pool: AsyncDbPool,
}

#[async_trait::async_trait]
impl TransactionExecutor for DbService {
    async fn execute_tx<F, T>(&self, f: F) -> Result<T, AppError>
    where
        T: Send,
        F: for<'a> FnOnce(&'a mut AsyncPgConnection) -> BoxFuture<'a, Result<T, AppError>> + Send,
    {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        //直接调用 f(c) 并使用 scope_boxed(),确保 conn 的生命周期 'a 与 Future 绑定
        conn.transaction::<T, AppError, _>(|c| f(c).scope_boxed())
            .await
    }
}
