use crate::errors::error::AppError;
use crate::models::block_db::BlockRow;
use ethers::prelude::U64;
use ethers_core::types::H256;

#[derive(Debug, Clone)]
pub struct Block {
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

impl Block {
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

    pub fn is_empty(&self) -> bool {
        self.block_number == 0 && self.block_hash.is_empty() && self.parent_hash.is_empty()
    }
}
