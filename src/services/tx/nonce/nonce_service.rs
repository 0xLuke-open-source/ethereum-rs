// services/tx/nonce/nonce_service.rs

use crate::errors::error::AppError;
use ethers_core::types::{H160, U256};
use ethers_providers::Middleware;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Mutex;

// 创建一次，永久共享
// let nonce_service = Arc::new(NonceService::new(&provider, wallet_address).await?);
//
// // 在所有需要的地方注入同一个 Arc
// let tx_service1 = TxService::new(..., nonce_service.clone());
// let tx_service2 = TxService::new(..., nonce_service.clone()); // 仅增加引用计数
pub struct NonceService {
    address: H160,
    /// 本地维护的下一个可用 nonce（原子操作，适合高并发预占）
    current_nonce: AtomicU64,
    /// 防止并发同步链上 nonce 时冲突
    sync_lock: Mutex<()>,
}

impl NonceService {
    /// 创建实例并从链上初始化 nonce
    pub async fn new<M: Middleware + 'static>(provider: &M, address: H160) -> Result<Self, AppError> {
        let chain_nonce = provider
            .get_transaction_count(address, None)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to fetch initial nonce: {}", e)))?;

        Ok(Self {
            address,
            current_nonce: AtomicU64::new(chain_nonce.as_u64()),
            sync_lock: Mutex::new(()),
        })
    }

    /// 预占一个 nonce（并发安全，快速）
    pub fn acquire(&self) -> u64 {
        self.current_nonce
            .fetch_add(1, Ordering::SeqCst)
    }

    /// 交易失败或取消时回滚 nonce（防止 nonce 空洞）
    pub fn rollback(&self) {
        let _ = self.current_nonce.fetch_sub(1, Ordering::SeqCst);
        // 如果下溢（理论上不会），可记录日志
    }

    /// 强制从链上同步最新 nonce（用于恢复或检测到不一致时）
    pub async fn sync<M: Middleware + 'static>(&self, provider: &M) -> Result<(), AppError> {
        let _guard = self.sync_lock.lock().await;

        let chain_nonce = provider
            .get_transaction_count(self.address, None)
            .await
            .map_err(|e| AppError::Internal(format!("Nonce sync failed: {}", e)))?;

        let new_nonce = chain_nonce.as_u64();
        let current = self.current_nonce.load(Ordering::SeqCst);

        if new_nonce > current {
            self.current_nonce.store(new_nonce, Ordering::SeqCst);
        }

        Ok(())
    }

    /// 获取当前缓存的 nonce（用于监控）
    pub fn current(&self) -> u64 {
        self.current_nonce.load(Ordering::SeqCst)
    }
}