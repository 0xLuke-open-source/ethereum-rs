use crate::config::EthereumConfig;
use crate::errors::error::AppError;
use crate::log_info;
use async_trait::async_trait;
use ethers::addressbook::Address;
use ethers::prelude::{H256, U64, U256};
use ethers_core::types::transaction::eip2718::TypedTransaction;
use ethers_core::types::{Block, Bytes, Transaction, TransactionReceipt};
use ethers_providers::{Http, Middleware, PendingTransaction, Provider, ProviderError};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::time::timeout;
use url::Url;

#[async_trait]
pub trait ProviderTrait: Send + Sync {
    async fn get_last_block_number(&self) -> Result<U64, AppError>;
    async fn get_block_with_txs(&self, number: u64)
    -> Result<Option<Block<Transaction>>, AppError>;
    async fn get_transaction_receipt(
        &self,
        tx_hash: H256,
    ) -> Result<Option<TransactionReceipt>, AppError>;
    async fn get_chain_id(&self) -> Result<U256, AppError>;
    async fn get_transaction_count(&self, address: &str) -> Result<U256, AppError>;

    async fn estimate_eip1559_fees(
        &self,
        estimator: Option<fn(U256, Vec<Vec<U256>>) -> (U256, U256)>,
    ) -> Result<(U256, U256), AppError>;
    async fn send_raw_transaction(
        &self,
        rlp: Bytes,
        timeout_secs: u64,
        confirmations: usize,
    ) -> Result<TransactionReceipt, AppError>;
    async fn call(&self, tx: &TypedTransaction) -> Result<Bytes, AppError>;
    async fn estimate_gas(&self, tx: &TypedTransaction) -> Result<U256, AppError>;
}

pub struct EthereumProvider {
    providers: Vec<Arc<Provider<Http>>>,
    index: AtomicUsize,
}

impl EthereumProvider {
    pub fn new(config: &EthereumConfig) -> Self {
        let providers = config
            .api_keys
            .split(',')
            .map(|k| k.trim())
            .filter(|k| !k.is_empty())
            .map(|key| {
                let mut url = Url::parse(&config.rpc_url).expect("Invalid base RPC URL");
                if !config.rpc_url.ends_with('/') {
                    url.set_path(&format!("/{}", key));
                } else {
                    url =
                        Url::parse(&format!("{}{}", config.rpc_url, key)).expect("Invalid RPC URL");
                }
                Arc::new(Provider::<Http>::try_from(url.as_str()).expect("Invalid RPC URL"))
            })
            .collect::<Vec<_>>();

        log_info!("成功初始化 {} 个RPC Provider", providers.len());
        assert!(!providers.is_empty(), "No valid api keys provided");

        Self {
            providers,
            index: AtomicUsize::new(0),
        }
    }

    pub fn get_provider(&self) -> Arc<Provider<Http>> {
        let i = self.index.fetch_add(1, Ordering::Relaxed);
        self.providers[i % self.providers.len()].clone()
    }
}
#[async_trait]
impl ProviderTrait for EthereumProvider {
    async fn get_last_block_number(&self) -> Result<U64, AppError> {
        self.get_provider()
            .get_block_number()
            .await
            .map_err(AppError::from)
    }

    async fn get_block_with_txs(
        &self,
        number: u64,
    ) -> Result<Option<Block<Transaction>>, AppError> {
        self.get_provider()
            .get_block_with_txs(number)
            .await
            .map_err(AppError::from)
    }

    async fn get_transaction_receipt(
        &self,
        tx_hash: H256,
    ) -> Result<Option<TransactionReceipt>, AppError> {
        self.get_provider()
            .get_transaction_receipt(tx_hash)
            .await
            .map_err(AppError::from)
    }

    async fn get_chain_id(&self) -> Result<U256, AppError> {
        self.get_provider()
            .get_chainid()
            .await
            .map_err(AppError::from)
    }

    async fn get_transaction_count(&self, address: &str) -> Result<U256, AppError> {
        let addr = address
            .parse::<Address>()
            .map_err(|_| AppError::InvalidAddress(address.to_string()))?;
        self.get_provider()
            .get_transaction_count(addr, None)
            .await
            .map_err(AppError::from)
    }

    async fn estimate_eip1559_fees(
        &self,
        estimator: Option<fn(U256, Vec<Vec<U256>>) -> (U256, U256)>,
    ) -> Result<(U256, U256), AppError> {
        self.get_provider()
            .estimate_eip1559_fees(estimator)
            .await
            .map_err(|e| AppError::ProviderError(format!("EIP1559 费用估算失败: {}", e)))
    }

    async fn send_raw_transaction(
        &self,
        rlp: Bytes,
        timeout_secs: u64,
        confirmations: usize,
    ) -> Result<TransactionReceipt, AppError> {
        // 1. 先获取并持有 provider 的所有权 (Arc).确保在整个 await 期间，对应的 Http Client 不会被释放
        let provider = self.get_provider();
        // 2. 广播交易
        let pending_tx = provider
            .send_raw_transaction(rlp)
            .await
            .map_err(|e| AppError::ProviderError(format!("Broadcast failed: {}", e)))?;

        // 3. 等待链上确认
        let receipt_result = timeout(
            std::time::Duration::from_secs(timeout_secs),
            pending_tx.confirmations(confirmations),
        )
        .await;
        let receipt = receipt_result
            .map_err(|_| AppError::Internal("Transaction confirmation timeout".to_string()))? // 处理 timeout 包装
            .map_err(|e| AppError::Internal(format!("Wait receipt error: {}", e)))? // 处理中间件错误
            .ok_or_else(|| AppError::Internal("Transaction dropped from mempool".to_string()))?; // 处理掉包

        // 4. 业务逻辑检查：检查交易是否 Revert (status == 0)
        // status 为 None 通常出现在非 EIP-1559 或老旧节点，但在现代以太坊网络中通常有值
        if let Some(status) = receipt.status {
            if status.is_zero() {
                return Err(AppError::Internal(format!(
                    "Transaction reverted on-chain. Hash: {:?}",
                    receipt.transaction_hash
                )));
            }
        }
        log_info!(
            "交易执行成功: hash={:?}, block={:?}",
            receipt.transaction_hash,
            receipt.block_number
        );
        // 5. 返回 receipt (这是 Owned 数据，没有生命周期问题)
        Ok(receipt)
    }

    async fn call(&self, tx: &TypedTransaction) -> Result<Bytes, AppError> {
        self.get_provider()
            .call(tx, None)
            .await
            .map_err(|e| AppError::ProviderError(format!("Call simulation failed: {}", e)))
    }

    async fn estimate_gas(&self, tx: &TypedTransaction) -> Result<U256, AppError> {
        self.get_provider()
            .estimate_gas(tx, None)
            .await
            .map_err(|e| AppError::ProviderError(format!("estimate_gas failed: {}", e)))
    }
}
