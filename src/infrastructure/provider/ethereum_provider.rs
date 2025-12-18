use crate::config::EthereumConfig;
use crate::errors::error::AppError;
use crate::log_info;
use async_trait::async_trait;
use ethers::addressbook::Address;
use ethers::prelude::{H256, U64, U256};
use ethers_core::types::{Block, Transaction, TransactionReceipt};
use ethers_providers::{Http, Middleware, Provider, ProviderError};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
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
}
