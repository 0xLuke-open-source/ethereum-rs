use std::sync::Arc;
use std::time::Duration;
use tracing::info;

use crate::config::Config;
use crate::config::filter_config::FilterConfig;
use crate::database::diesel::{DbService, create_async_db_pool};
use crate::errors::error::AppError;
use crate::infrastructure::parser::EventParser;
use crate::infrastructure::provider::ethereum_provider::EthereumProvider;
use crate::infrastructure::provider::{ProviderTrait, RetryAdapter};
use crate::log_info;
use crate::repositories::block_repository::BlockRepository;
use crate::repositories::transaction_repository::TransactionRepository;
use crate::services::BlockService;

/// 应用程序启动与管理结构体（仅后台服务，无HTTP API）
pub struct Application {
    pub block_service: Arc<BlockService>,
}
pub type Result<T> = std::result::Result<T, AppError>;
impl Application {
    /// 构建应用实例（仅初始化数据库/Redis，不启动服务）
    pub async fn build(config: Config) -> Result<Self> {
        let filter_config = Arc::new(FilterConfig::load());
        log_info!(
            "Filter configuration loaded: {} contracts, {} address",
            &filter_config.contracts.len(),
            &filter_config.addresses.len(),
        );
        // 初始化异步池
        let db_pool = create_async_db_pool(&config.database).await?;
        let db_service = Arc::new(DbService { pool: db_pool });
        info!("Diesel database pool initialized successfully");
        // 实例化 Repository (现在是无状态的)
        let block_repo = Arc::new(BlockRepository::new());
        let tx_repo = Arc::new(TransactionRepository::new());

        // 1. 先初始化 Provider
        let eth_provider = Arc::new(EthereumProvider::new(&config.ethereum));

        let provider = Arc::new(RetryAdapter::new(
            eth_provider,
            config.ethereum.max_retries,
            Duration::from_secs(config.ethereum.base_delay_secs),
        )) as Arc<dyn ProviderTrait>;

        // 2. 将 provider 注入 EventParser
        let event_parser = Arc::new(EventParser::new(provider.clone()));

        // 3. 实例化 BlockService
        let block_service = Arc::new(BlockService::new(
            Arc::new(config.ethereum),
            filter_config,
            block_repo,
            tx_repo,
            db_service,
            provider,
            event_parser,
        ));
        Ok(Self { block_service })
    }

    /// 启动应用核心服务（例如：区块同步循环）
    pub async fn run(self) -> anyhow::Result<()> {
        let s1 = self.block_service.clone();
        tokio::join!(async move {
            loop {
                match s1.sync_blocks().await {
                    Ok(()) => {
                        // 区块同步成功，立即尝试同步下一个
                        // tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                    Err(e) => {
                        tracing::error!("同步区块失败: {:?}", e);
                        // 失败后等待一段时间后重试，避免高速失败
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        },);

        log_info!("✔️ All parsing tasks started");

        // 等待 Ctrl+C 退出
        tokio::signal::ctrl_c().await?;
        log_info!("⚠️  Received shutdown signal, exiting...");
        Ok(())
    }
}
