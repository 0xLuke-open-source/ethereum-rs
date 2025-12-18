use crate::database::diesel::{DbConnection, DbPool};
use crate::errors::error::{AppError};
use diesel::result::Error as DieselError;

// 可以创建一个通用的仓储基类或工具模块
#[derive(Clone)]
pub struct RepositoryBase {
    pool: DbPool,
}

impl RepositoryBase {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn get_connection(&self) -> Result<DbConnection, AppError> {
        self.pool.get().map_err(AppError::ConnectionPool)
    }


    /// 转换 Diesel 查询错误（统一映射为 Error::DatabaseQuery）
    /// 支持精细化错误分类（如 NotFound 转为业务错误）
    pub fn map_diesel_error(&self, e: DieselError) -> AppError {
        match e {
            DieselError::NotFound => AppError::NotFound("Resource not found in database".to_string()),
            DieselError::DatabaseError(diesel::result::DatabaseErrorKind::UniqueViolation, info) => {
                AppError::Conflict(format!(
                    "Unique constraint violation: table={}, column={}, constraint={}, detail={}",
                    info.table_name().unwrap_or("unknown"),
                    info.column_name().unwrap_or("unknown"),
                    info.constraint_name().unwrap_or("unknown"),
                    info.details().unwrap_or("no detail")
                ))
            }
            DieselError::DatabaseError(kind, info) => {
                AppError::DatabaseQuery(DieselError::DatabaseError(kind, info))
            }
            _ => AppError::DatabaseQuery(e),
        }
    }
}
