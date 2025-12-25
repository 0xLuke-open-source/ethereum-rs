// services/tx/gas/gas_strategy.rs

use serde::{Deserialize, Serialize};

/// 交易优先级策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TxPriority {
    Low,
    Normal,
    High,
    Urgent,
}

impl TxPriority {
    /// tip 调整百分比（100 = 无调整）
    /// 示例：150 表示最终 tip = base_tip × 150%
    pub fn tip_multiplier_percent(&self) -> u128 {
        match self {
            TxPriority::Low => 80,     // -20%
            TxPriority::Normal => 100, // 无调整
            TxPriority::High => 150,   // +50%
            TxPriority::Urgent => 300, // +200%
        }
    }

    /// max_fee_per_gas 上限倍率（相对于调整后的 priority_fee）
    /// 返回百分比整数：200 表示允许 max_fee ≤ priority_fee × 2
    pub fn max_fee_cap_multiplier_percent(&self) -> u128 {
        match self {
            TxPriority::Low => 120,    // max_fee ≤ tip × 1.2（保守）
            TxPriority::Normal => 150, // ×1.5
            TxPriority::High => 200,   // ×2.0
            TxPriority::Urgent => 300, // ×3.0（允许更高以确保上链）
        }
    }
}
