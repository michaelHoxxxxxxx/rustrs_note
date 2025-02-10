# metrics.rs 学习笔记

## 文件概述
`metrics.rs` 是 electrs 项目的监控指标实现文件。该文件使用 Prometheus 库来收集和导出各种性能指标，包括内存使用、CPU 使用率和文件描述符数量等系统级指标。

## 主要组件分析

### 1. 导入和类型定义
```rust
// 导入外部 crate
use page_size;                    // 获取系统页面大小
use prometheus::{self, Encoder};  // Prometheus 监控库
use std::fs;                     // 文件系统操作
use std::io;                     // 输入输出操作
use std::net::SocketAddr;        // 网络地址类型
use std::thread;                 // 线程操作
use std::time::Duration;         // 时间间隔
use sysconf;                     // 系统配置信息
use tiny_http;                   // 轻量级 HTTP 服务器

// 从 prometheus 重新导出常用类型
pub use prometheus::{
    GaugeVec,           // 带标签的度量值
    Histogram,          // 直方图
    HistogramOpts,      // 直方图选项
    HistogramTimer,     // 直方图计时器
    HistogramVec,       // 带标签的直方图
    IntCounter as Counter,       // 整数计数器
    IntCounterVec as CounterVec, // 带标签的整数计数器
    IntGauge as Gauge,           // 整数度量值
    Opts as MetricOpts,          // 指标选项
};
```

### 2. 指标结构体定义
```rust
// 主要的指标结构体
pub struct Metrics {
    reg: prometheus::Registry,    // Prometheus 注册器，用于注册所有指标
    addr: SocketAddr,            // HTTP 服务器地址，用于暴露指标
}

// 系统统计信息结构体
struct Stats {
    utime: f64,     // 用户空间 CPU 时间
    rss: u64,       // 常驻内存大小
    fds: usize,     // 打开的文件描述符数量
}
```

### 3. Metrics 实现
```rust
impl Metrics {
    // 创建新的 Metrics 实例
    pub fn new(addr: SocketAddr) -> Metrics {
        Metrics {
            reg: prometheus::Registry::new(),
            addr,
        }
    }

    // 创建计数器类型的指标
    pub fn counter(&self, opts: prometheus::Opts) -> Counter {
        let c = Counter::with_opts(opts).unwrap();     // 创建计数器
        self.reg.register(Box::new(c.clone())).unwrap(); // 注册到注册器
        c   // 返回计数器
    }

    // 创建带标签的计数器
    pub fn counter_vec(&self, opts: prometheus::Opts, labels: &[&str]) -> CounterVec {
        let c = CounterVec::new(opts, labels).unwrap();
        self.reg.register(Box::new(c.clone())).unwrap();
        c
    }

    // 启动 HTTP 服务器暴露指标
    pub fn start(&self) {
        let server = tiny_http::Server::http(self.addr)
            .unwrap_or_else(|_| panic!("failed to start monitoring HTTP server at {}", self.addr));
        start_process_exporter(&self);  // 启动进程指标收集器
        let reg = self.reg.clone();     // 克隆注册器用于新线程
        spawn_thread("metrics", move || loop {  // 启动指标服务线程
            if let Err(e) = handle_request(&reg, server.recv()) {
                error!("http error: {}", e);
            }
        });
    }
}
```

### 4. 系统指标收集
```rust
// 解析进程统计信息
fn parse_stats() -> Result<Stats> {
    // 在 macOS 上返回空统计信息
    if cfg!(target_os = "macos") {
        return Ok(Stats {
            utime: 0f64,
            rss: 0u64,
            fds: 0usize,
        });
    }
    
    // 读取 /proc/self/stat 文件获取进程信息
    let value = fs::read_to_string("/proc/self/stat")
        .chain_err(|| "failed to read stats")?;
    let parts: Vec<&str> = value.split_whitespace().collect();
    
    // 获取系统配置信息
    let page_size = page_size::get() as u64;
    let ticks_per_second = sysconf::raw::sysconf(sysconf::raw::SysconfVariable::ScClkTck)
        .expect("failed to get _SC_CLK_TCK") as f64;

    // 解析统计信息
    let utime = parse_part(13, "utime")? as f64 / ticks_per_second;  // CPU 时间
    let rss = parse_part(23, "rss")? * page_size;                    // 内存使用
    let fds = fs::read_dir("/proc/self/fd")                         // 文件描述符数量
        .chain_err(|| "failed to read fd directory")?
        .count();
        
    Ok(Stats { utime, rss, fds })
}
```

## Rust 进阶概念解释

1. **条件编译**：
   - `cfg!(target_os = "macos")` 用于在编译时判断目标操作系统
   - 可以根据不同平台生成不同的代码
   - 常用于处理平台特定的功能

2. **错误处理链**：
   - `chain_err(|| "error message")` 用于添加错误上下文
   - 可以构建详细的错误信息链
   - 帮助定位错误来源

3. **智能指针**：
   - `Box<T>` 用于在堆上分配数据
   - 常用于需要动态分配或大小不确定的数据
   - 实现了 `Deref` trait

4. **线程安全**：
   - `Clone` trait 用于在线程间共享数据
   - 使用 `move` 闭包转移所有权
   - 线程安全的数据共享

5. **系统编程特性**：
   - 文件系统操作
   - 进程统计信息收集
   - 系统配置查询

## 监控系统设计要点

1. **可扩展性**：
   - 支持多种指标类型
   - 可自定义标签
   - 模块化的指标收集

2. **性能考虑**：
   - 异步指标收集
   - 定期更新
   - 最小化系统开销

3. **可用性**：
   - HTTP 接口暴露指标
   - 标准的 Prometheus 格式
   - 错误处理和恢复机制 