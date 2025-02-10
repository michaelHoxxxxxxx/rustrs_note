# daemon.rs 学习笔记

## 文件概述
`daemon.rs` 是 electrs 项目的守护进程通信实现文件。该文件负责与比特币/Liquid 守护进程进行 RPC 通信，处理区块链数据的获取和交互。

## 主要组件分析

### 1. 配置和常量
```rust
// 使用 lazy_static 宏来定义静态变量，这些变量会在第一次使用时才初始化
lazy_static! {
    // 从环境变量读取连接超时时间，如果未设置则默认为10秒
    static ref DAEMON_CONNECTION_TIMEOUT: Duration = Duration::from_secs(
        env::var("DAEMON_CONNECTION_TIMEOUT").map_or(10, |s| s.parse().unwrap())
    );
    // 从环境变量读取读取超时时间，如果未设置则默认为10分钟
    static ref DAEMON_READ_TIMEOUT: Duration = Duration::from_secs(
        env::var("DAEMON_READ_TIMEOUT").map_or(10 * 60, |s| s.parse().unwrap())
    );
    // 从环境变量读取写入超时时间，如果未设置则默认为10分钟
    static ref DAEMON_WRITE_TIMEOUT: Duration = Duration::from_secs(
        env::var("DAEMON_WRITE_TIMEOUT").map_or(10 * 60, |s| s.parse().unwrap())
    );
}

// 定义最大重试次数为5次
const MAX_ATTEMPTS: u32 = 5;
// 定义重试等待时间为1秒
const RETRY_WAIT_DURATION: Duration = Duration::from_secs(1);
```

### 2. 数据结构定义
```rust
// 使用派生宏自动实现序列化、反序列化和调试输出功能
#[derive(Serialize, Deserialize, Debug)]
pub struct BlockchainInfo {
    pub chain: String,                     // 链的名称（例如：mainnet, testnet）
    pub blocks: u32,                       // 当前区块数量
    pub headers: u32,                      // 当前区块头数量
    pub bestblockhash: String,            // 最新区块的哈希值
    pub pruned: bool,                     // 是否为修剪节点
    pub verificationprogress: f32,        // 区块验证进度（0.0-1.0）
    pub initialblockdownload: Option<bool>, // 是否正在进行初始区块下载
}

// 网络信息结构体，用于存储比特币节点的版本信息
#[derive(Serialize, Deserialize, Debug)]
struct NetworkInfo {
    version: u64,        // 节点版本号（例如：220000）
    subversion: String,  // 节点子版本（例如："/Satoshi:0.22.0/"）
    relayfee: f64,      // 交易中继费用（BTC/kB）
}

// 连接结构体，管理与比特币节点的TCP连接
struct Connection {
    tx: TcpStream,                         // TCP发送流，用于发送数据
    rx: Lines<BufReader<TcpStream>>,       // TCP接收流，使用BufReader按行读取
    cookie_getter: Arc<dyn CookieGetter>,  // Cookie获取器，用于RPC认证
    addr: SocketAddr,                      // 比特币节点的地址
    signal: Waiter,                        // 信号等待器，用于处理中断
}

// 守护进程结构体，管理与比特币节点的所有交互
pub struct Daemon {
    daemon_dir: PathBuf,                   // 守护进程工作目录
    blocks_dir: PathBuf,                   // 区块数据存储目录
    network: Network,                      // 网络类型（主网/测试网等）
    conn: Mutex<Connection>,               // 线程安全的连接实例
    message_id: Counter,                   // RPC消息ID计数器
    signal: Waiter,                        // 信号等待器
    rpc_threads: Arc<rayon::ThreadPool>,   // RPC请求线程池
    latency: HistogramVec,                 // RPC延迟监控指标
    size: HistogramVec,                    // RPC数据大小监控指标
}
```

### 3. 区块相关 RPC 方法实现
```rust
impl Daemon {
    // 获取指定哈希值的区块头
    pub fn getblockheader(&self, blockhash: &BlockHash) -> Result<BlockHeader> {
        // 发送 getblockheader RPC请求，verbose=false表示返回原始格式
        header_from_value(self.request("getblockheader", json!([blockhash, false]))?)
    }

    // 批量获取多个区块头
    pub fn getblockheaders(&self, heights: &[usize]) -> Result<Vec<BlockHeader>> {
        // 将区块高度转换为JSON参数数组
        let heights: Vec<Value> = heights.iter().map(|height| json!([height])).collect();
        
        // 首先获取所有高度对应的区块哈希
        let params_list: Vec<Value> = self
            .requests("getblockhash", heights)?  // 批量请求区块哈希
            .into_iter()
            .map(|hash| json!([hash, false]))   // 为每个哈希创建getblockheader参数
            .collect();
        
        // 然后获取所有区块头
        let mut result = vec![];
        for h in self.requests("getblockheader", params_list)? {
            result.push(header_from_value(h)?);  // 解析每个区块头
        }
        Ok(result)
    }

    // 获取完整的区块数据
    pub fn getblock(&self, blockhash: &BlockHash) -> Result<Block> {
        // 发送 getblock RPC请求获取区块数据
        let block = block_from_value(
            self.request("getblock", json!([blockhash, false]))?
        )?;
        // 验证返回的区块哈希是否正确
        assert_eq!(block.block_hash(), *blockhash);
        Ok(block)
    }

    // 批量获取多个区块，包含重试机制
    pub fn getblocks(&self, blockhashes: &[BlockHash]) -> Result<Vec<Block>> {
        // 为每个区块哈希创建RPC参数
        let params_list: Vec<Value> = blockhashes
            .iter()
            .map(|hash| json!([hash, false]))
            .collect();

        // 重试循环
        let mut attempts = MAX_ATTEMPTS;
        let values = loop {
            attempts -= 1;
            match self.requests("getblock", params_list.clone()) {
                Ok(blocks) => break blocks,  // 成功获取所有区块
                Err(e) => {
                    let err_msg = format!("{e:?}");
                    if err_msg.contains("Block not found on disk") {
                        // 如果区块未找到，可能是因为节点还在索引
                        log::warn!("getblocks failing with: {e:?} trying {attempts} more time")
                    } else {
                        panic!("failed to get blocks from bitcoind: {}", err_msg);
                    }
                }
            }
            if attempts == 0 {
                panic!("failed to get blocks from bitcoind")
            }
            std::thread::sleep(RETRY_WAIT_DURATION);  // 等待一段时间后重试
        };
        
        // 解析所有区块数据
        let mut blocks = vec![];
        for value in values {
            blocks.push(block_from_value(value)?);
        }
        Ok(blocks)
    }
}
```

### 4. 并行请求处理
```rust
impl Daemon {
    // 并行发送多个RPC请求，返回一个并行迭代器
    fn requests_iter<'a>(
        &'a self,
        method: &'a str,
        params_list: Vec<Value>,
    ) -> impl ParallelIterator<Item = Result<Value>> + IndexedParallelIterator + 'a {
        // 在线程池中执行请求
        self.rpc_threads.install(move || {
            // 将参数列表转换为并行迭代器
            params_list.into_par_iter().map(move |params| {
                // 每个线程维护自己的守护进程实例，避免竞争
                thread_local!(static DAEMON_INSTANCE: OnceCell<Daemon> = OnceCell::new());

                // 在线程本地存储中获取或创建守护进程实例
                DAEMON_INSTANCE.with(|daemon| {
                    daemon
                        .get_or_init(|| self.retry_reconnect())  // 懒初始化
                        .retry_request(&method, &params)          // 发送请求
                })
            })
        })
    }

    // 带重试机制的RPC请求处理
    fn retry_request(&self, method: &str, params: &Value) -> Result<Value> {
        loop {
            match self.handle_request(method, &params) {
                // 如果是连接错误，尝试重新连接
                Err(e @ Error(ErrorKind::Connection(_), _)) => {
                    warn!("reconnecting to bitcoind: {}", e.display_chain());
                    self.signal.wait(Duration::from_secs(3), false)?;  // 等待3秒
                    let mut conn = self.conn.lock().unwrap();         // 获取连接锁
                    *conn = conn.reconnect()?;                        // 重新连接
                    continue;  // 继续重试
                }
                result => return result,  // 其他情况直接返回结果
            }
        }
    }
}
```

### 5. 初始化和同步
```rust
impl Daemon {
    pub fn new(
        daemon_dir: &PathBuf,
        blocks_dir: &PathBuf,
        daemon_rpc_addr: SocketAddr,
        daemon_parallelism: usize,
        cookie_getter: Arc<dyn CookieGetter>,
        network: Network,
        signal: Waiter,
        metrics: &Metrics,
    ) -> Result<Daemon> {
        let daemon = Daemon {
            daemon_dir: daemon_dir.clone(),
            blocks_dir: blocks_dir.clone(),
            network,
            conn: Mutex::new(Connection::new(
                daemon_rpc_addr,
                cookie_getter,
                signal.clone(),
            )?),
            message_id: Counter::new(),
            signal: signal.clone(),
            rpc_threads: Arc::new(
                rayon::ThreadPoolBuilder::new()
                    .num_threads(daemon_parallelism)
                    .thread_name(|i| format!("rpc-requests-{}", i))
                    .build()
                    .unwrap(),
            ),
            latency: metrics.histogram_vec(
                HistogramOpts::new("daemon_rpc", "Bitcoind RPC latency (in seconds)"),
                &["method"],
            ),
            size: metrics.histogram_vec(
                HistogramOpts::new("daemon_bytes", "Bitcoind RPC size (in bytes)"),
                &["method", "dir"],
            ),
        };

        // 检查版本兼容性
        let network_info = daemon.getnetworkinfo()?;
        if network_info.version < 16_00_00 {
            bail!(
                "{} is not supported - please use bitcoind 0.16+",
                network_info.subversion,
            )
        }

        // 检查区块链状态
        let blockchain_info = daemon.getblockchaininfo()?;
        if blockchain_info.pruned {
            bail!("pruned node is not supported (use '-prune=0' bitcoind flag)")
        }

        // 等待同步完成
        loop {
            let info = daemon.getblockchaininfo()?;
            if !info.initialblockdownload.unwrap_or(false) && info.blocks == info.headers {
                break;
            }
            warn!(
                "waiting for bitcoind sync to finish: {}/{} blocks, progress: {:.3}%",
                info.blocks,
                info.headers,
                info.verificationprogress * 100.0
            );
            signal.wait(Duration::from_secs(5), false)?;
        }
        Ok(daemon)
    }
}
```

## Rust 进阶概念解释

1. **并发控制**：
   - `Mutex<T>` 用于线程安全的共享状态
   - `Arc<T>` 用于跨线程共享所有权
   - `rayon::ThreadPool` 用于并行任务处理
   - `thread_local!` 用于线程本地存储

2. **错误处理链**：
   - `Result<T>` 用于错误传播
   - `chain_err()` 添加错误上下文
   - 自定义错误类型和处理
   - 重试机制的实现

3. **智能指针**：
   - `Arc` 原子引用计数
   - `Box` 堆内存分配
   - `OnceCell` 延迟初始化
   - `Mutex` 互斥访问

4. **trait 系统**：
   - `CookieGetter` trait 定义认证接口
   - `Send + Sync` 用于线程安全
   - trait 对象用于动态分发
   - `ParallelIterator` 并行迭代

5. **异步IO和并行处理**：
   - TCP流的读写操作
   - 超时处理
   - 缓冲读取
   - 并行请求处理

## 守护进程设计要点

1. **可靠性**：
   - 自动重连机制
   - 错误重试策略
   - 超时控制
   - 版本兼容性检查

2. **性能优化**：
   - 线程池并行处理
   - 连接复用
   - 批量请求处理
   - 线程本地缓存

3. **安全性**：
   - Cookie 认证
   - 线程安全保证
   - 错误处理机制
   - 状态同步控制

4. **监控和度量**：
   - RPC 延迟监控
   - 数据大小监控
   - 同步进度跟踪
   - 错误日志记录 