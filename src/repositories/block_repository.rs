use crate::errors::error::AppError;
use crate::models::BlockDomain;
use crate::models::block_db::{BlockInsert, BlockRow};
use crate::models::schema::eth_block::block_number;
use crate::models::schema::eth_block_db;
use crate::repositories::traits::repository::Repository;
use async_trait::async_trait;
use diesel_async::{AsyncPgConnection, RunQueryDsl};

#[derive(Clone)]
pub struct BlockRepository {}

impl BlockRepository {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn get_last_block_number(
        &self,
        conn: &mut AsyncPgConnection,
    ) -> Result<Option<BlockRow>, AppError> {
        use crate::models::schema::eth_block::dsl::*;
        use diesel::{ExpressionMethods, OptionalExtension, QueryDsl};

        eth_block
            .select((block_number, block_hash, parent_hash))
            .order_by(block_number.desc())
            .first::<BlockRow>(conn)
            .await
            .optional()
            .map_err(|e| AppError::DatabaseError(e.to_string()))
    }
}

#[async_trait]
impl Repository<BlockDomain, i64> for BlockRepository {
    async fn find_by_id(&self, id: i64) -> Result<Option<BlockDomain>, AppError> {
        todo!()
    }

    // 之前是同步获取连接，现在改为外部传入异步连接
    async fn save(
        &self,
        conn: &mut AsyncPgConnection,
        block: &BlockDomain,
    ) -> Result<(), AppError> {
        let diesel_block: BlockInsert = block.clone().try_into()?;
        diesel::insert_into(eth_block_db)
            .values(&diesel_block)
            .on_conflict(block_number)
            .do_nothing()
            .execute(conn) // 直接在异步连接上执行
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    async fn batch_save(&self, conn: &mut AsyncPgConnection, entities: &Vec<BlockDomain>) -> Result<(), AppError> {
        todo!()
    }

    async fn delete(&self, id: i64) -> Result<(), AppError> {
        todo!()
    }

    async fn find_all(&self) -> Result<Vec<BlockDomain>, AppError> {
        todo!()
    }

    async fn update(&self, entity: &BlockDomain) -> Result<(), AppError> {
        todo!()
    }

    async fn exists(&self, id: i64) -> Result<bool, AppError> {
        todo!()
    }
}
