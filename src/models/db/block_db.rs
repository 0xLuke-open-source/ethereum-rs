use crate::errors::error::AppError;
use crate::models::db::schema::eth_block;
use crate::models::domain::block::Block;
use bigdecimal::{BigDecimal, FromPrimitive};
use diesel::{Insertable, Queryable};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Insertable)]
#[diesel(table_name = eth_block)]
pub struct BlockInsert {
    pub block_number: i64,                 // BigInt -> i64 ✓
    pub block_hash: String,                // Varchar -> String ✓
    pub parent_hash: String,               // Varchar -> String ✓
    pub gas_used: BigDecimal,              // Numeric(78,0) -> BigDecimal ✨
    pub base_fee_per_gas: BigDecimal,      // Numeric(78,0) -> BigDecimal ✨
    pub timestamp: i64,                    // BigInt -> i64 ✓
    pub size: i32,                 // Int4 -> i32 ✓
}

#[derive(Debug, Clone, Serialize, Deserialize, Queryable)]
#[diesel(table_name = eth_block)]
pub struct BlockRow {
    pub block_number: i64,
    pub block_hash: String,
    pub parent_hash: String,
}

impl TryFrom<Block> for BlockInsert {
    type Error = AppError;

    fn try_from(block: Block) -> Result<BlockInsert, Self::Error> {
        let gas_used = BigDecimal::from_i64(block.gas_used as i64).ok_or_else(|| {
            AppError::Conversion(format!(
                "区块 {}: gas_used ({}) 转换为 BigDecimal 失败",
                block.block_number, block.gas_used
            ))
        })?;

        let base_fee_per_gas = BigDecimal::from_i64(block.base_fee_per_gas as i64).ok_or_else(|| {
            AppError::Conversion(format!(
                "区块 {}: base_fee_per_gas ({}) 转换为 BigDecimal 失败",
                block.block_number, block.base_fee_per_gas
            ))
        })?;

        Ok(Self {
            block_number: block.block_number,
            block_hash: block.block_hash,
            parent_hash: block.parent_hash,
            gas_used,
            base_fee_per_gas,
            timestamp: block.timestamp,
            size: block.size,
        })
    }
}
