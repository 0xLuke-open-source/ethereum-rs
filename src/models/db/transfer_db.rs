use crate::models::Transfer;
use crate::models::db::schema::eth_transfer;
use bigdecimal::BigDecimal;
use diesel::Insertable;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Insertable)]
#[diesel(table_name = eth_transfer)]
pub struct EthTransferInsert {
    pub block_number: i64,
    pub tx_hash: String,
    pub from_address: String,
    pub to_address: String,
    pub amount: BigDecimal,
    pub contract_address: Option<String>,
    pub timestamp: i64,
    pub gas: BigDecimal,
    pub max_fee_per_gas: BigDecimal,
    pub status: i16,
    pub log_index: i64,
}

impl TryFrom<Transfer> for EthTransferInsert {
    type Error = anyhow::Error;

    fn try_from(transfer: Transfer) -> Result<EthTransferInsert, Self::Error> {
        Ok(EthTransferInsert {
            block_number: transfer.block_number,
            tx_hash: transfer.tx_hash,
            from_address: transfer.from_address,
            to_address: transfer.to_address,
            amount: transfer.amount,
            contract_address: transfer.contract_address,
            timestamp: transfer.timestamp,
            gas: transfer.gas,
            max_fee_per_gas: transfer.max_fee_per_gas,
            status: transfer.status,
            log_index: transfer.log_index,
        })
    }
}
