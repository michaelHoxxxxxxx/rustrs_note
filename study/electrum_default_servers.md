# electrum/discovery/default_servers.rs 学习笔记

## 文件概述
`default_servers.rs` 是 electrs 项目中负责配置默认 Electrum 服务器列表的文件。该文件定义了不同网络（比特币主网、测试网、Liquid 等）的默认服务器，为服务发现机制提供初始连接点。

## 主要功能

### 1. 默认服务器配置
```rust
// 导入必要的类型和模块
use crate::chain::Network;                    // 网络类型枚举
use crate::electrum::discovery::{             // 从 discovery 模块导入
    DiscoveryManager,                         // 服务发现管理器
    Service                                   // 服务类型枚举
};

// 添加默认服务器的公共函数
pub fn add_default_servers(discovery: &DiscoveryManager, network: Network) {
    match network {
        // 比特币主网服务器配置
        #[cfg(not(feature = "liquid"))]       // 条件编译：非 Liquid 网络
        Network::Bitcoin => {
            // 添加 Tor 洋葱服务器
            discovery
                .add_default_server(
                    "3smoooajg7qqac2y.onion".into(),  // .onion 地址表示 Tor 服务器
                    vec![
                        Service::Tcp(50001),          // TCP 服务端口
                        Service::Ssl(50002)           // SSL 服务端口
                    ],
                )
                .ok();                               // 忽略可能的错误
                
            // 添加标准 Electrum 服务器
            discovery
                .add_default_server(
                    "electrum.hsmiths.com".into(),    // 标准域名
                    vec![
                        Service::Tcp(50001),          // 标准 TCP 端口
                        Service::Ssl(50002)           // 标准 SSL 端口
                    ],
                )
                .ok();
                
            // 添加特殊端口的服务器
            discovery
                .add_default_server(
                    "bitcoin.dragon.zone".into(),
                    vec![
                        Service::Tcp(50003),          // 非标准 TCP 端口
                        Service::Ssl(50004)           // 非标准 SSL 端口
                    ],
                )
                .ok();
        }

        // 测试网服务器配置
        #[cfg(not(feature = "liquid"))]
        Network::Testnet => {
            // 添加测试网 Tor 服务器
            discovery
                .add_default_server(
                    "hsmithsxurybd7uh.onion".into(),
                    vec![
                        Service::Tcp(53011),          // 测试网 TCP 端口
                        Service::Ssl(53012)           // 测试网 SSL 端口
                    ],
                )
                .ok();

            // 添加测试网标准服务器
            discovery
                .add_default_server(
                    "testnet.hsmiths.com".into(),
                    vec![
                        Service::Tcp(53011),
                        Service::Ssl(53012)
                    ],
                )
                .ok();
        }

        _ => (),                                     // 其他网络类型暂不支持
    }
}
```

### 2. 服务器类型定义
```rust
// 服务器配置结构体
#[derive(Clone, Debug)]
struct ServerConfig {
    hostname: String,        // 服务器主机名（域名或 IP）
    services: Vec<Service>,  // 该服务器支持的服务列表
}

// 服务类型枚举
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum Service {
    Tcp(Port),              // TCP 服务，包含端口号
    Ssl(Port),              // SSL 服务，包含端口号
}

// 端口类型别名
type Port = u16;            // 使用 16 位无符号整数表示端口号
```

## 服务器分类

### 1. 主网服务器
```rust
// 主要类型：
// 1. 标准 Electrum 服务器
discovery.add_default_server(
    "electrum.hsmiths.com".into(),     // 知名的 Electrum 服务器
    vec![Service::Tcp(50001), Service::Ssl(50002)]  // 标准端口配置
);

// 2. Tor 洋葱服务器
discovery.add_default_server(
    "3smoooajg7qqac2y.onion".into(),   // .onion 地址
    vec![Service::Tcp(50001), Service::Ssl(50002)]  // 标准端口配置
);

// 3. 特殊端口服务器
discovery.add_default_server(
    "ecdsa.net".into(),                // 使用非标准端口的服务器
    vec![Service::Tcp(50001), Service::Ssl(110)]    // 自定义 SSL 端口
);
```

### 2. 测试网服务器
```rust
// 测试网配置示例
discovery.add_default_server(
    "testnet.hsmiths.com".into(),      // 测试网专用服务器
    vec![Service::Tcp(53011), Service::Ssl(53012)]  // 测试网标准端口
);
```

## 设计特点

1. **网络分类**：
   - 使用 `match` 语句根据网络类型选择服务器
   - 通过条件编译 `#[cfg(not(feature = "liquid"))]` 区分网络
   - 为每个网络维护独立的服务器列表

2. **服务多样性**：
   - 同时支持 TCP 和 SSL 服务
   - 混合使用标准端口和自定义端口
   - 支持 Tor 网络和普通网络

3. **错误处理**：
   - 使用 `ok()` 方法优雅处理错误
   - 单个服务器添加失败不影响整体功能
   - 保持服务的健壮性

4. **安全考虑**：
   - SSL 加密连接支持
   - Tor 网络匿名支持
   - 多样化的服务器选择

## 最佳实践

1. **服务器配置**：
   ```rust
   // 标准配置模式
   discovery.add_default_server(
       hostname,                        // 服务器域名
       vec![
           Service::Tcp(standard_port), // 标准 TCP 端口
           Service::Ssl(ssl_port)       // 标准 SSL 端口
       ]
   ).ok();                             // 错误处理
   ```

2. **端口约定**：
   - 主网：
     - TCP: 50001
     - SSL: 50002
   - 测试网：
     - TCP: 53011
     - SSL: 53012

3. **服务器选择标准**：
   - 地理位置分散
   - 支持多种协议
   - 稳定性记录良好
   - 社区信誉度高

## 使用示例

```rust
// 创建发现管理器并添加默认服务器
let discovery = DiscoveryManager::new(
    Network::Bitcoin,
    features,
    protocol_version,
    announce,
    tor_proxy
);

// 添加默认服务器
add_default_servers(&discovery, Network::Bitcoin);

// 开始服务发现过程
discovery.start();
``` 