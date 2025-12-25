// services/tx/simulation/simulation_service.rs

use crate::errors::error::AppError;
use crate::infrastructure::provider::ProviderTrait;
use crate::services::tx::types::TxContext;
use ethers_core::types::{TransactionRequest, U256};
use ethers_providers::{Middleware, Provider};
use std::sync::Arc;

pub struct SimulationService {}

impl SimulationService {
    pub fn new(provider: Arc<Provider<ethers_providers::Http>>) -> Self {
        Self {}
    }

    pub async fn run(&self, ctx: &TxContext, provider: &dyn ProviderTrait) -> Result<(), AppError> {
        let req = TransactionRequest::new()
            .to(ctx.to)
            .value(ctx.value)
            .data(ctx.data.clone());

        provider
            .call(&req.into())
            .await
            .map_err(|e| AppError::Internal(format!("Simulation failed (likely revert): {}", e)))?;
        Ok(())
    }
}
