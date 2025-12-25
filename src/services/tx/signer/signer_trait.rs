use async_trait::async_trait;
use ethers::types::{transaction::eip2718::TypedTransaction, Signature, H160};
use crate::errors::error::AppError;
#[async_trait]
pub trait TxSigner: Send + Sync {
    async fn sign_tx(&self, tx: &TypedTransaction) -> Result<Signature, AppError>;
    fn address(&self) -> H160;
    fn chain_id(&self) -> Option<u64>; // 返回 None 表示不强制 chain_id
}