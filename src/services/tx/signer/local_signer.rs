// services/tx/signer/local_signer.rs

use crate::errors::error::AppError;
use crate::services::tx::signer::TxSigner;
use ethers_core::types::{H160, Signature};
use ethers_core::types::transaction::eip2718::TypedTransaction;
use ethers_signers::{LocalWallet, Signer};
use std::sync::Arc;

#[derive(Clone)]
pub struct LocalSigner {
    wallet: Arc<LocalWallet>,
}

impl LocalSigner {
    pub fn new(wallet: LocalWallet) -> Self {
        Self { wallet: Arc::new(wallet) }
    }
}

#[async_trait::async_trait]
impl TxSigner for LocalSigner {
    async fn sign_tx(&self, tx: &TypedTransaction) -> Result<Signature, AppError> {
        self.wallet
            .sign_transaction(tx)
            .await
            .map_err(|e| AppError::Internal(format!("Signing failed: {}", e)))
    }

    fn address(&self) -> H160 {
        self.wallet.address()
    }

    fn chain_id(&self) -> Option<u64> {
        Option::from(self.wallet.chain_id())
    }
}