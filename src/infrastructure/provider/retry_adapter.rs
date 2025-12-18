use super::ethereum_provider::{EthereumProvider, ProviderTrait};
use crate::errors::error::AppError;
use crate::log_warn;
use async_trait::async_trait;
use ethers::prelude::{U64, U256};
use ethers::providers::ProviderError;
use ethers_core::types::{Address, Block, H256, Transaction, TransactionReceipt};
use ethers_providers::Middleware;
use std::sync::Arc;
use std::time::Duration;
use rand::Rng;
use tokio::time::sleep;

pub struct RetryAdapter {
    provider: Arc<EthereumProvider>,
    max_retries: usize,
    base_delay_secs: Duration,
}

impl RetryAdapter {
    pub fn new(provider: Arc<EthereumProvider>, max_retries: usize, base_delay_secs: Duration) -> Self {
        Self {
            provider,
            max_retries,
            base_delay_secs,
        }
    }

    async fn retry_call<T, Fut, F>(&self, mut f: F) -> Result<T, AppError>
    where
        F: FnMut(Arc<ethers_providers::Provider<ethers_providers::Http>>) -> Fut + Send + Copy,
        Fut: std::future::Future<Output = Result<T, ProviderError>> + Send,
    {
        let mut last_error: Option<ProviderError> = None;
        for attempt in 0..self.max_retries {
            // 延迟逻辑：从第二次尝试 (attempt = 1) 开始执行
            if attempt > 0 {
                // 计算指数倍数，最高限制在 2^10 = 1024
                let exponent = (attempt - 1).min(10);
                let base_ms = self.base_delay_secs.as_millis() as u64;

                // 计算基础延迟时间：base * 2^n
                let delay_ms = base_ms * (1u64 << exponent);

                // 生成 0~10% 的随机抖动 (Jitter)
                // 这样可以防止多个重试任务在同一时间点“齐射” RPC 节点
                let jitter = rand::thread_rng().gen_range(0..=(delay_ms / 10 + 1));

                let final_delay = Duration::from_millis(delay_ms + jitter);

                log_warn!(
                    "RPC 尝试失败，正在进行第 {} 次重试，等待 {:?}...",
                    attempt + 1,
                    final_delay
                );

                sleep(final_delay).await;
            }
            let p = self.provider.get_provider();
            match f(p).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = Some(e);
                    log_warn!("RPC 调用失败 (第 {} 次): {:?}", attempt + 1, last_error);
                }
            }
        }
        Err(AppError::ProviderError(format!(
            "重试 {} 次失败，最后错误: {:?}",
            self.max_retries, last_error
        )))
    }
}

#[async_trait]
impl ProviderTrait for RetryAdapter {
    async fn get_last_block_number(&self) -> Result<U64, AppError> {
        self.retry_call(|p| async move { p.get_block_number().await })
            .await
    }

    async fn get_block_with_txs(
        &self,
        number: u64,
    ) -> Result<Option<Block<Transaction>>, AppError> {
        let number = number;
        self.retry_call(move |p| async move { p.get_block_with_txs(number).await })
            .await
    }

    async fn get_transaction_receipt(
        &self,
        tx_hash: H256,
    ) -> Result<Option<TransactionReceipt>, AppError> {
        let tx_hash = tx_hash;
        self.retry_call(move |p| async move { p.get_transaction_receipt(tx_hash).await })
            .await
    }

    async fn get_chain_id(&self) -> Result<U256, AppError> {
        self.retry_call(|p| async move { p.get_chainid().await })
            .await
    }

    async fn get_transaction_count(&self, address: &str) -> Result<U256, AppError> {
        let addr = address
            .parse::<Address>()
            .map_err(|_| AppError::InvalidAddress(address.to_string()))?;

        self.retry_call(move |p| async move { p.get_transaction_count(addr, None).await })
            .await
    }
}
