use crate::database::diesel::DbPool;
use crate::errors::error::AppError;
use crate::models::domain::transfer::Transfer;
use crate::models::schema::eth_transfer::{log_index, tx_hash};
use crate::models::schema::eth_transfer_db;
use crate::models::transfer_db::EthTransferInsert;
use crate::repositories::base::repository_base::RepositoryBase;
use crate::repositories::traits::repository::Repository;
use async_trait::async_trait;
use diesel::{Connection, RunQueryDsl};
use tokio::task;

#[derive(Clone)]
pub struct TransactionRepository {
    base: RepositoryBase,
}

impl TransactionRepository {
    pub fn new(pool: DbPool) -> Self {
        Self {
            base: RepositoryBase::new(pool),
        }
    }
}

#[async_trait]
impl Repository<Transfer, i64> for TransactionRepository {
    async fn find_by_id(&self, id: i64) -> Result<Option<Transfer>, AppError> {
        todo!()
    }

    async fn save(&self, transfer: &Transfer) -> Result<(), AppError> {
        let self_clone = self.clone();
        let transfer_clone = transfer.clone();

        let result = task::spawn_blocking(move || -> Result<(), AppError> {
            let mut conn = self_clone.base.get_connection()?;
            let diesel_transfer: EthTransferInsert = transfer_clone.try_into()?;
            diesel::insert_into(eth_transfer_db)
                .values(&diesel_transfer)
                .execute(&mut conn)?;
            Ok(())
        })
        .await?;
        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }

    async fn batch_save(&self, entities: &Vec<Transfer>) -> Result<(), AppError> {
        if entities.is_empty() {
            return Ok(());
        }
        let self_clone = self.clone();
        let entities_clone = entities.clone();
        // log_info!("当前入库交易:{:?}",entities_clone);

        task::spawn_blocking(move || -> Result<(), AppError> {
            let mut conn = self_clone.base.get_connection()?;
            // 批量转换类型
            let diesel_transfers: Vec<EthTransferInsert> = entities_clone
                .into_iter()
                .map(|t| t.try_into())
                .collect::<Result<Vec<_>, _>>()?;

            //事务
            conn.transaction::<_, AppError, _>(|conn| {
                //分片 执行批量插入,Diesel 自动支持 Vec<&T> 或 Vec<T> 作为 .values() 的参数
                for chunk in diesel_transfers.chunks(1000) {
                    diesel::insert_into(eth_transfer_db)
                        .values(chunk)
                        .on_conflict((tx_hash, log_index))
                        .do_nothing()
                        .execute(conn)
                        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
                }
                Ok(())
            })
        })
        .await?
    }

    async fn delete(&self, id: i64) -> Result<(), AppError> {
        todo!()
    }

    async fn find_all(&self) -> Result<Vec<Transfer>, AppError> {
        todo!()
    }

    async fn update(&self, entity: &Transfer) -> Result<(), AppError> {
        todo!()
    }

    async fn exists(&self, id: i64) -> Result<bool, AppError> {
        todo!()
    }
}
