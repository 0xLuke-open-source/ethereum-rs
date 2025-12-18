use std::sync::Arc;
use std::time::Duration;
use tracing::info;

use crate::config::Config;
use crate::database::diesel::create_diesel_pool;
use crate::database::redis::create_redis_pool;
use crate::errors::error::AppError;
use crate::infrastructure::provider::ethereum_provider::EthereumProvider;
use crate::log_info;
use crate::repositories::block_repository::BlockRepository;
use crate::repositories::transaction_repository::TransactionRepository;
use crate::services::BlockService;

/// 应用程序启动与管理结构体（仅后台服务，无HTTP API）
pub struct Application {
    pub block_service: Arc<BlockService>,
    // pub tx_service: Arc<TxService>,
    // pub erc20_service: Arc<Erc20Service>,
}
pub type Result<T> = std::result::Result<T, AppError>;
impl Application {
    /// 构建应用实例（仅初始化数据库/Redis，不启动服务）
    pub async fn build(config: Config) -> Result<Self> {
        // 1. 初始化 Diesel 数据库连接池（同步池，Diesel 核心）
        let db_pool = create_diesel_pool(&config.database).map_err(|e| {
            eprintln!("Failed to create Diesel database pool: {:?}", e);
            e
        })?;
        info!("✅ Diesel database pool initialized successfully");

        // 2. 初始化 Redis 异步连接池（redis-rs 的 ConnectionManager）
        // let redis_manager = create_redis_pool(&config.redis).await.map_err(|e| {
        //     eprintln!("Failed to create Redis connection pool: {:?}", e);
        //     e
        // })?;
        info!("✅ Redis connection pool initialized successfully");

        // 3. 验证连接有效性（可选，建议保留）

        // 4. 构建核心服务（仓库 → 服务）
        // 全局存储服务实例（或通过参数传递，此处简化为全局静态，生产建议用依赖注入框架）

        // Repository 层
        let block_repo = Arc::new(BlockRepository::new(db_pool.clone()));
        let transaction_repo = Arc::new(TransactionRepository::new(db_pool.clone()));
        // let tx_repo = Arc::new(TxRepository::new(db_pool.clone()));
        // let erc20_repo = Arc::new(Erc20Repository::new(db_pool));

        // Service 层
        let block_service = Arc::new(BlockService::new(
            block_repo,
            transaction_repo,
            Arc::new(config.ethereum),
        ));
        // let tx_service = Arc::new(TxService::new(tx_repo));
        // let erc20_service = Arc::new(Erc20Service::new(erc20_repo));

        Ok(Self {
            block_service,
            // tx_service,
            // erc20_service,
        })
    }

    /// 启动应用核心服务（例如：区块同步循环）
    pub async fn run(self) -> anyhow::Result<()> {
        let s1 = self.block_service.clone();
        // let s2 = self.tx_service.clone();
        // let s3 = self.erc20_service.clone();

        tokio::join!(
            async move {
                loop {
                    match s1.sync_blocks().await {
                        Ok(()) => {
                            // 区块同步成功，立即尝试同步下一个
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                        Err(e) => {
                            tracing::error!("同步区块失败: {:?}", e);
                            // 失败后等待一段时间后重试，避免高速失败
                            tokio::time::sleep(Duration::from_secs(5)).await;
                        }
                    }
                }
            },
            // async move {
            //     loop {
            //         if let Err(e) = s2.check_pending_txs().await {
            //             log_error!("tx check error: {:?}", e);
            //         }
            //         tokio::time::sleep(Duration::from_secs(1)).await;
            //     }
            // },
            // async move {
            //     loop {
            //         if let Err(e) = s3.handle_deposit().await {
            //             log_error!("erc20 handle error: {:?}", e);
            //         }
            //     }
            // }
        );

        log_info!("✔️ All parsing tasks started");

        // 等待 Ctrl+C 退出
        tokio::signal::ctrl_c().await?;
        log_info!("⚠️  Received shutdown signal, exiting...");
        Ok(())
    }
}
