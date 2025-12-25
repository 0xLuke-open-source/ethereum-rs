use super::ethereum_provider::{EthereumProvider, ProviderTrait};
use crate::errors::error::AppError;
use crate::{log_info, log_warn};
use async_trait::async_trait;
use ethers::prelude::{U64, U256};
use ethers::providers::ProviderError;
use ethers_core::types::transaction::eip2718::TypedTransaction;
use ethers_core::types::{Address, Block, Bytes, H256, Transaction, TransactionReceipt};
use ethers_providers::{Http, Middleware, PendingTransaction};
use rand::Rng;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

pub struct RetryAdapter {
    provider: Arc<EthereumProvider>,
    max_retries: usize,
    base_delay_secs: Duration,
}

impl RetryAdapter {
    pub fn new(
        provider: Arc<EthereumProvider>,
        max_retries: usize,
        base_delay_secs: Duration,
    ) -> Self {
        Self {
            provider,
            max_retries,
            base_delay_secs,
        }
    }

    async fn retry_call<T, Fut, F>(&self, mut f: F) -> Result<T, AppError>
    where
        F: FnMut(Arc<ethers_providers::Provider<ethers_providers::Http>>) -> Fut + Send,
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

    async fn estimate_eip1559_fees(
        &self,
        estimator: Option<fn(U256, Vec<Vec<U256>>) -> (U256, U256)>,
    ) -> Result<(U256, U256), AppError> {
        let estimator = estimator;
        self.retry_call(move |p| async move { p.estimate_eip1559_fees(estimator).await })
            .await
    }

    async fn send_raw_transaction(
        &self,
        rlp: Bytes,
        timeout_secs: u64,
        confirmations: usize,
    ) -> Result<TransactionReceipt, AppError> {
        // 1. 调用 retry_call，内部只处理网络/节点层的重试
        let receipt = self
            .retry_call(move |p| {
                let rlp = rlp.clone();
                async move {
                    // 1. 发送交易
                    let pending_tx = p.send_raw_transaction(rlp).await?;

                    // 2. 等待确认 (将等待逻辑也放入重试闭包内)
                    // 注意：如果等待超时，也会触发重试
                    let wait_res = tokio::time::timeout(
                        Duration::from_secs(timeout_secs),
                        pending_tx.confirmations(confirmations),
                    )
                    .await;
                    // 处理超时和结果，并统一转为 ProviderError 以便触发重试
                    match wait_res {
                        Ok(Ok(Some(r))) => Ok(r),
                        Ok(Ok(None)) => {
                            Err(ProviderError::CustomError("Dropped from mempool".into()))
                        }
                        Ok(Err(e)) => Err(e), // Provider 级错误
                        Err(_) => Err(ProviderError::CustomError("Timeout".into())),
                    }
                }
            })
            .await?;
        //2. 拿到回执后，在重试逻辑外检查业务状态 (Status)
        // 这样如果 Revert，会直接返回给上层，而不会在 RetryAdapter 里盲目重试
        if receipt.status == Some(0.into()) {
            return Err(AppError::Internal(format!(
                "Transaction reverted! Hash: {:?}",
                receipt.transaction_hash
            )));
        }
        log_info!(
            "交易执行成功: hash={:?}, block={:?}",
            receipt.transaction_hash,
            receipt.block_number
        );
        Ok(receipt)
    }

    async fn call(&self, tx: &TypedTransaction) -> Result<Bytes, AppError> {
        self.retry_call(move |p| async move {
            let tx = tx.clone();
            p.call(&tx, None).await
        })
        .await
    }

    async fn estimate_gas(&self, tx: &TypedTransaction) -> Result<U256, AppError> {
        self.retry_call(move |p| async move {
            let tx = tx.clone();
            p.estimate_gas(&tx, None).await
        })
        .await
    }
}
