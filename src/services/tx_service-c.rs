// services/tx_service.rs

use crate::config::EthereumConfig;
use crate::errors::error::AppError;
use crate::infrastructure::provider::{EthereumProvider, ProviderTrait};
use crate::{log_error, log_info, log_warn};
use ethers::prelude::*;
use ethers::middleware::{
    gas_escalator::{Frequency, GeometricGasPrice, GasEscalatorMiddleware},
    NonceManagerMiddleware, SignerMiddleware,
};
use ethers::types::{
    transaction::eip2718::TypedTransaction,
    Eip1559TransactionRequest, TransactionReceipt, H160, H256, U256, Bytes,
};
use ethers_core::utils::{keccak256, format_units};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{sleep, timeout};

/// 生产级中间件栈：Nonce 管理 → Gas 自动提价 → 签名 → HTTP Provider
type TxClient = NonceManagerMiddleware<
    GasEscalatorMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>,
>;

#[derive(Clone)]
pub struct TxService {
    pub config: Arc<EthereumConfig>,
    /// 用于查询（如 nonce、balance）的带重试 Provider
    pub query_provider: Arc<dyn ProviderTrait>,
    /// 用于发送交易的完整中间件客户端
    pub client: Arc<TxClient>,
    pub wallet_address: H160,
}

impl TxService {
    pub async fn new(
        config: Arc<EthereumConfig>,
        provider_base: Arc<EthereumProvider>,
    ) -> Result<Self, AppError> {
        // 1. 查询专用 Provider（带自定义 RetryAdapter）
        let query_provider = Arc::new(crate::infrastructure::provider::RetryAdapter::new(
            provider_base.clone(),
            config.max_retries.unwrap_or(6),
            Duration::from_secs(config.base_delay_secs.unwrap_or(1)),
        )) as Arc<dyn ProviderTrait>;

        // 2. 基础 HTTP Provider（用于 ethers 中间件）
        let http_provider = Provider::<Http>::try_from(config.rpc_url.as_str())
            .map_err(|e| AppError::Internal(format!("无效的 RPC URL: {}", e)))?
            .interval(Duration::from_millis(1200)); // 更频繁轮询，提升响应性

        // 3. 安全加载钱包（强制绑定 chain_id）
        let private_key = std::env::var("ETH_PRIVATE_KEY")
            .map_err(|_| AppError::Internal("环境变量 ETH_PRIVATE_KEY 未设置".to_string()))?;

        let wallet = private_key
            .parse::<LocalWallet>()
            .map_err(|e| AppError::Internal(format!("私钥格式错误: {}", e)))?
            .with_chain_id(config.chain_id);

        let wallet_address = wallet.address();

        // 4. 构建中间件栈（从内到外）
        let signer = SignerMiddleware::new(http_provider, wallet);

        // Gas Escalator 策略优化：
        // 每 2 个区块（~24s）涨价 15%，最多等待 10 次（~200s）
        // 更激进但合理，避免长时间卡 mempool
        let escalator = GeometricGasPrice::new(1.15, 10u64, None);
        let gas_esc = GasEscalatorMiddleware::new(signer, escalator, Frequency::PerBlock);

        let client = NonceManagerMiddleware::new(gas_esc, wallet_address);

        Ok(Self {
            config,
            query_provider,
            client: Arc::new(client),
            wallet_address,
        })
    }

    /// 核心：发送 EIP-1559 交易（带预模拟 + 自动提价）
    pub async fn send_transaction(
        &self,
        to: H160,
        value: U256,
        data: Bytes,
        gas_limit_opt: Option<U256>,
        required_confirmations: Option<u64>,
    ) -> Result<TransactionReceipt, AppError> {
        // 1. 交易预模拟（关键防 revert 措施）
        self.simulate_call(to, data.clone(), value).await?;

        // 2. 估算 EIP-1559 费用
        let (max_fee_per_gas, max_priority_fee_per_gas) = self
            .client
            .estimate_eip1559_fees(None)
            .await
            .map_err(|e| AppError::Internal(format!("EIP-1559 费率估算失败: {}", e)))?;

        // 3. 构建交易请求
        let mut tx = Eip1559TransactionRequest::new()
            .to(to)
            .value(value)
            .data(data)
            .max_fee_per_gas(max_fee_per_gas)
            .max_priority_fee_per_gas(max_priority_fee_per_gas)
            .chain_id(self.config.chain_id);

        // 4. 估算并设置 Gas Limit（+20% buffer）
        let gas_limit = match gas_limit_opt {
            Some(limit) => limit,
            None => {
                let estimated = self
                    .client
                    .estimate_gas(&TypedTransaction::Eip1559(tx.clone()), None)
                    .await
                    .map_err(|e| AppError::Internal(format!("Gas Limit 估算失败: {}", e)))?;
                estimated * 120 / 100
            }
        };
        tx = tx.gas(gas_limit);

        let value_display = if value.is_zero() {
            "0".to_string()
        } else {
            format_units(value, "ether").unwrap_or_else(|_| value.to_string())
        };

        log_info!(
            "正在广播交易 → to: {:?} | value: {} ETH | gas: {} | max_fee: {} Gwei | priority: {} Gwei",
            to,
            value_display,
            gas_limit,
            max_fee_per_gas / 1_000_000_000u128,
            max_priority_fee_per_gas / 1_000_000_000u128
        );

        // 5. 发送交易（NonceManager + GasEscalator 自动接管）
        let pending_tx = self
            .client
            .send_transaction(tx, None)
            .await
            .map_err(|e| AppError::Internal(format!("交易广播失败: {}", e)))?;

        let tx_hash = pending_tx.tx_hash();
        log_info!("交易已广播，等待上链 → hash: {}", tx_hash);

        // 6. 等待确认（默认 1 确认，L2 可用；主网建议外部传 12）
        let confirmations = required_confirmations.unwrap_or(1);
        let receipt = timeout(Duration::from_secs(600), pending_tx.confirmations(confirmations))
            .await
            .map_err(|_| AppError::Internal("交易确认超时（10分钟）".to_string()))?
            .map_err(|e| AppError::Internal(format!("确认过程异常: {}", e)))?
            .ok_or_else(|| AppError::Internal("交易被 mempool 丢弃".to_string()))?;

        // 7. 检查执行结果
        if receipt.status.unwrap_or(U64::zero()) == U64::zero() {
            log_error!("交易执行失败（REVERT） → hash: {}", tx_hash);
            return Err(AppError::Internal(format!("交易 Revert: {}", tx_hash)));
        }

        log_info!(
            "交易成功确认 → hash: {} | block: {:?} | gas_used: {:?} | confirmations: {}",
            tx_hash,
            receipt.block_number,
            receipt.gas_used,
            confirmations
        );

        Ok(receipt)
    }

    /// 高安全性场景：离线签名 + 原始广播
    pub async fn sign_and_broadcast_raw(
        &self,
        mut tx_req: Eip1559TransactionRequest,
    ) -> Result<H256, AppError> {
        // 自动填充 nonce（如果未提供）
        if tx_req.nonce.is_none() {
            let nonce = self
                .client
                .get_transaction_count(self.wallet_address, None)
                .await
                .map_err(|e| AppError::Internal(format!("获取 nonce 失败: {}", e)))?;
            tx_req = tx_req.nonce(nonce);
        }

        let typed: TypedTransaction = tx_req.into();
        let signature = self
            .client
            .signer()
            .sign_transaction(&typed)
            .await
            .map_err(|e| AppError::Internal(format!("离线签名失败: {}", e)))?;

        let signed_rlp = typed.rlp_signed(&signature);

        let pending = self
            .query_provider
            .send_raw_transaction(signed_rlp.into())
            .await
            .map_err(|e| AppError::Internal(format!("原始交易广播失败: {}", e)))?;

        let hash = pending.tx_hash();
        log_info!("离线签名交易已广播 → hash: {}", hash);
        Ok(hash)
    }

    /// 交易预执行模拟（强烈推荐始终启用）
    pub async fn simulate_call(&self, to: H160, data: Bytes, value: U256) -> Result<(), AppError> {
        let req = TransactionRequest::new()
            .to(to)
            .data(data)
            .value(value);

        self.client
            .call(&req.into(), None)
            .await
            .map_err(|e| AppError::Internal(format!("交易模拟失败（可能 Revert）: {}", e)))?;

        Ok(())
    }

    /// 标准 ERC20 transfer 封装
    pub async fn erc20_transfer(
        &self,
        token: H160,
        to: H160,
        amount: U256,
    ) -> Result<H256, AppError> {
        let selector = &keccak256("transfer(address,uint256)")[..4];
        let mut data = Vec::with_capacity(68);
        data.extend_from_slice(selector);
        data.extend_from_slice(&ethers::abi::encode(&[
            ethers::abi::Token::Address(to),
            ethers::abi::Token::Uint(amount),
        ]));

        let receipt = self
            .send_transaction(token, U256::zero(), data.into(), None, None)
            .await?;

        Ok(receipt.transaction_hash)
    }

    /// 批量转账（顺序发送，安全可靠）
    pub async fn batch_erc20_transfer(
        &self,
        token: H160,
        recipients: Vec<(H160, U256)>,
        delay_ms: Option<u64>,
    ) -> Result<Vec<H256>, AppError> {
        let mut hashes = Vec::with_capacity(recipients.len());

        for (to, amount) in recipients {
            let hash = self.erc20_transfer(token, to, amount).await?;
            hashes.push(hash);

            if let Some(ms) = delay_ms {
                sleep(Duration::from_millis(ms)).await;
            }
        }

        Ok(hashes)
    }
}