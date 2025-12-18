//! 日志模块：完全适配 env_logger 0.11.8（含颜色、文件、轮转）
use env_logger::fmt::Formatter;
use env_logger::{
    Builder, Target, {self, WriteStyle},
};
use log::{Level, LevelFilter, Record};
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use std::sync::Mutex;
use std::sync::Once;

// ==================== 配置常量 ====================
const LOG_DIR: &str = "LOG_DIR";
const DEFAULT_LOG_DIR: &str = "logs";
const LOG_LEVEL: &str = "LOG_LEVEL";
const DEFAULT_LOG_LEVEL: &str = "INFO";
const LOG_FILE_NAME: &str = "eth-block-parser.log";
const LOG_MAX_SIZE_MB: u64 = 10;
const LOG_MAX_ROTATIONS: usize = 5;

static INIT_LOGGER: Once = Once::new();
// 新增：全局文件写入器（替代文件 Builder 方案）
static FILE_WRITER: Mutex<Option<File>> = Mutex::new(None);

// ==================== 初始化日志系统 ====================
pub fn init_logger() {
    INIT_LOGGER.call_once(|| {
        // 读取环境变量
        let log_dir = std::env::var(LOG_DIR).unwrap_or_else(|_| DEFAULT_LOG_DIR.to_string());
        let log_level = std::env::var(LOG_LEVEL)
            .unwrap_or_else(|_| DEFAULT_LOG_LEVEL.to_string())
            .to_uppercase();

        // 日志级别映射
        let level_filter = match log_level.as_str() {
            "TRACE" => LevelFilter::Trace,
            "DEBUG" => LevelFilter::Debug,
            "INFO" => LevelFilter::Info,
            "WARN" => LevelFilter::Warn,
            "ERROR" => LevelFilter::Error,
            _ => {
                eprintln!("⚠️ 无效日志级别「{}」，使用默认 INFO", log_level);
                LevelFilter::Info
            }
        };

        // 创建日志目录
        if let Err(e) = fs::create_dir_all(&log_dir) {
            eprintln!("❌ 创建日志目录失败: {}", e);
        }

        // 日志轮转
        if let Err(e) = rotate_logs(&log_dir, LOG_FILE_NAME) {
            eprintln!("⚠️ 日志轮转失败: {}", e);
        }

        // 提前创建文件并保存到全局变量（核心改动1）
        let log_file_path = Path::new(&log_dir).join(LOG_FILE_NAME);
        let file = match File::create(&log_file_path) {
            Ok(f) => {
                *FILE_WRITER.lock().unwrap() = Some(f);
                true
            }
            Err(e) => {
                eprintln!("❌ 创建日志文件失败: {}", e);
                false
            }
        };

        // ==================== 控制台 Builder（唯一的日志器） ====================
        let mut console_builder = Builder::from_default_env();
        console_builder
            .filter(None, level_filter)
            .filter(Some("ethers_providers"), LevelFilter::Warn)
            .filter(Some("ethers_contract"), LevelFilter::Warn)
            .write_style(WriteStyle::Always)
            .format(move |f: &mut Formatter, record: &Record| {
                let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S.%3f");

                // 1. 控制台彩色输出（保留你的逻辑）
                let level_color = match record.level() {
                    Level::Error => "\x1b[91m", // 亮红色
                    Level::Warn => "\x1b[93m",  // 亮黄色
                    Level::Info => "\x1b[92m",  // 亮绿色
                    Level::Debug => "\x1b[96m", // 亮青色
                    Level::Trace => "\x1b[95m", // 亮紫色
                };
                let reset = "\x1b[0m";
                let module_color = "\x1b[31m"; // 红色

                let console_log = writeln!(
                    f,
                    "[{}] [{}] [{}] - {}",
                    now,
                    format!("{}{:>5}{}", level_color, record.level(), reset),
                    format!(
                        "{}{}{}",
                        module_color,
                        record.module_path().unwrap_or("unknown"),
                        reset
                    ),
                    record.args()
                );

                // 2. 同时写入文件（核心改动2：复用全局文件句柄）
                if file {
                    let file_log = format!(
                        "[{}] [线程: {}] [模块: {}] [级别: {}] - {}\n",
                        now,
                        std::thread::current().name().unwrap_or("unknown"),
                        record.module_path().unwrap_or("unknown"),
                        record.level(),
                        record.args()
                    );
                    // 忽略文件写入错误（避免影响控制台输出）
                    let _ = FILE_WRITER
                        .lock()
                        .unwrap()
                        .as_mut()
                        .unwrap()
                        .write_all(file_log.as_bytes());
                }

                console_log
            })
            .target(Target::Stdout);

        // 仅初始化一次（核心改动3：删除文件 Builder）
        if let Err(e) = console_builder.try_init() {
            eprintln!("❌ 控制台日志初始化失败: {}", e);
        } else {
            log::info!(
                "✅ 日志系统初始化完成 | 级别: {} | 日志文件: {}",
                log_level,
                log_file_path.display()
            );
        }
    });
}

// ==================== 日志轮转（无改动） ====================
fn rotate_logs(log_dir: &str, log_file: &str) -> io::Result<()> {
    let log_path = Path::new(log_dir).join(log_file);

    if !log_path.exists() {
        return Ok(());
    }

    let file_size_mb = fs::metadata(&log_path)?.len() / (1024 * 1024);
    if file_size_mb < LOG_MAX_SIZE_MB {
        return Ok(());
    }

    log::info!(
        "📜 日志文件超过阈值 {}MB，开始轮转 | 当前大小: {}MB",
        LOG_MAX_SIZE_MB,
        file_size_mb
    );

    for i in (1..LOG_MAX_ROTATIONS).rev() {
        let src = Path::new(log_dir).join(format!("{}.{}", log_file, i));
        let dest = Path::new(log_dir).join(format!("{}.{}", log_file, i + 1));
        if src.exists() {
            fs::rename(&src, &dest)?;
        }
    }

    let new_log_1 = Path::new(log_dir).join(format!("{}.1", log_file));
    fs::rename(&log_path, &new_log_1)?;
    File::create(&log_path)?;

    // 轮转后更新全局文件句柄
    *FILE_WRITER.lock().unwrap() = File::create(log_path).ok();

    Ok(())
}

// ==================== 便捷日志宏（无改动） ====================
#[macro_export]
macro_rules! log_trace { ($($arg:tt)*) => { log::trace!($($arg)*) }; }
#[macro_export]
macro_rules! log_debug { ($($arg:tt)*) => { log::debug!($($arg)*) }; }
#[macro_export]
macro_rules! log_info  { ($($arg:tt)*) => { log::info!($($arg)*) }; }
#[macro_export]
macro_rules! log_warn  { ($($arg:tt)*) => { log::warn!($($arg)*) }; }
#[macro_export]
macro_rules! log_error { ($($arg:tt)*) => { log::error!($($arg)*) }; }
