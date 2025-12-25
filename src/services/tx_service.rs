// services/tx/tx_service.rs
use crate::errors::error::AppError;
use crate::infrastructure::provider::ProviderTrait;
use crate::log_info;
use crate::services::tx::gas::gas_service::GasService;
use crate::services::tx::nonce::nonce_service::NonceService;
use crate::services::tx::signer::TxSigner;
use crate::services::tx::simulation::simulation_service::SimulationService;
use crate::services::tx::types::{TxContext, TxOptions, TxResult};
use ethers_contract::EthEvent;
use ethers_core::abi::RawLog;
use ethers_core::types::{Address, Eip1559TransactionRequest, TransactionReceipt, U256, transaction::eip2718::TypedTransaction, Bytes};
use std::sync::Arc;
use ethers_core::utils::keccak256;

pub struct TxService {
    pub signer: Arc<dyn TxSigner>,
    pub nonce_svc: Arc<NonceService>,
    pub gas_svc: Arc<GasService>,
    pub simulation: Arc<SimulationService>,
    pub provider: Arc<dyn ProviderTrait>,
}

#[derive(EthEvent, Debug)]
#[ethevent(name = "Transfer", abi = "Transfer(address,address,uint256)")]
pub struct TransferEvent {
    #[ethevent(indexed)]
    pub from: Address,
    #[ethevent(indexed)]
    pub to: Address,
    pub value: U256,
}
impl TxService {
    pub fn new(
        signer: Arc<dyn TxSigner>,
        nonce_svc: Arc<NonceService>,
        gas_svc: Arc<GasService>,
        simulation: Arc<SimulationService>,
        provider: Arc<dyn ProviderTrait>,
    ) -> Self {
        Self {
            signer,
            nonce_svc,
            gas_svc,
            simulation,
            provider,
        }
    }

    /// 1. 集成 ETH 原生转账
    pub async fn transfer_eth(
        &self,
        to: Address,
        amount: U256,
        options: Option<TxOptions>,
    ) -> Result<TxResult, AppError> {
        let ctx = TxContext {
            to,
            value: amount,
            data: Bytes::default(), // ETH 转账 data 为空
            options: options.unwrap_or_default(),
        };

        log_info!("发起 ETH 转账: 目标 {:?}, 金额 {}", to, amount);
        self.execute(ctx).await
    }

    /// 核心集成：ERC20 代币转账
    pub async fn erc20_transfer(
        &self,
        token_address: Address, // ERC20 合约地址
        to: Address,            // 接收者地址
        amount: U256,           // 转账金额（注意精度）
        options: Option<TxOptions>,
    ) -> Result<TxResult, AppError> {

        // 1. 构造标准 ERC20 transfer 函数的选择器 (0xa9059cbb)
        let selector = &keccak256("transfer(address,uint256)")[..4];

        // 2. 编码参数：address (to) 和 uint256 (amount)
        // 每个参数占 32 字节，总计 4 + 32 + 32 = 68 字节
        let mut data = Vec::with_capacity(68);
        data.extend_from_slice(selector);
        data.extend_from_slice(&ethers::abi::encode(&[
            ethers::abi::Token::Address(to),
            ethers::abi::Token::Uint(amount),
        ]));

        // 3. 构建交易上下文
        // 注意：ERC20 转账的 to 是合约地址，value 通常为 0
        let ctx = TxContext {
            to: token_address,
            value: U256::zero(),
            data: data.into(),
            options: options.unwrap_or_default(),
        };

        log_info!("正在发起 ERC20 转账: 代币 {:?}, 目标 {:?}, 金额 {}", token_address, to, amount);

        // 4. 调用现有的 execute 流程
        // 这将自动享受您实现的：模拟预执行、Nonce 管理、Gas 计算、签名及广播
        self.execute(ctx).await
    }


    async fn execute(&self, ctx: TxContext) -> Result<TxResult, AppError> {
        // 1. 预执行模拟
        self.simulation.run(&ctx, &*self.provider).await?;

        // 2. 获取动态费用
        let (max_fee_per_gas, priority_fee_per_gas) = self
            .gas_svc
            .resolve_fees(&*self.provider, ctx.options.priority)
            .await?;

        // 3. 预占 nonce
        let nonce = self.nonce_svc.acquire();

        // 4. 构建交易
        let mut tx_req = Eip1559TransactionRequest::new()
            .to(ctx.to)
            .value(ctx.value)
            .data(ctx.data)
            .max_fee_per_gas(max_fee_per_gas)
            .max_priority_fee_per_gas(priority_fee_per_gas)
            .nonce(nonce);

        if let Some(chain_id) = self.signer.chain_id() {
            tx_req = tx_req.chain_id(chain_id);
        }

        // 5. 估算 Gas Limit + Buffer
        let estimated_gas = self
            .provider
            .estimate_gas(&TypedTransaction::Eip1559(tx_req.clone()))
            .await
            .map_err(|e| {
                self.nonce_svc.rollback();
                AppError::Internal(format!("Gas estimation failed: {}", e))
            })?;

        let gas_limit = estimated_gas * ctx.options.gas_limit_buffer / 100;
        tx_req = tx_req.gas(gas_limit);

        // 6. 签名
        let typed_tx: TypedTransaction = tx_req.into();
        let signature = self.signer.sign_tx(&typed_tx).await.map_err(|e| {
            self.nonce_svc.rollback();
            e
        })?;

        let signed_rlp = typed_tx.rlp_signed(&signature);

        // 7. 广播
        let receipt_tx = self
            .provider
            .send_raw_transaction(
                signed_rlp,
                ctx.options.timeout_secs,
                ctx.options.confirmations as usize,
            )
            .await
            .map_err(|e| {
                self.nonce_svc.rollback();
                e
            })?;

        // 解析所有的 Transfer 事件
        let transfers: Vec<TransferEvent> = parse_logs_from_receipt(&receipt_tx);
        for tx in transfers {
            log_info!(
                "成功转账: 从 {:?} 到 {:?}, 金额: {}",
                tx.from,
                tx.to,
                tx.value
            );
        }
        Ok(TxResult {
            tx_hash: receipt_tx.transaction_hash,
            receipt: receipt_tx,
        })
    }
}

/// 通用解析函数：从 Receipt 中提取特定的事件
pub fn parse_logs_from_receipt<T: EthEvent>(receipt: &TransactionReceipt) -> Vec<T> {
    receipt
        .logs
        .iter()
        .filter_map(|log| {
            // 将 ethers 内部 log 转换为 RawLog
            let raw_log = RawLog {
                topics: log.topics.clone(),
                data: log.data.to_vec(),
            };
            // 尝试解析
            T::decode_log(&raw_log).ok()
        })
        .collect()
}
