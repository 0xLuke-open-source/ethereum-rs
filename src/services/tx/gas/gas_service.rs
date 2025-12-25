// services/tx/gas/gas_service.rs

use crate::errors::error::AppError;
use crate::services::tx::gas::gas_strategy::TxPriority;
use ethers_core::types::U256;
use ethers_providers::Middleware;
use crate::infrastructure::provider::ProviderTrait;

/// Gas 费用计算服务（纯整数运算，无浮点风险）
#[derive(Clone, Copy, Debug)]
pub struct GasService {
    /// 全局对 tip 的额外调整百分比（100 = 无调整，110 = +10%，90 = -10%）
    base_tip_percent: u128,
}

impl Default for GasService {
    fn default() -> Self {
        Self::default()
    }
}

impl GasService {
    /// 构造函数：传入百分比整数
    /// 示例：GasService::new(110) 表示全局 tip +10%
    pub fn new(base_tip_percent: u128) -> Self {
        Self { base_tip_percent }
    }

    /// 便捷构造函数：无额外调整
    pub fn default() -> Self {
        Self::new(100)
    }

    /// 核心方法：根据优先级动态计算 EIP-1559 费用
    pub async fn resolve_fees(
        &self,
        provider: &dyn ProviderTrait,
        priority: TxPriority,
    ) -> Result<(U256, U256), AppError> {
        // 1. 获取链上建议的费用
        let (max_fee_per_gas, base_priority_fee) = provider
            .estimate_eip1559_fees(None)
            .await
            .map_err(|e| AppError::Internal(format!("EIP1559 fee estimation failed: {}", e)))?;

        // 2. 计算优先级调整后的 tip（整数百分比运算）
        let priority_multiplier = priority.tip_multiplier_percent(); // 如 High -> 150

        let total_multiplier = self
            .base_tip_percent
            .checked_mul(priority_multiplier)
            .ok_or_else(|| {
                AppError::Internal("Tip multiplier overflow during calculation".to_string())
            })?
            / 100;

        let adjusted_priority_fee = base_priority_fee
            .checked_mul(U256::from(total_multiplier))
            .ok_or_else(|| AppError::Internal("Adjusted priority fee overflow".to_string()))?
            / U256::from(100);

        // 3. 计算 max_fee_per_gas 的安全上限
        // 策略：max_fee 不应远高于调整后的 tip
        let cap_multiplier = priority.max_fee_cap_multiplier_percent(); // 如 High -> 200

        let max_allowed_fee = adjusted_priority_fee
            .checked_mul(U256::from(cap_multiplier))
            .ok_or_else(|| AppError::Internal("Max fee cap calculation overflow".to_string()))?
            / U256::from(100);

        // 取链上建议值与我们安全上限的较小值（保守策略）
        let final_max_fee_per_gas = max_fee_per_gas.min(max_allowed_fee);

        Ok((final_max_fee_per_gas, adjusted_priority_fee))
    }
}
