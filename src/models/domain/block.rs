use crate::errors::error::AppError;
use crate::models::block_db::BlockRow;
use ethers::prelude::U64;
use ethers_core::types::{H256, Transaction};

#[derive(Debug, Clone)]
pub struct BlockDomain {
    pub block_number: i64,
    pub block_hash: String,
    pub parent_hash: String,
    pub gas_used: f64,
    pub base_fee_per_gas: f64,
    pub timestamp: i64,
    pub size: i32,
}

#[derive(Debug, Clone)]
pub struct BlockQuery {
    pub block_number: U64,
    pub block_hash: H256,
}

impl TryFrom<BlockRow> for BlockQuery {
    type Error = AppError;

    fn try_from(db: BlockRow) -> Result<Self, Self::Error> {
        let block_hash = db.block_hash.parse::<H256>().map_err(|e| {
            AppError::Conversion(format!("Invalid block_hash {}: {}", db.block_hash, e))
        })?;
        let block_number = U64::from(db.block_number);
        Ok(Self {
            block_number,
            block_hash,
        })
    }
}

impl BlockDomain {
    pub fn new(
        block_number: i64,
        block_hash: String,
        parent_hash: String,
        gas_used: f64,
        base_fee_per_gas: f64,
        timestamp: i64,
        size: i32,
    ) -> Self {
        Self {
            block_number,
            block_hash,
            parent_hash,
            gas_used,
            base_fee_per_gas,
            timestamp,
            size,
        }
    }

    pub fn from_ethers(block: &ethers_core::types::Block<Transaction>) -> Result<Self, AppError> {
        let block_number = crate::utils::option_u64_to_i64(block.number)?;
        let block_hash = crate::utils::h256_opt_to_string(block.hash);
        let block_parent_hash = crate::utils::h256_to_string(block.parent_hash);
        let gas_used = crate::utils::u256_to_i64(block.gas_used)? as f64;
        let base_fee_per_gas = crate::utils::opt_u256_to_i64_loose(block.base_fee_per_gas)? as f64;
        let block_timestamp = crate::utils::u256_to_i64(block.timestamp)?;
        let size: i32 = block
            .transactions
            .len()
            .try_into()
            .map_err(|_| AppError::InvalidNumber("transactions count overflow".into()))?;

        Ok(Self::new(
            block_number,
            block_hash,
            block_parent_hash,
            gas_used,
            base_fee_per_gas,
            block_timestamp,
            size,
        ))
    }

    pub fn is_empty(&self) -> bool {
        self.block_number == 0 && self.block_hash.is_empty() && self.parent_hash.is_empty()
    }
}
