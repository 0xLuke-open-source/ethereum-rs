use crate::errors::error::AppError;
use ethers_core::types::{H256, U64, U256};

pub fn option_u64_to_i64(opt_u64: Option<U64>) -> Result<i64, AppError> {
    // 1. 处理空值：使用 map_or_else 避免两次解包
    let u64_val = opt_u64
        .ok_or(AppError::InvalidNumber("Block number is None".to_string()))?
        .as_u64(); // 转换为原生 u64

    // 2. u64 转 i64（溢出检查）
    u64_val.try_into().map_err(|e| {
        // e 是 std::num::TryFromIntError
        AppError::ConversionError(format!("u64({}) 转 i64 溢出: {}", u64_val, e))
    })
}

pub fn h256_opt_to_string(data: Option<H256>) -> String {
    match data {
        Some(str) => format!("{:#x}", str), // 0x + 64 位 hex
        None => String::new(),
    }
}

pub fn h256_to_string(data: H256) -> String {
    // 逻辑：直接调用 H256 类型实现的 ToString trait，将其转换为 String。
    format!("{:#x}", data)
}

pub fn u256_to_i64(u256_val: U256) -> Result<i64, AppError> {
    // 1. 检查 U256 是否超出 U128 范围（即高 128 位是否为 0）
    let u128_val: u128 = u256_val.try_into().map_err(|e| {
        // e 是 TryFromIntError，我们可以利用它进行格式化
        AppError::ConversionError(format!("U256({}) 超出u128范围: {}", u256_val, e))
    })?;

    // 2. 检查 u128 是否超出 i64 范围（即是否大于 i64::MAX）
    // 由于 U256 是无符号的，我们只需要比较它与 i64 的正最大值。
    if u128_val > i64::MAX as u128 {
        return Err(AppError::ConversionError(format!(
            "U256({}) 超出i64范围（最大值: {}）",
            u256_val,
            i64::MAX
        )));
    }

    // 3. 安全转换
    // 此时 u128_val 保证在 [0, i64::MAX] 范围内，转换是安全的。
    Ok(u128_val as i64)
}

pub fn opt_u256_to_i64_loose(opt_u256: Option<U256>) -> Result<i64, AppError> {
    // 宽松策略：如果输入是 None，视为 U256::zero()
    let u256_val = opt_u256.unwrap_or(U256::zero());

    // U256 内部是 [u64; 4]，索引0=最低64位，索引1=次低64位
    let parts: [u64; 4] = u256_val.0;
    let low_u64 = parts[0];
    let high_u64 = parts[1];
    // parts[2] 和 parts[3] 默认为 0，但为了安全起见，应检查。

    // 溢出校验: U256 必须小于或等于 i64::MAX
    // 1. 检查次低64位是否为0 (高位 parts[2], parts[3] 假设为 0，因为 U256 存储了 256 位)
    // 2. 检查最低64位是否超过 i64 的最大值
    if parts[1] != 0 || parts[2] != 0 || parts[3] != 0 || low_u64 > i64::MAX as u64 {
        return Err(AppError::ConversionError(format!(
            "U256({}) 超出 i64 范围（i64最大值: {}）",
            u256_val,
            i64::MAX
        )));
    }

    // 如果通过校验，则 low_u64 可以在 i64 范围内安全转换
    Ok(low_u64 as i64)
}
