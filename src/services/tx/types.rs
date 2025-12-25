// services/tx/types.rs

use ethers_core::types::{Bytes, H160, H256, TransactionReceipt, U256};
use serde::{Deserialize, Serialize};
use crate::services::tx::gas::gas_strategy::TxPriority;

#[derive(Debug, Clone)]
pub struct TxOptions {
    pub priority: TxPriority,
    pub gas_limit_buffer: u64,     // 百分比，例如 120 表示 +20%
    pub confirmations: u64,        // 所需确认数
    pub timeout_secs: u64,         // 等待超时秒数
}

impl Default for TxOptions {
    fn default() -> Self {
        Self {
            priority: TxPriority::Normal,
            gas_limit_buffer: 120,
            confirmations: 1,
            timeout_secs: 300,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TxContext {
    pub to: H160,
    pub value: U256,
    pub data: Bytes,
    pub options: TxOptions,
}

#[derive(Debug, Clone)]
pub struct TxResult {
    pub tx_hash: H256,
    pub receipt: TransactionReceipt,
}