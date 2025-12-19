use crate::errors::error::AppError;
use async_trait::async_trait;
use diesel_async::AsyncPgConnection;

#[async_trait]
pub trait Repository<T, ID>: Send + Sync {
    async fn find_by_id(&self, id: ID) -> Result<Option<T>, AppError>;
    async fn save(&self, conn: &mut AsyncPgConnection, entity: &T) -> Result<(), AppError>;
    async fn batch_save(
        &self,
        conn: &mut AsyncPgConnection,
        entities: &Vec<T>,
    ) -> Result<(), AppError>;
    async fn delete(&self, id: ID) -> Result<(), AppError>;
    async fn find_all(&self) -> Result<Vec<T>, AppError>;
    async fn update(&self, entity: &T) -> Result<(), AppError>;
    async fn exists(&self, id: ID) -> Result<bool, AppError>;
}
