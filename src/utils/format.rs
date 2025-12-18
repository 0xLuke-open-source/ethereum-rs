use bigdecimal::BigDecimal;
use ethers_core::types::U256;
use std::str::FromStr;

/// 将U256 BigDecimal
pub fn u256_to_bigdecimal(value: U256) -> BigDecimal {
    // 方式 A：先转字符串再转 BigDecimal (最安全，处理大数最稳)
    let s = value.to_string();
    BigDecimal::from_str(&s).unwrap_or_else(|_| BigDecimal::from(0))
}
