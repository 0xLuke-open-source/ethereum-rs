use crate::infrastructure::protocol::constants::ERC20_TRANSFER_TOPIC;
use crate::utils::format::u256_to_bigdecimal;
use crate::utils::u256_to_i64;
use bigdecimal::BigDecimal;
use ethers_core::types::{H160, Log, Transaction, TransactionReceipt, U256};

#[derive(Debug, Clone)]
pub struct Transfer {
    pub block_number: i64,
    pub tx_hash: String,
    pub from_address: String,
    pub to_address: String,
    pub amount: BigDecimal,
    pub contract_address: Option<String>,
    pub timestamp: i64,
    pub gas: BigDecimal,
    pub max_fee_per_gas: BigDecimal,
    pub status: i16,
    pub log_index: i64,
}
impl Transfer {
    pub fn new(
        block_number: i64,
        tx_hash: String,
        from_address: String,
        to_address: String,
        amount: BigDecimal,
        contract_address: Option<String>,
        timestamp: i64,
        gas: BigDecimal,
        max_fee_per_gas: BigDecimal,
        status: i16,
        log_index: i64,
    ) -> Self {
        Self {
            block_number,
            tx_hash,
            from_address,
            to_address,
            amount,
            contract_address,
            timestamp,
            gas,
            max_fee_per_gas,
            status,
            log_index,
        }
    }

    /// ETH 交易
    pub fn from_eth_tx(
        tx: &Transaction,
        receipt: &TransactionReceipt,
        block_number: i64,
        timestamp: i64,
        log_index: i64,
    ) -> Self {
        Self {
            block_number,
            tx_hash: format!("{:#x}", tx.hash),
            from_address: format!("{:#x}", tx.from),
            to_address: tx.to.map(|v| format!("{:#x}", v)).unwrap_or_default(),
            amount: u256_to_bigdecimal(tx.value),
            contract_address: None,
            timestamp,
            gas: u256_to_bigdecimal(tx.gas),
            max_fee_per_gas: tx
                .max_fee_per_gas
                .map(u256_to_bigdecimal)
                .unwrap_or_else(|| BigDecimal::from(0)),
            status: receipt.status.unwrap_or_default().as_u64() as i16,
            log_index,
        }
    }

    /// ERC20 交易
    pub fn from_erc20_log(
        tx: &Transaction,
        log: &Log,
        receipt: &TransactionReceipt,
        block_number: i64,
        tx_hash: String,
        timestamp: i64,
        amount: U256,
        log_index: i64,
    ) -> Self {
        Self {
            block_number,
            tx_hash,
            from_address: format!("{:#x}", H160::from(log.topics[1])),
            to_address: format!("{:#x}", H160::from(log.topics[2])),
            amount: u256_to_bigdecimal(amount),
            contract_address: Some(format!("{:#x}", log.address)),
            timestamp,
            gas: u256_to_bigdecimal(receipt.gas_used.unwrap_or_default()),
            max_fee_per_gas: tx
                .max_fee_per_gas
                .map(u256_to_bigdecimal)
                .unwrap_or_else(|| BigDecimal::from(0)),
            status: receipt.status.unwrap_or_default().as_u64() as i16,
            log_index,
        }
    }

    ///解析交易
    pub async fn process_transaction(
        tx: Transaction,
        receipt: TransactionReceipt,
        block_number: i64,
        block_timestamp: i64,
    ) -> Vec<Transfer> {
        let mut transfers = vec![];
        // ---------- ETH ----------
        if let Some(_) = tx.to {
            if !tx.value.is_zero() {
                transfers.push(Transfer::from_eth_tx(
                    &tx,
                    &receipt,
                    block_number,
                    block_timestamp,
                    0,
                ));
            }
        }

        // ---------- ERC20 ----------
        for log in receipt.logs.iter().filter(|log| {
            log.topics.len() == 3
                && log.topics[0] == *ERC20_TRANSFER_TOPIC
                && log.data.0.len() == 32
        }) {
            let value = U256::from_big_endian(&log.data.0);
            transfers.push(Transfer::from_erc20_log(
                &tx,
                log,
                &receipt,
                block_number,
                format!("{:#x}", tx.hash),
                block_timestamp,
                value,
                u256_to_i64(log.log_index.unwrap_or_default()).unwrap_or_default(),
            ));
        }

        transfers
    }
}
