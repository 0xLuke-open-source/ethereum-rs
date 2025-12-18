use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use crate::config::DatabaseConfig;
use crate::errors::error::AppError;

pub type DbPool = Pool<ConnectionManager<PgConnection>>;
pub type DbConnection = PooledConnection<ConnectionManager<PgConnection>>;

pub fn create_diesel_pool(config: &DatabaseConfig) -> Result<DbPool, AppError> {
    let database_url = format!(
        "postgresql://{}:{}@{}:{}/{}",
        config.username, config.password, config.host, config.port, config.database_name
    );

    let manager = ConnectionManager::<PgConnection>::new(database_url);
    let pool = Pool::builder()
        .max_size(config.max_connections)
        .build(manager)
        .map_err(|e| AppError::ConnectionPool(e))?;

    Ok(pool)
}
