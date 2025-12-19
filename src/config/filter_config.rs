use crate::{log_error, log_info};
use arc_swap::ArcSwap;
use ethers_core::types::H160;
use notify::{Config as NotifyConfig, RecursiveMode, Watcher};
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
struct AddressList {
    addresses: Vec<String>,
}
pub struct FilterConfig {
    pub contracts: HashSet<H160>,
    pub addresses: HashSet<H160>,
}

pub struct FilterConfigContainer {
    // 使用 ArcSwap 存储当前的配置，支持无锁替换
    current: ArcSwap<FilterConfig>,
}

impl FilterConfigContainer {
    pub fn new() -> Arc<Self> {
        let initial = Arc::new(FilterConfig::load());
        let container = Arc::new(Self {
            current: ArcSwap::from(initial),
        });

        // 启动后台监听线程
        let container_clone = Arc::clone(&container);
        std::thread::spawn(move || {
            container_clone.watch_config();
        });

        container
    }

    // 获取当前配置的快照（解析区块时调用）
    pub fn load(&self) -> Arc<FilterConfig> {
        self.current.load_full()
    }

    fn watch_config(&self) {
        let (tx, rx) = std::sync::mpsc::channel();

        // 初始化监听器
        let mut watcher = notify::RecommendedWatcher::new(tx, NotifyConfig::default())
            .expect("Failed to create watcher");

        // 监听 config 目录
        watcher
            .watch(Path::new("config/"), RecursiveMode::NonRecursive)
            .expect("Failed to watch config directory");

        log_info!("🚀 已启动配置文件热重载监听: config/");

        for res in rx {
            match res {
                Ok(event) => {
                    // 仅当文件修改或重命名时触发加载
                    if event.kind.is_modify() || event.kind.is_create() {
                        log_info!("🔄 检测到配置变动，正在重新加载地址库...");
                        let new_config = Arc::new(FilterConfig::load());
                        self.current.store(new_config);
                        log_info!("✅ 地址库已动态更新！");
                    }
                }
                Err(e) => log_error!("watch error: {:?}", e),
            }
        }
    }
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
        let content = fs::read_to_string(path).unwrap_or_else(|e| {
            panic!(
                "致命错误: 无法读取文件 '{}', 请检查路径是否正确。错误: {}",
                path, e
            );
        });
        let list: AddressList =
            toml::from_str(&content).unwrap_or(AddressList { addresses: vec![] });
        list.addresses
            .iter()
            .filter_map(|addr| addr.parse::<H160>().ok())
            .collect()
    }
}
