pub use eth_block::table as eth_block_db;
pub use eth_transfer::table as eth_transfer_db;

diesel::table! {
    /// 以太坊区块表
    eth_block (id) {
        /// 主键 ID
        id -> Int8,
        /// 区块号
        block_number -> Int8,
        /// 区块哈希
        block_hash -> Varchar,
        /// 父区块哈希
        parent_hash -> Varchar,
        /// Gas 使用量
        gas_used -> Numeric,
        /// 基础燃料费
        base_fee_per_gas -> Numeric,
        /// 创建时间
        created_at -> Nullable<Timestamp>,
        /// 区块时间戳
        timestamp -> Int8,
        /// 区块大小
        size -> Int4,
    }
}

diesel::table! {
    /// 以太坊交易转账表
    eth_transfer (id) {
        /// 主键 ID
        id -> Int8,
        /// 区块号
        block_number -> Int8,
        /// 交易哈希
        tx_hash -> Varchar,
        /// 发送方地址
        from_address -> Varchar,
        /// 接收方地址
        to_address -> Varchar,
        /// 转账金额
        amount -> Numeric,
        /// 合约地址
        contract_address -> Nullable<Varchar>,
        /// 时间戳
        timestamp -> Int8,
        /// Gas
        gas -> Numeric,
        /// 每个Gas的最大费用
        max_fee_per_gas -> Numeric,
        /// 状态 1=确认 2=确认中 3=失败
        status -> Int2,
        /// 创建时间
        created_at -> Nullable<Timestamp>,
        log_index -> Int8,
    }
}
