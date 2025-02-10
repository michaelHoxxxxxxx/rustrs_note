# Electrs 源码学习笔记

本文档用于记录 Electrs 项目的源码学习过程。Electrs 是一个用 Rust 实现的高效比特币全节点索引服务器。

## 学习路线图

按照代码复杂度和依赖关系，推荐以下学习顺序：

### 第一阶段：基础设施和工具
1. `errors.rs` - 错误处理机制
2. `config.rs` - 配置系统
3. `metrics.rs` - 监控指标
4. `signal.rs` - 信号处理

### 第二阶段：核心概念
1. `lib.rs` - 项目入口和模块组织
2. `app.rs` - 应用程序结构
3. `chain.rs` - 区块链数据处理
4. `daemon.rs` - 后台服务实现

### 第三阶段：功能模块
1. `util/` - 工具函数和通用组件
2. `electrum/` - Electrum 协议实现
3. `new_index/` - 索引系统实现
4. `rest.rs` - REST API 实现

### 第四阶段：高级特性
1. `elements/` - Elements 侧链支持
2. `otlp_trace.rs` - OpenTelemetry 追踪
3. `bin/` - 可执行文件入口

## 学习笔记
每个模块的学习笔记将在学习过程中逐步添加到对应的文件中。

## 开始学习
我们将从最基础的 `errors.rs` 开始学习，这是理解整个项目错误处理机制的关键。请等待确认后开始第一课的学习。
