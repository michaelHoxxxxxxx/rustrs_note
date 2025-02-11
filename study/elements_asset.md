# elements/asset.rs 学习笔记

## 文件概述
`asset.rs` 是 electrs 项目中 Liquid 资产的核心实现文件。该文件实现了资产的发行、追踪、统计和查询等功能，支持原生资产（比特币）和发行资产两种类型。

## 主要组件分析

### 1. 常量定义
```rust
lazy_static! {
    // 原生资产 ID（比特币）
    pub static ref NATIVE_ASSET_ID: AssetId =
        "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d"
            .parse()
            .unwrap();
    // 测试网原生资产 ID
    pub static ref NATIVE_ASSET_ID_TESTNET: AssetId =
        "144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49"
            .parse()
            .unwrap();
    // 回归测试网原生资产 ID
    pub static ref NATIVE_ASSET_ID_REGTEST: AssetId =
        "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225"
            .parse()
            .unwrap();
}
```

### 2. 资产类型定义
```rust
// Liquid 资产枚举
#[derive(Serialize)]
#[serde(untagged)]
pub enum LiquidAsset {
    Issued(IssuedAsset),    // 发行资产
    Native(PeggedAsset),    // 原生资产（锚定比特币）
}

// 锚定资产结构
#[derive(Serialize)]
pub struct PeggedAsset {
    pub asset_id: AssetId,
    pub chain_stats: PeggedAssetStats,    // 链上统计
    pub mempool_stats: PeggedAssetStats,  // 内存池统计
}

// 发行资产结构
#[derive(Serialize)]
pub struct IssuedAsset {
    pub asset_id: AssetId,
    pub issuance_txin: TxInput,           // 发行交易输入
    pub issuance_prevout: OutPoint,       // 发行前序输出
    pub reissuance_token: AssetId,        // 再发行令牌
    pub contract_hash: Option<ContractHash>, // 合约哈希
    pub status: TransactionStatus,        // 交易状态
    pub chain_stats: IssuedAssetStats,    // 链上统计
    pub mempool_stats: IssuedAssetStats,  // 内存池统计
    pub meta: Option<AssetMeta>,          // 元数据
}
```

### 3. 资产统计结构
```rust
// 发行资产统计
pub struct IssuedAssetStats {
    pub tx_count: usize,              // 交易数量
    pub issuance_count: usize,        // 发行次数
    pub issued_amount: u64,           // 发行总量
    pub burned_amount: u64,           // 销毁总量
    pub has_blinded_issuances: bool,  // 是否有混淆发行
    pub reissuance_tokens: Option<u64>, // 再发行令牌数量
    pub burned_reissuance_tokens: u64,  // 销毁的再发行令牌
}

// 锚定资产统计
pub struct PeggedAssetStats {
    pub tx_count: usize,          // 交易数量
    pub peg_in_count: usize,      // 转入次数
    pub peg_in_amount: u64,       // 转入总量
    pub peg_out_count: usize,     // 转出次数
    pub peg_out_amount: u64,      // 转出总量
    pub burn_count: usize,        // 销毁次数
    pub burned_amount: u64,       // 销毁总量
}
```

### 4. 核心功能实现
```rust
impl LiquidAsset {
    // 计算资产供应量
    pub fn supply(&self) -> Option<u64> {
        match self {
            // 原生资产：转入 - 转出 - 销毁
            LiquidAsset::Native(asset) => Some(
                asset.chain_stats.peg_in_amount
                    - asset.chain_stats.peg_out_amount
                    - asset.chain_stats.burned_amount
                    + asset.mempool_stats.peg_in_amount
                    - asset.mempool_stats.peg_out_amount
                    - asset.mempool_stats.burned_amount,
            ),
            // 发行资产：发行 - 销毁（如果有混淆发行则返回 None）
            LiquidAsset::Issued(asset) => {
                if asset.chain_stats.has_blinded_issuances {
                    None
                } else {
                    Some(
                        asset.chain_stats.issued_amount 
                        - asset.chain_stats.burned_amount
                        + asset.mempool_stats.issued_amount
                        - asset.mempool_stats.burned_amount,
                    )
                }
            }
        }
    }

    // 获取资产精度
    pub fn precision(&self) -> u8 {
        match self {
            LiquidAsset::Native(_) => 8,  // 比特币精度
            LiquidAsset::Issued(asset) => asset.meta.as_ref().map_or(0, |m| m.precision),
        }
    }
}
```

## 核心功能解析

1. **资产类型系统**：
   - 原生资产（锚定比特币）
   - 发行资产（自定义资产）
   - 支持再发行机制

2. **统计追踪**：
   - 链上和内存池分离统计
   - 发行和销毁追踪
   - 转入转出记录

3. **隐私特性**：
   - 混淆发行支持
   - 合约哈希
   - 再发行令牌

4. **索引功能**：
   - 确认交易索引
   - 内存池交易索引
   - 历史记录维护

## 设计要点

1. **数据结构**：
   - 清晰的类型层次
   - 完整的统计信息
   - 灵活的元数据支持

2. **功能完整性**：
   - 发行管理
   - 统计追踪
   - 查询支持

3. **性能考虑**：
   - 缓存机制
   - 批量处理
   - 内存池优化

4. **安全性**：
   - 类型安全
   - 错误处理
   - 状态验证

## 使用示例

```rust
// 1. 查询资产信息
let asset = lookup_asset(&query, registry, &asset_id, None)?;

// 2. 获取资产供应量
if let Some(supply) = asset.supply() {
    println!("Asset supply: {}", supply);
}

// 3. 索引确认交易
index_confirmed_tx_assets(
    &tx,
    confirmed_height,
    network,
    parent_network,
    &mut rows
);

// 4. 处理内存池交易
index_mempool_tx_assets(
    &tx,
    network,
    parent_network,
    &mut asset_history,
    &mut asset_issuance
);
``` 