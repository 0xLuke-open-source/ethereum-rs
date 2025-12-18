use crate::database::diesel::DbPool;
use crate::errors::error::AppError;
use crate::models::block_db::{BlockInsert, BlockRow};
use crate::models::schema::eth_block::dsl::eth_block;
use crate::models::schema::eth_block::{block_hash, block_number, parent_hash};
use crate::models::schema::eth_block_db;
use crate::repositories::base::repository_base::RepositoryBase;
use crate::repositories::traits::repository::Repository;
use async_trait::async_trait;
use diesel::{ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl};
use tokio::task;
use crate::models::BlockDomain;

#[derive(Clone)]
pub struct BlockRepository {
    base: RepositoryBase,
}

impl BlockRepository {
    pub fn new(pool: DbPool) -> Self {
        Self {
            base: RepositoryBase::new(pool),
        }
    }
    pub fn get_last_block_number(&self) -> Result<Option<BlockRow>, AppError> {
        let mut conn = self.base.get_connection()?;
        let block = eth_block
            .select((block_number, block_hash, parent_hash))
            .order_by(block_number.desc())
            .first::<BlockRow>(&mut conn)
            .optional()
            .map_err(|e| self.base.map_diesel_error(e))?;
        Ok(block)
    }
}

#[async_trait]
impl Repository<BlockDomain, i64> for BlockRepository {
    async fn find_by_id(&self, id: i64) -> Result<Option<BlockDomain>, AppError> {
        todo!()
    }

    async fn save(&self, block: &BlockDomain) -> Result<(), AppError> {
        let self_clone = self.clone();
        let block_clone = block.clone();
        let result = task::spawn_blocking(move || -> Result<(), AppError> {
            let mut conn = self_clone.base.get_connection()?;
            let diesel_block: BlockInsert = block_clone.try_into()?;
            diesel::insert_into(eth_block_db)
                .values(&diesel_block)
                .on_conflict(block_number)
                .do_nothing()
                .execute(&mut conn)?;
            Ok(())
        })
        .await?;
        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }

    async fn batch_save(&self, entities: &Vec<BlockDomain>) -> Result<(), AppError> {
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
