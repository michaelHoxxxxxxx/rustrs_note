# util/fees.rs 学习笔记

## 文件概述
`fees.rs` 是 electrs 项目中处理交易手续费相关的工具文件。该文件实现了交易手续费的计算、费率统计和直方图生成等功能。

## 常量定义

```rust
const VSIZE_BIN_WIDTH: u64 = 50_000;    // 虚拟字节大小的区间宽度，用于生成手续费直方图
```

## 主要组件分析

### 1. TxFeeInfo 结构体
```rust
pub struct TxFeeInfo {
    pub fee: u64,           // 手续费金额（以聪为单位）
    pub vsize: u64,         // 虚拟大小（= weight/4，以虚拟字节为单位）
    pub fee_per_vbyte: f64, // 每虚拟字节的手续费（以 sat/vB 为单位）
}
```

#### TxFeeInfo 方法
```rust
impl TxFeeInfo {
    pub fn new(tx: &Transaction, prevouts: &HashMap<u32, &TxOut>, network: Network) -> Self {    // 创建新的交易手续费信息
        let fee = get_tx_fee(tx, prevouts, network);    // 计算交易手续费

        let weight = tx.weight();    // 获取交易权重
        #[cfg(not(feature = "liquid"))]    // 非 Liquid 网络的条件编译
        let weight = weight.to_wu();    // 将权重转换为权重单位

        let vsize_float = weight as f64 / 4f64;    // 计算虚拟大小（更精确的 sat/vB）

        TxFeeInfo {
            fee,
            vsize: vsize_float.ceil() as u64,    // 向上取整
            fee_per_vbyte: fee as f64 / vsize_float,    // 计算每字节费率
        }
    }
}
```

### 2. 核心功能实现

#### 1) 交易手续费计算
```rust
#[cfg(not(feature = "liquid"))]    // 非 Liquid 网络版本
pub fn get_tx_fee(tx: &Transaction, prevouts: &HashMap<u32, &TxOut>, _network: Network) -> u64 {
    if tx.is_coinbase() {    // 如果是coinbase交易，手续费为0
        return 0;
    }

    let total_in: u64 = prevouts    // 计算输入总额
        .values()
        .map(|prevout| prevout.value.to_sat())
        .sum();
    let total_out: u64 = tx.output.iter()    // 计算输出总额
        .map(|vout| vout.value.to_sat())
        .sum();
    total_in - total_out    // 手续费 = 输入总额 - 输出总额
}

#[cfg(feature = "liquid")]    // Liquid 网络版本
pub fn get_tx_fee(tx: &Transaction, _prevouts: &HashMap<u32, &TxOut>, network: Network) -> u64 {
    tx.fee_in(*network.native_asset())    // 直接获取原生资产的手续费
}
```

#### 2) 手续费直方图生成
```rust
pub fn make_fee_histogram(mut entries: Vec<&TxFeeInfo>) -> Vec<(f64, u64)> {
    // 按费率排序（从低到高）
    entries.sort_unstable_by(|e1, e2| e1.fee_per_vbyte.partial_cmp(&e2.fee_per_vbyte).unwrap());

    let mut histogram = vec![];
    let mut bin_size = 0;
    let mut last_fee_rate = 0.0;
    
    // 从高费率到低费率遍历
    for e in entries.iter().rev() {
        if bin_size > VSIZE_BIN_WIDTH && last_fee_rate != e.fee_per_vbyte {
            // 当累积的虚拟大小超过区间宽度且费率发生变化时，添加一个新的直方图条目
            histogram.push((last_fee_rate, bin_size));
            bin_size = 0;
        }
        last_fee_rate = e.fee_per_vbyte;
        bin_size += e.vsize;
    }
    // 处理最后一个区间
    if bin_size > 0 {
        histogram.push((last_fee_rate, bin_size));
    }
    histogram
}
```

## 设计要点

1. **费用计算**：
   - 支持普通比特币网络和 Liquid 网络
   - 通过条件编译区分不同网络的实现
   - 考虑 coinbase 交易的特殊情况

2. **性能优化**：
   - 使用 `sort_unstable_by` 进行快速排序
   - 高效的直方图生成算法
   - 精确的浮点数计算

3. **可扩展性**：
   - 模块化的费用计算逻辑
   - 灵活的网络适配
   - 可配置的直方图区间

## 使用示例

```rust
// 1. 创建交易手续费信息
let tx_fee_info = TxFeeInfo::new(&transaction, &prevouts, network);

// 2. 获取手续费信息
println!("Fee: {} satoshis", tx_fee_info.fee);
println!("Virtual size: {} vbytes", tx_fee_info.vsize);
println!("Fee rate: {:.2} sat/vB", tx_fee_info.fee_per_vbyte);

// 3. 生成手续费直方图
let fee_infos = vec![&tx_fee_info1, &tx_fee_info2, &tx_fee_info3];
let histogram = make_fee_histogram(fee_infos);
for (fee_rate, size) in histogram {
    println!("Fee rate >= {:.2} sat/vB: {} vbytes", fee_rate, size);
}
```

## 注意事项

1. **费率计算**：
   - 使用虚拟大小（vsize）而不是实际大小
   - 费率精度使用浮点数计算
   - 向上取整以确保足够的手续费

2. **直方图生成**：
   - 按费率降序排列
   - 使用可配置的区间宽度
   - 累积相同费率的交易

3. **网络兼容性**：
   - 需要正确配置网络类型
   - Liquid 网络使用不同的费用计算方式
   - 确保提供正确的前置输出信息 