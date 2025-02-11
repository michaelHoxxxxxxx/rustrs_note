# elements/registry.rs 学习笔记

## 文件概述
`registry.rs` 是 electrs 项目中实现 Liquid 资产注册表的核心文件。该文件提供了资产元数据的管理、存储和查询功能，包括文件系统同步、缓存管理和排序功能。

## 主要组件分析

### 1. 资产注册表结构
```rust
pub struct AssetRegistry {
    directory: path::PathBuf,                                  // 资产元数据目录
    assets_cache: HashMap<AssetId, (SystemTime, AssetMeta)>,  // 资产缓存：ID -> (修改时间, 元数据)
}

// 资产条目类型别名
pub type AssetEntry<'a> = (&'a AssetId, &'a AssetMeta);
```

### 2. 资产元数据结构
```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AssetMeta {
    #[serde(skip_serializing_if = "JsonValue::is_null")]
    pub contract: JsonValue,     // 资产合约信息
    #[serde(skip_serializing_if = "JsonValue::is_null")]
    pub entity: JsonValue,       // 发行实体信息
    pub precision: u8,           // 资产精度
    pub name: String,            // 资产名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticker: Option<String>,  // 资产代码（可选）
}

impl AssetMeta {
    // 获取发行实体域名
    fn domain(&self) -> Option<&str> {
        self.entity["domain"].as_str()
    }
}
```

### 3. 排序功能实现
```rust
// 排序结构体和枚举
pub struct AssetSorting(AssetSortField, AssetSortDir);

pub enum AssetSortField {
    Name,    // 按名称排序
    Domain,  // 按域名排序
    Ticker,  // 按代码排序
}

pub enum AssetSortDir {
    Descending,  // 降序
    Ascending,   // 升序
}

// 排序实现
impl AssetSorting {
    // 创建排序比较器
    fn as_comparator(self) -> Box<dyn Fn(&AssetEntry, &AssetEntry) -> cmp::Ordering> {
        let sort_fn: Box<dyn Fn(&AssetEntry, &AssetEntry) -> cmp::Ordering> = match self.0 {
            // 按名称排序（使用资产 ID 作为次要排序键）
            AssetSortField::Name => {
                Box::new(|a, b| lc_cmp(&a.1.name, &b.1.name).then_with(|| a.0.cmp(b.0)))
            }
            // 按域名排序
            AssetSortField::Domain => Box::new(|a, b| a.1.domain().cmp(&b.1.domain())),
            // 按代码排序
            AssetSortField::Ticker => Box::new(|a, b| lc_cmp_opt(&a.1.ticker, &b.1.ticker)),
        };

        // 根据排序方向返回最终的比较器
        match self.1 {
            AssetSortDir::Ascending => sort_fn,
            AssetSortDir::Descending => Box::new(move |a, b| sort_fn(a, b).reverse()),
        }
    }
}
```

### 4. 文件系统同步
```rust
impl AssetRegistry {
    // 同步文件系统中的资产元数据
    pub fn fs_sync(&mut self) -> Result<()> {
        // 遍历资产目录
        for entry in fs::read_dir(&self.directory)? {
            let entry = entry?;
            // 检查是否为分区目录
            if !entry.file_type()?.is_dir() || entry.file_name().len() != DIR_PARTITION_LEN {
                continue;
            }

            // 遍历分区目录中的文件
            for file_entry in fs::read_dir(entry.path())? {
                let file_entry = file_entry?;
                let path = file_entry.path();
                // 只处理 JSON 文件
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }

                // 解析资产 ID
                let asset_id = AssetId::from_str(
                    path.file_stem()
                        .unwrap()
                        .to_str()?
                )?;

                // 获取文件修改时间
                let modified = file_entry.metadata()?.modified()?;

                // 检查缓存是否需要更新
                if let Some((last_update, _)) = self.assets_cache.get(&asset_id) {
                    if *last_update == modified {
                        continue;
                    }
                }

                // 读取并解析元数据
                let metadata: AssetMeta = serde_json::from_str(
                    &fs::read_to_string(path)?
                )?;

                // 更新缓存
                self.assets_cache.insert(asset_id, (modified, metadata));
            }
        }
        Ok(())
    }
}
```

## 核心功能解析

1. **资产管理**：
   - 元数据存储和缓存
   - 文件系统同步
   - 资产查询和列表

2. **排序功能**：
   - 多字段排序支持
   - 大小写不敏感比较
   - 灵活的排序方向

3. **缓存机制**：
   - 基于时间戳的缓存更新
   - 内存缓存优化
   - 异步同步支持

## 设计要点

1. **性能优化**：
   - 缓存机制
   - 分区存储
   - 异步更新

2. **可扩展性**：
   - 灵活的排序系统
   - 通用的元数据结构
   - 模块化设计

3. **可靠性**：
   - 错误处理
   - 文件系统同步
   - 数据一致性

4. **易用性**：
   - 查询参数支持
   - 友好的 API
   - 完整的文档

## 使用示例

```rust
// 1. 创建资产注册表
let registry = AssetRegistry::new(path::PathBuf::from("/path/to/assets"));

// 2. 查询资产元数据
if let Some(meta) = registry.get(&asset_id) {
    println!("Asset name: {}, precision: {}", meta.name, meta.precision);
}

// 3. 列出资产（带排序）
let sorting = AssetSorting::from_query_params(&query_params)?;
let (total, assets) = registry.list(0, 10, sorting);

// 4. 启动同步线程
let registry = Arc::new(RwLock::new(registry));
let _sync_thread = AssetRegistry::spawn_sync(registry.clone());
``` 