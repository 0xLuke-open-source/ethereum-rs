use async_trait::async_trait;
use crate::errors::error::AppError;

#[async_trait]
pub trait Repository<T, ID>: Send + Sync {
    async fn find_by_id(&self, id: ID) -> Result<Option<T>, AppError>;
    async fn save(&self, entity: &T) -> Result<(), AppError>;
    async fn batch_save(&self, entities: &Vec<T>) -> Result<(), AppError>;
    async fn delete(&self, id: ID) -> Result<(), AppError>;
    async fn find_all(&self) -> Result<Vec<T>, AppError>;
    async fn update(&self, entity: &T) -> Result<(), AppError>;
    async fn exists(&self, id: ID) -> Result<bool, AppError>;
}