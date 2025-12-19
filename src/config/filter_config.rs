use ethers_core::types::H160;
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;

#[derive(Debug, Deserialize)]
struct AddressList {
    addresses: Vec<String>,
}
pub struct FilterConfig {
    pub contracts: HashSet<H160>,
    pub addresses: HashSet<H160>,
}

impl FilterConfig {
    pub fn load() -> Self {
        let contracts = Self::load_file("config/contracts.toml");
        let addresses = Self::load_file("config/address.toml");
        Self {
            contracts,
            addresses,
        }
    }

    fn load_file(path: &str) -> HashSet<H160> {
        let content = fs::read_to_string(path)
            .unwrap_or_else(|e| {
                panic!("致命错误: 无法读取文件 '{}', 请检查路径是否正确。错误: {}", path, e);
            });
        let list: AddressList =
            toml::from_str(&content).unwrap_or(AddressList { addresses: vec![] });
        list.addresses
            .iter()
            .filter_map(|addr| addr.parse::<H160>().ok())
            .collect()
    }
}
