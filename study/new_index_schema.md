# new_index/schema.rs 学习笔记

## 文件概述
`schema.rs` 是 electrs 项目中索引系统的核心文件，负责定义数据库模式、存储结构和索引操作。该文件实现了区块链数据的存储和检索功能，是整个索引系统的基础。

## 主要组件分析

### 1. 导入和依赖
```rust
// 比特币相关导入
use bitcoin::hashes::sha256d::Hash as Sha256dHash;
use bitcoin::hex::FromHex;

// 加密相关
use crypto::digest::Digest;
use crypto::sha2::Sha256;

// 并行计算
use rayon::prelude::*;

// 标准库
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, RwLock};
```

### 2. 核心数据结构

#### Store 结构体
```rust
pub struct Store {
    txstore_db: DB,                                    // 交易存储数据库
    history_db: DB,                                    // 历史记录数据库
    cache_db: DB,                                      // 缓存数据库
    added_blockhashes: RwLock<HashSet<BlockHash>>,     // 已添加区块哈希集合
    indexed_blockhashes: RwLock<HashSet<BlockHash>>,   // 已索引区块哈希集合
    indexed_headers: RwLock<HeaderList>,               // 已索引区块头列表
}
```

#### Utxo 结构体
```rust
pub struct Utxo {
    pub txid: Txid,                 // 交易ID
    pub vout: u32,                 // 输出索引
    pub confirmed: Option<BlockId>, // 确认区块信息
    pub value: Value,              // 金额

    #[cfg(feature = "liquid")]     // Liquid 特有字段
    pub asset: confidential::Asset,
    pub nonce: confidential::Nonce,
    pub witness: elements::TxOutWitness,
}
```

#### ScriptStats 结构体
```rust
#[derive(Serialize, Deserialize, Debug)]
pub struct ScriptStats {
    pub tx_count: usize,           // 交易数量
    pub funded_txo_count: usize,   // 资金交易输出数量
    pub spent_txo_count: usize,    // 已花费交易输出数量
    #[cfg(not(feature = "liquid"))]
    pub funded_txo_sum: u64,       // 资金总额
    pub spent_txo_sum: u64,        // 已花费总额
}
```

### 3. 索引器实现

#### Indexer 结构体
```rust
pub struct Indexer {
    store: Arc<Store>,             // 存储实例
    flush: DBFlush,                // 数据库刷新控制
    from: FetchFrom,               // 数据获取来源
    iconfig: IndexerConfig,        // 索引器配置
    duration: HistogramVec,        // 性能指标
    tip_metric: Gauge,             // 链尖高度指标
}
```

#### IndexerConfig 结构体
```rust
struct IndexerConfig {
    light_mode: bool,              // 轻量模式
    address_search: bool,          // 地址搜索功能
    index_unspendables: bool,      // 索引不可花费输出
    network: Network,              // 网络类型
    #[cfg(feature = "liquid")]
    parent_network: crate::chain::BNetwork,  // Liquid父网络
}
```

## 主要功能

1. **数据存储管理**：
   - 交易数据存储
   - 历史记录管理
   - 缓存系统

2. **区块索引**：
   - 区块头索引
   - 交易索引
   - UTXO 集管理

3. **查询支持**：
   - 交易查询
   - 地址查询
   - 统计信息

4. **性能优化**：
   - 并行处理
   - 缓存机制
   - 数据压缩

## 设计特点

1. **模块化设计**：
   - 清晰的数据结构划分
   - 功能模块独立
   - 接口定义明确

2. **可扩展性**：
   - 支持 Liquid 侧链
   - 配置化的索引选项
   - 灵活的存储结构

3. **性能考虑**：
   - 使用读写锁保护共享数据
   - 支持并行处理
   - 优化的存储结构

4. **安全性**：
   - 严格的数据验证
   - 事务性操作
   - 错误处理机制 

## 核心功能实现

### 1. 索引器更新流程
```rust
pub fn update(&mut self, daemon: &Daemon) -> Result<BlockHash> {
    // 1. 重新连接守护进程并获取最新区块哈希
    let daemon = daemon.reconnect()?;
    let tip = daemon.getbestblockhash()?;
    
    // 2. 获取新的区块头
    let new_headers = self.get_new_headers(&daemon, &tip)?;

    // 3. 添加新区块的交易数据
    let to_add = self.headers_to_add(&new_headers);
    start_fetcher(self.from, &daemon, to_add)?.map(|blocks| self.add(&blocks));
    
    // 4. 索引新区块的历史记录
    let to_index = self.headers_to_index(&new_headers);
    start_fetcher(self.from, &daemon, to_index)?.map(|blocks| self.index(&blocks));
    
    // 5. 更新同步状态
    self.store.txstore_db.put_sync(b"t", &serialize(&tip));
    
    // 6. 更新索引头
    let mut headers = self.store.indexed_headers.write().unwrap();
    headers.apply(new_headers);
    
    Ok(tip)
}
```

### 2. 查询功能实现

#### 区块查询
```rust
impl ChainQuery {
    // 获取区块交易ID列表
    pub fn get_block_txids(&self, hash: &BlockHash) -> Option<Vec<Txid>> {
        let _timer = self.start_timer("get_block_txids");
        if self.light_mode {
            // 轻量模式：从 REST API 获取
            let mut blockinfo = self.daemon.getblock_raw(hash, 1).ok()?;
            Some(serde_json::from_value(blockinfo["tx"].take()).unwrap())
        } else {
            // 完整模式：从本地数据库获取
            self.store.txstore_db.get(&BlockRow::txids_key(full_hash(&hash[..])))
                .map(|val| bincode::deserialize_little(&val)
                    .expect("failed to parse block txids"))
        }
    }
    
    // 获取区块元数据
    pub fn get_block_meta(&self, hash: &BlockHash) -> Option<BlockMeta> {
        // 支持轻量模式和完整模式
    }
    
    // 获取原始区块数据
    pub fn get_block_raw(&self, hash: &BlockHash) -> Option<Vec<u8>> {
        // 完整的区块重建逻辑
    }
}
```

### 3. 历史记录查询

```rust
// 历史记录查询实现
pub fn history(
    &self,
    scripthash: &[u8],
    last_seen_txid: Option<&Txid>,
    limit: usize,
) -> Vec<(Transaction, BlockId)> {
    // 脚本哈希查询
    self._history(b'H', scripthash, last_seen_txid, limit)
}

// 通用历史记录查询
fn _history(
    &self,
    code: u8,
    hash: &[u8],
    last_seen_txid: Option<&Txid>,
    limit: usize,
) -> Vec<(Transaction, BlockId)> {
    // 实现分页查询和去重逻辑
}
```

## 性能优化技术

1. **数据库优化**：
   - 自动压缩功能
   - 批量写入操作
   - 缓存管理

2. **查询优化**：
   - 轻量模式支持
   - 交易ID索引
   - 区块元数据缓存

3. **并发处理**：
   - 读写锁保护
   - 异步区块获取
   - 并行数据处理

4. **内存管理**：
   - 智能指针（Arc）使用
   - 批量数据处理
   - 内存映射优化

## 错误处理和安全性

1. **错误处理**：
   - 使用 Result 类型
   - 详细的错误信息
   - 优雅的错误恢复

2. **数据一致性**：
   - 事务性写入
   - 同步点检查
   - 状态验证

3. **并发安全**：
   - 使用 RwLock 保护共享数据
   - 原子操作
   - 死锁预防 

## UTXO 管理和统计

### 1. UTXO 查询实现
```rust
pub fn utxo(&self, scripthash: &[u8], limit: usize) -> Result<Vec<Utxo>> {
    // 1. 尝试从缓存获取 UTXO 集合
    let cache: Option<(UtxoMap, usize)> = self.store
        .cache_db
        .get(&UtxoCacheRow::key(scripthash))
        .map(|c| bincode::deserialize_little(&c).unwrap())
        .and_then(|...| { /* 验证缓存有效性 */ });

    // 2. 更新 UTXO 集合
    let (newutxos, lastblock, processed_items) = cache.map_or_else(
        || self.utxo_delta(scripthash, HashMap::new(), 0, limit),
        |(oldutxos, blockheight)| 
            self.utxo_delta(scripthash, oldutxos, blockheight + 1, limit)
    )?;

    // 3. 更新缓存
    if let Some(lastblock) = lastblock {
        if had_cache || processed_items > MIN_HISTORY_ITEMS_TO_CACHE {
            self.store.cache_db.write(/* 缓存更新逻辑 */);
        }
    }

    // 4. 格式化输出
    Ok(newutxos.into_iter().map(|...| /* 转换为 Utxo 对象 */).collect())
}
```

### 2. UTXO 增量更新
```rust
fn utxo_delta(
    &self,
    scripthash: &[u8],
    init_utxos: UtxoMap,
    start_height: usize,
    limit: usize,
) -> Result<(UtxoMap, Option<BlockHash>, usize)> {
    // 1. 获取历史记录迭代器
    let history_iter = self.history_iter_scan(b'H', scripthash, start_height)
        .map(TxHistoryRow::from_row)
        .filter_map(|history| /* 获取确认区块 */);

    // 2. 更新 UTXO 集合
    let mut utxos = init_utxos;
    for (history, blockid) in history_iter {
        match history.key.txinfo {
            TxHistoryInfo::Funding(ref info) => {
                // 添加新的 UTXO
                utxos.insert(history.get_funded_outpoint(), 
                           (blockid, info.value))
            }
            TxHistoryInfo::Spending(_) => {
                // 移除已花费的 UTXO
                utxos.remove(&history.get_funded_outpoint())
            }
            // Liquid 特有的交易类型处理
            #[cfg(feature = "liquid")]
            TxHistoryInfo::Issuing(_) |
            TxHistoryInfo::Burning(_) |
            TxHistoryInfo::Pegin(_) |
            TxHistoryInfo::Pegout(_) => unreachable!(),
        };
    }

    Ok((utxos, lastblock, processed_items))
}
```

### 3. 统计功能实现
```rust
pub fn stats(&self, scripthash: &[u8]) -> ScriptStats {
    // 1. 从缓存获取统计信息
    let cache: Option<(ScriptStats, usize)> = /* 缓存获取逻辑 */;

    // 2. 更新统计信息
    let (newstats, lastblock) = cache.map_or_else(
        || self.stats_delta(scripthash, ScriptStats::default(), 0),
        |(oldstats, blockheight)| 
            self.stats_delta(scripthash, oldstats, blockheight + 1)
    );

    // 3. 更新缓存
    if let Some(lastblock) = lastblock {
        if newstats.funded_txo_count + newstats.spent_txo_count 
           > MIN_HISTORY_ITEMS_TO_CACHE {
            self.store.cache_db.write(/* 缓存更新逻辑 */);
        }
    }

    newstats
}
```

## 缓存优化策略

1. **分层缓存设计**：
   - UTXO 缓存
   - 统计信息缓存
   - 区块数据缓存

2. **缓存更新策略**：
   - 增量更新
   - 懒加载
   - 缓存失效检查

3. **性能优化**：
   - 最小缓存阈值
   - 批量更新
   - 并发访问控制

## Liquid 特性支持

1. **资产类型**：
   - 机密资产
   - 发行资产
   - Peg-in/Peg-out

2. **交易类型**：
   - 标准交易
   - 发行交易
   - 销毁交易
   - 双向锚定

3. **安全特性**：
   - 机密交易
   - 资产验证
   - 见证数据 