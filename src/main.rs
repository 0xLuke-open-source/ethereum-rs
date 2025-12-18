use crate::config::Config;
use crate::utils::logger::init_logger;
use anyhow::Context;
use crate::startup::startup::Application;

mod cli;
mod config;
mod database;
mod errors;

mod infrastructure;
mod models;
mod repositories;
mod services;
mod startup;
mod utils;
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志（全局只需调用一次）
    init_logger();

    // 打印不同级别日志
    // log_trace!("这是TRACE级日志（开发调试用）");
    // log_debug!("这是DEBUG级日志 | 当前区块号: {}", 6000000);
    // log_info!("这是INFO级日志 | 已连接RPC: {}", "https://sepolia.infura.io");
    // log_warn!("这是WARN级日志 | 批量解析并发数过高: {}", 20);
    // log_error!("这是ERROR级日志 | 解析区块失败: {}", 6000001);

    log_info!("Starting application initialization...");

    // 1. 加载配置
    let config = Config::load().context("Failed to load application configuration")?;

    // 2. 构建应用实例 (初始化资源)
    // Application::build 返回 Result<Application, Error>，
    // 使用 ? 自动转换为 anyhow::Result<Application>
    let application = Application::build(config)
        .await
        .context("Application building failed (DB/Redis initialization)")?;

    log_info!("Application build complete. Starting service loop.");

    // 3. 运行应用核心服务
    // run 函数包含了启动后台任务和主循环逻辑
    application
        .run()
        .await
        .context("Application core service failed during runtime")?;

    // 如果 run() 正常退出，则返回 Ok
    Ok(())
}
