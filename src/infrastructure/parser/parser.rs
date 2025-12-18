use crate::errors::error::AppError;
use crate::infrastructure::provider::ProviderTrait;
use crate::models::Transfer;
use crate::utils::is_target_transaction;
use crate::{log_error, log_warn};
use ethers_core::types::{Transaction, U64};
use std::sync::Arc;

pub struct EventParser {
    provider: Arc<dyn ProviderTrait>,
}

impl EventParser {
    pub fn new(provider: Arc<dyn ProviderTrait>) -> Self {
        Self { provider }
    }

    /// 解析单个区块中的目标转账事件
    pub async fn parse_transfers_from_block(
        &self,
        block: &ethers_core::types::Block<Transaction>,
        block_number: i64,
        block_timestamp: i64,
    ) -> Result<(Vec<Transfer>, usize), AppError> {
        let mut transfers = Vec::new();
        let mut skipped_count = 0;

        for tx in &block.transactions {
            if !is_target_transaction(tx) {
                skipped_count += 1;
                continue;
            }

            let receipt = match self.provider.get_transaction_receipt(tx.hash).await {
                Ok(Some(r)) => r,
                Ok(None) => {
                    log_warn!("交易 {:?} 收据未找到，跳过", tx.hash);
                    skipped_count += 1;
                    continue;
                }
                Err(e) => {
                    log_error!("交易 {:?} 获取收据失败（已重试）: {:?}", tx.hash, e);
                    skipped_count += 1;
                    continue;
                }
            };

            if receipt.status != Some(U64::from(1)) {
                log_warn!("交易 {:?} 执行失败 (status=0{:?})，跳过", tx.hash,receipt.status.unwrap_or_default().as_ref());
                skipped_count += 1;
                continue;
            }

            // 这里可以扩展为解析多种事件，目前只解析 Transfer
            let mut tx_transfers = Transfer::process_transaction(
                tx.clone(),
                receipt,
                block_number,
                block_timestamp,
            );

            transfers.append(&mut tx_transfers);
        }
        Ok((transfers, skipped_count))
    }
}
