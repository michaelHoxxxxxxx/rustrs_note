# electrum/discovery.rs 学习笔记

## 文件概述
`discovery.rs` 是 electrs 项目中实现 Electrum 服务器发现机制的核心文件。该文件实现了服务器健康检查、对等节点发现和服务器状态管理等功能。

## 主要组件分析

### 1. 常量定义
```rust
// 定义了各种时间间隔和限制常量
const HEALTH_CHECK_FREQ: Duration = Duration::from_secs(3600);    // 每小时检查一次服务器
const JOB_INTERVAL: Duration = Duration::from_secs(1);           // 每秒运行一个健康检查任务
const MAX_CONSECUTIVE_FAILURES: usize = 24;                      // 24次连续失败后删除服务器
const MAX_QUEUE_SIZE: usize = 500;                              // 健康检查队列最大大小
const MAX_SERVERS_PER_REQUEST: usize = 3;                       // 每次请求最多添加的服务器数
const MAX_SERVICES_PER_REQUEST: usize = 6;                      // 每次请求最多添加的服务数
```

### 2. 核心数据结构

#### DiscoveryManager
```rust
pub struct DiscoveryManager {
    // BinaryHeap 是一个优先队列，用于调度健康检查任务
    queue: RwLock<BinaryHeap<HealthCheck>>,     // 使用读写锁保护的健康检查队列
    
    // HashMap 存储健康的服务器信息
    healthy: RwLock<HashMap<ServerAddr, Server>>, // 健康服务器列表
    
    our_version: ProtocolVersion,               // 本地服务器版本
    our_addrs: HashSet<ServerAddr>,            // 本地服务器地址集合
    our_features: ServerFeatures,              // 本地服务器特性
    announce: bool,                            // 是否向其他服务器公布自己
    tor_proxy: Option<SocketAddr>,             // Tor 代理地址（可选）
}
```

#### Server 和 ServerAddr
```rust
// 表示一个 Electrum 服务器
struct Server {
    services: HashSet<Service>,                // 服务集合（TCP/SSL）
    hostname: Hostname,                        // 主机名
    features: ServerFeatures,                  // 服务器特性
}

// 服务器地址枚举
enum ServerAddr {
    Clearnet(IpAddr),                         // 普通 IP 地址
    Onion(Hostname),                          // Tor 洋葱地址
}
```

### 3. 健康检查机制

#### HealthCheck 结构
```rust
struct HealthCheck {
    addr: ServerAddr,                         // 服务器地址
    hostname: Hostname,                       // 主机名
    service: Service,                         // 服务类型（TCP/SSL）
    is_default: bool,                         // 是否是默认服务器
    added_by: Option<IpAddr>,                // 添加该服务器的节点
    last_check: Option<Instant>,             // 上次检查时间
    last_healthy: Option<Instant>,           // 上次健康时间
    consecutive_failures: usize,             // 连续失败次数
}
```

### 4. 主要功能实现

#### 服务器管理
```rust
impl DiscoveryManager {
    // 创建新的发现管理器
    pub fn new(
        our_network: Network,
        our_features: ServerFeatures,
        our_version: ProtocolVersion,
        announce: bool,
        tor_proxy: Option<SocketAddr>,
    ) -> Self { ... }

    // 添加新的服务器请求
    pub fn add_server_request(&self, added_by: IpAddr, features: ServerFeatures) -> Result<()> { ... }

    // 添加默认服务器
    pub fn add_default_server(&self, hostname: Hostname, services: Vec<Service>) -> Result<()> { ... }

    // 获取健康服务器列表
    pub fn get_servers(&self) -> Vec<ServerEntry> { ... }
}
```

### 5. 健康检查实现
```rust
impl DiscoveryManager {
    // 运行健康检查
    fn run_health_check(&self) -> Result<()> {
        // 检查队列中的下一个任务是否准备好执行
        if self.queue.read().unwrap().peek().map_or(true, |next| {
            next.last_check
                .map_or(false, |t| t.elapsed() < HEALTH_CHECK_FREQ)
        }) {
            return Ok(());
        }

        // 获取并执行健康检查任务
        let mut job = self.queue.write().unwrap().pop().unwrap();
        match self.check_server(&job.addr, &job.hostname, job.service) {
            Ok(features) => {
                // 处理健康的服务器
                if !was_healthy {
                    self.save_healthy_service(&job, features);
                }
                
                // 更新检查状态
                job.last_check = Some(Instant::now());
                job.last_healthy = job.last_check;
                job.consecutive_failures = 0;
                
                // 重新加入队列
                self.queue.write().unwrap().push(job);
                Ok(())
            }
            Err(e) => {
                // 处理不健康的服务器
                if was_healthy {
                    self.remove_unhealthy_service(&job);
                }

                // 更新失败状态
                job.last_check = Some(Instant::now());
                job.consecutive_failures += 1;

                // 决定是否继续重试
                if job.should_retry() {
                    self.queue.write().unwrap().push(job);
                }
                Err(e)
            }
        }
    }
}
```

### 6. 服务器验证
```rust
impl DiscoveryManager {
    // 验证服务器兼容性
    fn verify_compatibility(&self, features: &ServerFeatures) -> Result<()> {
        // 检查创世区块哈希是否匹配
        ensure!(
            features.genesis_hash == self.our_features.genesis_hash,
            "incompatible networks"
        );

        // 检查协议版本兼容性
        ensure!(
            features.protocol_min <= self.our_version && features.protocol_max >= self.our_version,
            "incompatible protocol versions"
        );

        // 检查哈希函数
        ensure!(
            features.hash_function == "sha256",
            "incompatible hash function"
        );

        Ok(())
    }
}
```

### 7. 服务器地址处理
```rust
impl ServerAddr {
    // 解析服务器地址
    fn resolve(host: &str) -> Result<Self> {
        Ok(if host.ends_with(".onion") {
            ServerAddr::Onion(host.into())
        } else if let Ok(ip) = IpAddr::from_str(host) {
            ServerAddr::Clearnet(ip)
        } else {
            // 尝试 DNS 解析
            let ip = format!("{}:1", host)
                .to_socket_addrs()?
                .next()?
                .ip();
            ServerAddr::Clearnet(ip)
        })
    }
}

// 检查是否为远程地址
fn is_remote_addr(addr: &ServerAddr) -> bool {
    match addr {
        ServerAddr::Onion(_) => true,
        ServerAddr::Clearnet(ip) => {
            !ip.is_loopback()
                && !ip.is_unspecified()
                && !ip.is_multicast()
                && !match ip {
                    IpAddr::V4(ipv4) => ipv4.is_private(),
                    IpAddr::V6(_) => false,
                }
        }
    }
}
```

### 8. 测试实现
```rust
#[cfg(test)]
mod tests {
    // 测试基本功能
    #[test]
    fn test() -> Result<()> {
        // 创建测试用的服务器特性
        let features = ServerFeatures {
            hosts: serde_json::from_str("{\"test.foobar.example\":{\"tcp_port\":60002}}").unwrap(),
            server_version: format!("electrs-esplora 9"),
            genesis_hash: genesis_hash(Network::Testnet),
            protocol_min: PROTOCOL_VERSION,
            protocol_max: PROTOCOL_VERSION,
            hash_function: "sha256".into(),
            pruning: None,
        };

        // 创建发现管理器
        let discovery = Arc::new(DiscoveryManager::new(
            Network::Testnet,
            features,
            PROTOCOL_VERSION,
            false,
            None,
        ));

        // 添加测试服务器
        discovery.add_default_server(
            "electrum.blockstream.info".into(),
            vec![Service::Tcp(60001)]
        )?;

        // 运行健康检查
        for _ in 0..12 {
            discovery.run_health_check().ok();
            thread::sleep(time::Duration::from_secs(1));
        }

        Ok(())
    }
}
```

## 高级功能实现

1. **健康检查机制**：
   - 定期检查服务器状态
   - 跟踪连续失败次数
   - 自动重试机制
   - 优先级队列调度

2. **服务器验证**：
   - 网络兼容性检查
   - 协议版本验证
   - 哈希函数验证
   - 特性兼容性检查

3. **地址处理**：
   - 支持 Tor 洋葱地址
   - DNS 解析支持
   - IP 地址验证
   - 私有网络过滤

4. **测试覆盖**：
   - 集成测试
   - 网络功能测试
   - 服务器管理测试
   - 错误处理测试

## 安全考虑

1. **网络安全**：
   - 过滤私有网络地址
   - 验证远程服务器
   - Tor 网络支持
   - 协议兼容性检查

2. **并发安全**：
   - 使用读写锁保护共享状态
   - 原子操作保证
   - 线程安全的数据结构
   - 死锁预防

3. **错误处理**：
   - 优雅的错误恢复
   - 详细的错误信息
   - 重试机制
   - 资源清理 