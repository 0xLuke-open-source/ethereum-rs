use ethers::prelude::U256;
use ethers_core::types::{Transaction};

// 定义 ERC-20 transfer 的函数签名（前4个字节）
const ERC20_TRANSFER_SIGNATURE: [u8; 4] = [0xa9, 0x05, 0x9c, 0xbb];

/// 检查交易是否为 ETH 转账或 ERC-20 transfer
pub fn is_target_transaction(tx: &Transaction) -> bool {
    // 交易必须有目标地址 (排除合约创建交易)
    if tx.to.is_none() {
        return false;
    }

    // --- 识别 ETH 转账 ---
    if tx.input.is_empty() {
        // 纯 ETH 转账：input 为空且 value > 0
        return tx.value > U256::zero();
    }

    // --- 识别 ERC-20 Transfer ---
    // 必须有 input 数据，且 input 长度至少为 4 字节的签名
    if tx.input.len() >= 4 {
        let input_slice = &tx.input.as_ref()[0..4];

        // 检查 input 的前 4 字节是否匹配 transfer 函数签名
        if input_slice == ERC20_TRANSFER_SIGNATURE {
            // 进一步检查：确保交易的 value == 0，因为 ERC-20 transfer 不应携带 ETH
            // 严格来说，transfer 也可以携带 ETH，但通常认为是纯 ERC-20 操作。
            // 这里为了只关注 ERC-20，可以加上此限制。
            return tx.value == U256::zero();
        }
    }
    // 既不是 ETH 转账，也不是 ERC-20 transfer（可能是其他合约调用）
    false
}