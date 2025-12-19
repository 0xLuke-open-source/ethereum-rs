use crate::errors::error::AppError;
use crate::models::domain::transfer::Transfer;
use crate::models::schema::eth_transfer::{log_index, tx_hash};
use crate::models::schema::eth_transfer_db;
use crate::models::transfer_db::EthTransferInsert;
use crate::repositories::traits::repository::Repository;
use async_trait::async_trait;
use diesel_async::{AsyncPgConnection, RunQueryDsl};

#[derive(Clone)]
pub struct TransactionRepository {}

impl TransactionRepository {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Repository<Transfer, i64> for TransactionRepository {
    async fn find_by_id(&self, id: i64) -> Result<Option<Transfer>, AppError> {
        todo!()
    }

    async fn save(&self, conn: &mut AsyncPgConnection, entity: &Transfer) -> Result<(), AppError> {
        todo!()
    }

    async fn batch_save(
        &self,
        conn: &mut AsyncPgConnection,
        transfers: &Vec<Transfer>,
    ) -> Result<(), AppError> {
        let diesel_transfers: Vec<EthTransferInsert> = transfers
            .iter()
            .map(|t| t.clone().try_into())
            .collect::<Result<Vec<_>, _>>()?;

        for chunk in diesel_transfers.chunks(1000) {
            diesel::insert_into(eth_transfer_db)
                .values(chunk)
                .on_conflict((tx_hash, log_index))
                .do_nothing()
                .execute(conn)
                .await
                .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        }
        Ok(())
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
