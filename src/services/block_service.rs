use crate::config::EthereumConfig;
use crate::errors::error::AppError;
use crate::infrastructure::provider::ethereum_provider::EthereumProvider;
use crate::infrastructure::provider::{ProviderTrait, RetryAdapter};
use crate::models::domain::Block;
use crate::models::domain::block::BlockQuery;
use crate::models::domain::transfer::Transfer;
use crate::repositories::block_repository::BlockRepository;
use crate::repositories::traits::repository::Repository;
use crate::repositories::transaction_repository::TransactionRepository;
use crate::utils::{
    h256_opt_to_string, h256_to_string, is_target_transaction, opt_u256_to_i64_loose,
    option_u64_to_i64, u256_to_i64,
};
use crate::{log_error, log_info, log_warn};
use anyhow::Context;
use ethers::prelude::U64;
use ethers_core::types::Transaction;
use std::sync::Arc;
use std::time::Duration;

pub struct BlockService {
    pub config: Arc<EthereumConfig>,
    pub block_repository: Arc<BlockRepository>,
    pub transaction_repository: Arc<TransactionRepository>,
    // 使用 trait object，支持多态（可以是 EthereumProvider 或 RetryAdapter）
    pub provider: Arc<dyn ProviderTrait>,
}

impl BlockService {
    pub fn new(
        block_repository: Arc<BlockRepository>,
        transaction_repository: Arc<TransactionRepository>,
        provider: Arc<EthereumProvider>,
        config: Arc<EthereumConfig>,
    ) -> Self {
        // 1. 创建基础的 provider 池（支持多个 api_key）
        let eth_provider = Arc::new(EthereumProvider::new(&config));
        // 2. 包裹重试适配器（可在这里配置重试次数和初始延迟）
        let retry_adapter = Arc::new(RetryAdapter::new(
            eth_provider,
            config.max_retries, // 最大重试次数
            Duration::from_secs(config.base_delay_secs), // 初始延迟 2s，之后指数增长：2s → 4s → 8s → 16s
        )) as Arc<dyn ProviderTrait>;
        Self {
            block_repository,
            transaction_repository,
            provider: retry_adapter,
            config,
        }
    }

    pub async fn sync_blocks(&self) -> anyhow::Result<()> {
        // 获取网络最新高度（已自动带重试）
        let current_net_block = self
            .provider
            .get_last_block_number()
            .await
            .context("获取链上最新区块号失败")?;

        // 安全高度（延迟确认数）
        let max_safe_block = current_net_block.saturating_sub(self.config.delay.into());

        let mut local_block = self
            .block_repository
            .get_last_block_number()?
            .map(BlockQuery::try_from)
            .transpose()?;

        let mut next_block = match local_block.as_ref() {
            None => U64::from(self.config.init_height),
            Some(b) => b.block_number + 1,
        };

        //如果本地高度大于等于安全高度则跳过
        if next_block > max_safe_block {
            log_info!(
                "等待新区块... start={}, safe={}",
                next_block,
                max_safe_block
            );
            return Ok(());
        }

        log_info!("开始同步区块: {} → {}", next_block, max_safe_block);

        //本地高度小于或等于安全目标时，继续同步
        //主同步循环
        while next_block <= max_safe_block {
            let block_number = next_block.as_u64();

            // 这里不再手动处理 None 和 Err —— 重试逻辑已由 RetryAdapter 接管
            // 如果最终仍失败，会直接返回 AppError，被外层捕获
            let block_data = match self.provider.get_block_with_txs(block_number).await {
                Ok(Some(block)) => block, // 成功获取区块
                Ok(None) => {
                    // 理论上不应该出现（链上连续），但仍记录并短暂等待
                    log_warn!(
                        "区块 {} 暂未同步到节点，等待后重试（由 RetryAdapter 控制）",
                        block_number
                    );
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    continue;
                }
                Err(e) => {
                    // 严重错误：网络或节点问题，RetryAdapter 已尽力重试
                    log_error!("获取区块 {} 最终失败: {:?}", block_number, e);
                    // 可选择继续等待或直接中断同步
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    continue;
                }
            };
            //父 hash 校验（只要本地有块就校验）
            if let Some(prev) = local_block.as_ref() {
                if block_data.parent_hash != prev.block_hash {
                    log_warn!(
                        "链分叉检测到！区块 {} 本地父哈希 {} ≠ 链上父哈希 {}",
                        block_number,
                        prev.block_hash,
                        block_data.parent_hash
                    );

                    //这里先用延迟解析的方式来简单解决分叉的问题--后续加回滚块、交易来处理
                    return Err(anyhow::anyhow!(
                        "Chain re-org detected at block {}",
                        block_number
                    ));
                }
            }
            self.process_and_save_block(U64::from(block_number), block_data.clone())
                .await
                .with_context(|| format!("处理区块 {} 失败", block_number))?;

            let block_hash = block_data
                .hash
                .ok_or_else(|| anyhow::anyhow!("block {} missing hash", block_number))?;

            //推进本地状态
            local_block = Some(BlockQuery {
                block_number: next_block,
                block_hash,
            });
            next_block += U64::from(1);
        }
        log_info!("区块同步完成，当前安全高度 {}", max_safe_block);
        Ok(())
    }

    async fn process_and_save_block(
        &self,
        current_block: U64,
        block: ethers_core::types::Block<Transaction>,
    ) -> Result<(), AppError> {
        log_info!("当前解析入库区块:{}", current_block);

        let block_number = option_u64_to_i64(block.number)?;
        let block_hash = h256_opt_to_string(block.hash);
        let block_parent_hash = h256_to_string(block.parent_hash);
        let gas_used = u256_to_i64(block.gas_used)?;
        let base_fee_per_gas = opt_u256_to_i64_loose(block.base_fee_per_gas)?;
        let block_timestamp = u256_to_i64(block.timestamp)?;
        let size: i32 = block
            .transactions
            .len()
            .try_into()
            .map_err(|_| AppError::InvalidNumber("transactions count overflow".into()))?;

        let new_block = Block::new(
            block_number,
            block_hash,
            block_parent_hash,
            gas_used as f64,
            base_fee_per_gas as f64,
            block_timestamp,
            size,
        );

        //保存区块
        self.block_repository.save(&new_block).await?;

        let mut processed_transfers = vec![];
        let mut skipped_tx_count = 0;

        for tx in block.transactions {
            let tx_hash = format!("{:#x}", tx.hash);
            if !is_target_transaction(&tx) {
                skipped_tx_count += 1;
                // 忽略其他合约交互、L2 交易等
                // log_info!("不是目标交易,跳过 {:?}", tx_hash);
                continue;
            }
            // log_info!("TX_FOUND: Hash={:#x}, From={:#x}, To={:?}",  tx.hash, tx.from, tx.to);
            // 获取交易收据（已自动带重试）
            let receipt = match self.provider.get_transaction_receipt(tx.hash).await {
                Ok(Some(r)) => r,
                Ok(None) => {
                    log_warn!("交易 {} 收据不存在，跳过", tx_hash);
                    skipped_tx_count += 1;
                    continue;
                }
                Err(e) => {
                    log_error!("交易 {} 获取收据失败: {:?}, 跳过", tx_hash, e);
                    skipped_tx_count += 1;
                    continue;
                }
            };

            if receipt.status != Some(U64::from(1)) {
                log_warn!("交易 {} 执行失败（status=0），跳过", tx_hash);
                skipped_tx_count += 1;
                continue;
            }

            // 解析交易
            let transfers =
                Transfer::process_transaction(tx, receipt, block_number, block_timestamp).await;
            processed_transfers.extend(transfers);
        }

        // 批量保存
        if !processed_transfers.is_empty() {
            self.transaction_repository
                .batch_save(&processed_transfers)
                .await?;
        }

        log_info!(
            "区块 {} 处理完成, 解析交易数: {}, 跳过交易数: {}",
            block_number,
            processed_transfers.len(),
            skipped_tx_count
        );
        Ok(())
    }
}
