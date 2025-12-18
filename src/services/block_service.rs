use crate::config::EthereumConfig;
use crate::errors::error::AppError;
use crate::infrastructure::parser::EventParser;
use crate::infrastructure::provider::ethereum_provider::EthereumProvider;
use crate::infrastructure::provider::{ProviderTrait, RetryAdapter};
use crate::models::BlockDomain;
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
    pub provider: Arc<dyn ProviderTrait>,
    event_parser: Arc<EventParser>,
}

impl BlockService {
    pub fn new(
        block_repository: Arc<BlockRepository>,
        transaction_repository: Arc<TransactionRepository>,
        config: Arc<EthereumConfig>,
    ) -> Self {
        // 1. 创建基础的 provider 池（支持多个 api_key）
        let eth_provider = Arc::new(EthereumProvider::new(&config));
        // 2. 包裹重试适配器（可在这里配置重试次数和初始延迟）
        let provider = Arc::new(RetryAdapter::new(
            eth_provider,
            config.max_retries,
            Duration::from_secs(config.base_delay_secs),
        )) as Arc<dyn ProviderTrait>;
        let event_parser = Arc::new(EventParser::new(provider.clone()));
        Self {
            config,
            provider,
            block_repository,
            transaction_repository,
            event_parser,
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
        block_height: U64,
        block: ethers_core::types::Block<Transaction>,
    ) -> Result<(), AppError> {
        log_info!("当前解析入库区块:{}", block_height);
        let block_domain = BlockDomain::from_ethers(&block)?;
        //保存区块
        self.block_repository.save(&block_domain).await?;

        // 委托给 EventParser 解析事件
        let (transfers, skipped_count) = self
            .event_parser
            .parse_transfers_from_block(&block, block_domain.block_number, block_domain.timestamp)
            .await?;

        // 批量保存转账
        if !transfers.is_empty() {
            self.transaction_repository.batch_save(&transfers).await?;
            log_info!(
                "区块 {} 保存转账 {} 笔，跳过交易 {} 笔",
                block_height,
                transfers.len(),
                skipped_count
            );
        } else {
            log_info!(
                "区块 {} 无转账事件，跳过交易 {} 笔",
                block_height,
                skipped_count
            );
        }
        Ok(())
    }
}
