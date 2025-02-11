# new_index/precache.rs 学习笔记

## 文件概述
`precache.rs` 是 electrs 项目中负责预缓存功能的模块。该模块主要用于提前缓存脚本哈希相关的数据，以提高后续查询的性能。

## 导入和依赖
```rust
use crate::chain::address::Address;
use crate::errors::*;
use crate::new_index::ChainQuery;
use crate::util::FullHash;
use crypto::digest::Digest;
use crypto::sha2::Sha256;
use rayon::prelude::*;
```

## 核心功能实现

### 1. 预缓存功能
```rust
#[trace]
pub fn precache(chain: &ChainQuery, scripthashes: Vec<FullHash>) {
    let total = scripthashes.len();
    info!("Pre-caching stats and utxo set for {} scripthashes", total);

    // 创建线程池进行并行处理
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(16)
        .thread_name(|i| format!("precache-{}", i))
        .build()
        .unwrap();
        
    pool.install(|| {
        scripthashes
            .par_iter()
            .enumerate()
            .for_each(|(i, scripthash)| {
                // 每处理5个打印一次进度
                if i % 5 == 0 {
                    info!("running pre-cache for scripthash {}/{}", i + 1, total);
                }
                chain.stats(&scripthash[..]);
            })
    });
}
```

### 2. 从文件加载脚本哈希
```rust
pub fn scripthashes_from_file(path: String) -> Result<Vec<FullHash>> {
    let reader = io::BufReader::new(File::open(path)?);
    reader
        .lines()
        .map(|line| {
            let line = line?;
            let cols: Vec<&str> = line.split(',').collect();
            to_scripthash(cols[0], cols[1])
        })
        .collect()
}
```

### 3. 脚本哈希转换
```rust
fn to_scripthash(script_type: &str, script_str: &str) -> Result<FullHash> {
    match script_type {
        "address" => address_to_scripthash(script_str),
        "scripthash" => Ok(FullHash::from_hex(script_str)?),
        "scriptpubkey" => Ok(compute_script_hash(
            &Vec::from_hex(script_str)?
        )),
        _ => bail!("Invalid script type")
    }
}
```

### 4. 地址转脚本哈希
```rust
fn address_to_scripthash(addr: &str) -> Result<FullHash> {
    let addr = Address::from_str(addr)?;
    Ok(compute_script_hash(&addr.script_pubkey().as_bytes()))
}
```

### 5. 计算脚本哈希
```rust
pub fn compute_script_hash(data: &[u8]) -> FullHash {
    let mut hash = FullHash::default();
    let mut sha2 = Sha256::new();
    sha2.input(data);
    sha2.result(&mut hash);
    hash
}
```

## 设计要点

1. **并行处理**：
   - 使用 rayon 库实现并行处理
   - 默认使用16个线程的线程池
   - 通过 `par_iter()` 实现并行迭代

2. **文件处理**：
   - 使用 `BufReader` 高效读取文件
   - 支持按行读取和解析
   - CSV 格式：`script_type,script_data`

3. **多种输入格式支持**：
   - 地址（address）
   - 脚本哈希（scripthash）
   - 脚本公钥（scriptpubkey）

4. **错误处理**：
   - 使用 Result 类型
   - 链式错误处理
   - 详细的错误信息

## 使用示例

```rust
// 从文件加载脚本哈希
let scripthashes = scripthashes_from_file("addresses.txt")?;

// 预缓存数据
precache(&chain, scripthashes);

// 计算单个脚本哈希
let script_hash = compute_script_hash(script_data);
```

## 注意事项

1. **性能考虑**：
   - 预缓存操作是CPU密集型的
   - 使用并行处理提高性能
   - 注意内存使用

2. **输入文件格式**：
   - 每行必须包含两列，用逗号分隔
   - 第一列是脚本类型
   - 第二列是具体数据

3. **错误处理**：
   - 文件读取错误
   - 格式解析错误
   - 哈希计算错误

4. **资源管理**：
   - 合理控制线程数
   - 及时释放文件句柄
   - 监控内存使用 