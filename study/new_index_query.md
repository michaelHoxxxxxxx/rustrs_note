# new_index/query.rs 学习笔记

## 文件概述
`query.rs` 是 electrs 项目中处理区块链查询的核心模块。该文件实现了与区块链数据查询相关的各种功能，包括交易查询、UTXO查询、费用估算等。

## 常量定义
```rust
const FEE_ESTIMATES_TTL: u64 = 60;    // 费用估算的缓存时间（秒）

// 确认目标区块数数组，用于费用估算
const CONF_TARGETS: [u16; 28] = [
    1u16, 2u16, 3u16, 4u16, 5u16, 6u16, 7u16, 8u16, 9u16, 10u16, 
    11u16, 12u16, 13u16, 14u16, 15u16, 16u16, 17u16, 18u16, 19u16, 
    20u16, 21u16, 22u16, 23u16, 24u16, 25u16, 144u16, 504u16, 1008u16,
];    // 从1个区块到1008个区块的确认目标
```

## 主要组件

### Query 结构体
```rust
pub struct Query {
    chain: Arc<ChainQuery>,         // 区块链查询接口（只读）
    mempool: Arc<RwLock<Mempool>>,  // 内存池访问
    daemon: Arc<Daemon>,            // 守护进程接口
    config: Arc<Config>,            // 配置信息
    cached_estimates: RwLock<(HashMap<u16, f64>, Option<Instant>)>,  // 费用估算缓存
    cached_relayfee: RwLock<Option<f64>>,  // 中继费用缓存
    #[cfg(feature = "liquid")]
    asset_db: Option<Arc<RwLock<AssetRegistry>>>,  // Liquid资产注册表（可选）
}
```

## 核心功能实现

### 1. 基础查询功能

#### 构造函数和基本访问器
```rust
impl Query {
    pub fn new(
        chain: Arc<ChainQuery>,
        mempool: Arc<RwLock<Mempool>>,
        daemon: Arc<Daemon>,
        config: Arc<Config>,
    ) -> Self {
        Query {
            chain,
            mempool,
            daemon,
            config,
            cached_estimates: RwLock::new((HashMap::new(), None)),
            cached_relayfee: RwLock::new(None),
        }
    }

    // 访问器方法
    pub fn chain(&self) -> &ChainQuery { &self.chain }
    pub fn config(&self) -> &Config { &self.config }
    pub fn network(&self) -> Network { self.config.network_type }
    pub fn mempool(&self) -> RwLockReadGuard<Mempool> { 
        self.mempool.read().unwrap() 
    }
}
```

### 2. 交易相关查询

#### 广播原始交易
```rust
pub fn broadcast_raw(&self, txhex: &str) -> Result<Txid> {
    let txid = self.daemon.broadcast_raw(txhex)?;    // 广播交易
    let _ = self.mempool.write().unwrap()
        .add_by_txid(&self.daemon, txid);    // 添加到内存池
    Ok(txid)
}
```

#### UTXO查询
```rust
pub fn utxo(&self, scripthash: &[u8]) -> Result<Vec<Utxo>> {
    let mut utxos = self.chain.utxo(scripthash, self.config.utxos_limit)?;
    let mempool = self.mempool();
    // 移除已经在内存池中被花费的UTXO
    utxos.retain(|utxo| !mempool.has_spend(&OutPoint::from(utxo)));
    // 添加内存池中的UTXO
    utxos.extend(mempool.utxo(scripthash));
    Ok(utxos)
}
```

#### 交易历史查询
```rust
pub fn history_txids(&self, scripthash: &[u8], limit: usize) 
    -> Vec<(Txid, Option<BlockId>)> 
{
    // 获取已确认的交易
    let confirmed_txids = self.chain.history_txids(scripthash, limit);
    let confirmed_len = confirmed_txids.len();
    let confirmed_txids = confirmed_txids.into_iter()
        .map(|(tx, b)| (tx, Some(b)));

    // 获取内存池中的交易
    let mempool_txids = self.mempool()
        .history_txids(scripthash, limit - confirmed_len)
        .into_iter()
        .map(|tx| (tx, None));

    // 合并结果
    confirmed_txids.chain(mempool_txids).collect()
}
```

### 3. 费用估算功能

#### 获取费用估算
```rust
pub fn estimate_fee(&self, conf_target: u16) -> Option<f64> {
    // 对于regtest网络，直接返回中继费用
    if self.config.network_type.is_regtest() {
        return self.get_relayfee().ok();
    }

    // 检查缓存是否有效
    if let (ref cache, Some(cache_time)) = *self.cached_estimates.read().unwrap() {
        if cache_time.elapsed() < Duration::from_secs(FEE_ESTIMATES_TTL) {
            return cache.get(&conf_target).copied();
        }
    }

    // 更新费用估算缓存
    self.update_fee_estimates();
    self.cached_estimates.read().unwrap()
        .0.get(&conf_target)
        .copied()
}
```

#### 更新费用估算缓存
```rust
fn update_fee_estimates(&self) {
    match self.daemon.estimatesmartfee_batch(&CONF_TARGETS) {
        Ok(estimates) => {
            *self.cached_estimates.write().unwrap() = (
                estimates, 
                Some(Instant::now())
            );
        }
        Err(err) => {
            warn!("failed estimating feerates: {:?}", err);
        }
    }
}
```

## 设计要点

1. **并发控制**：
   - 使用 `Arc` 进行共享所有权
   - 使用 `RwLock` 保护共享数据
   - 适当的锁粒度控制

2. **缓存机制**：
   - 费用估算缓存
   - 中继费用缓存
   - TTL（生存时间）控制

3. **错误处理**：
   - 使用 Result 类型
   - 详细的错误信息
   - 优雅的错误传播

4. **性能优化**：
   - 并行处理（使用 rayon）
   - 缓存重要数据
   - 批量处理请求

## 使用示例

```rust
// 创建查询实例
let query = Query::new(chain, mempool, daemon, config);

// 查询UTXO
let utxos = query.utxo(&scripthash)?;

// 查询交易历史
let history = query.history_txids(&scripthash, 100);

// 广播交易
let txid = query.broadcast_raw(&raw_tx_hex)?;

// 估算费用
let fee_rate = query.estimate_fee(6);  // 6个区块确认的费率
```

## 注意事项

1. **并发安全**：
   - 正确使用读写锁
   - 避免死锁
   - 最小化锁持有时间

2. **缓存管理**：
   - 及时更新缓存
   - 处理缓存失效
   - 避免缓存穿透

3. **资源管理**：
   - 合理设置限制
   - 释放不需要的资源
   - 处理超时情况

4. **错误处理**：
   - 处理网络错误
   - 处理数据不一致
   - 提供有意义的错误信息 