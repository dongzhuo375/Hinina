// 客户端缓存原语。
//
// 提供两个**与业务无关**的缓存组件，供 Service 层按需组合：
//
// - [`TtlCache`]：内存缓存，TTL + 容量上限；淘汰顺序「先过期、再最旧插入」
//   （近似 FIFO，**不做精确 LRU** —— 精确 LRU 需要在命中时更新访问序，会把
//   读路径变成写锁；本仓库缓存容量仅数百条、TTL 分钟级，收益不抵复杂度）
// - [`JsonDiskCache`]：`cache/{namespace}/{key}.json` 磁盘缓存，条目自带
//   `fetchedAt` 以便跨重启继续计时；损坏文件视为未命中，过期文件懒删除
//
// 通用约定（各使用点必须遵守，见 `doc/Architecture.md` 的「客户端缓存策略」）：
//
// 1. **缓存是优化，不是正确性依赖**：读失败回退网络、写失败只告警、解析损坏视为未命中；
// 2. **只缓存成功结果**：错误（尤其 401/403）绝不入缓存 —— 否则会话失效会被缓存掩盖，
//    前端 `sessionGuard` 拿不到 `Auth` 变体，该登出时不登出；
// 3. **键必须带作用域**：比赛域含 `contest_id`、用户域含 `submit_id`，避免串号；
// 4. **用户域数据不落盘**：`JsonDiskCache` 只用于公共数据（比赛元信息、题面）。

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::infra::storage::Storage;

// ── 内存缓存（TTL + 容量上限） ──

/// 带 TTL 与容量上限的内存缓存。
///
/// 线程安全（`RwLock`）：命中走读锁，**不更新访问序**，因此并发读之间无写竞争。
pub struct TtlCache<K, V> {
    ttl: Duration,
    capacity: usize,
    inner: RwLock<HashMap<K, Entry<V>>>,
}

struct Entry<V> {
    inserted_at: Instant,
    value: V,
}

impl<K, V> TtlCache<K, V>
where
    K: Eq + std::hash::Hash + Clone,
    V: Clone,
{
    /// 创建缓存。`capacity` 为 0 时按 1 处理（缓存至少要能放一条）。
    pub fn new(ttl: Duration, capacity: usize) -> Self {
        Self {
            ttl,
            capacity: capacity.max(1),
            inner: RwLock::new(HashMap::new()),
        }
    }

    /// 读取（未命中或已过期返回 `None`）。
    ///
    /// 过期条目**不在读路径删除**（读锁不写），由后续 `insert` 的清理步骤回收；
    /// 因此 `len()` 只统计存活条目，不受残留过期项影响。
    pub fn get(&self, key: &K) -> Option<V> {
        let guard = self.inner.read().unwrap_or_else(|e| e.into_inner());
        let entry = guard.get(key)?;
        if entry.inserted_at.elapsed() >= self.ttl {
            return None;
        }
        Some(entry.value.clone())
    }

    /// 写入（已存在的键视为更新，刷新其插入时间）。
    ///
    /// **容量上界恒成立**：先无条件清理过期条目（既回收无压力时残留的过期项，
    /// 也让容量判定以存活数为准），再在满员时淘汰 `inserted_at` 最旧的一条。
    ///
    /// 反例（修复前）：容量 2、表中已有「已过期但未回收」的键 a 与存活的 b、c 时，
    /// 重新插入 a 会因 `contains_key` 短路而跳过容量检查 —— 过期键被「复活」，
    /// 存活数涨到 3。故容量检查不能以 `!contains_key` 为前提。
    pub fn insert(&self, key: K, value: V) {
        let mut guard = self.inner.write().unwrap_or_else(|e| e.into_inner());

        self.purge_expired(&mut guard);

        // purge 后表中只剩存活条目，`len()` 即存活数
        if !guard.contains_key(&key) && guard.len() >= self.capacity {
            if let Some(oldest) = self.oldest_key(&guard) {
                guard.remove(&oldest);
                debug!(capacity = self.capacity, "缓存容量已满，淘汰最旧条目");
            }
        }

        guard.insert(
            key,
            Entry {
                inserted_at: Instant::now(),
                value,
            },
        );
    }

    /// 删除指定键，返回是否确实存在。
    pub fn invalidate(&self, key: &K) -> bool {
        self.inner
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(key)
            .is_some()
    }

    /// 清空全部条目。
    pub fn clear(&self) {
        self.inner
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }

    /// 存活条目数（不含已过期但尚未回收的条目）。
    pub fn len(&self) -> usize {
        let guard = self.inner.read().unwrap_or_else(|e| e.into_inner());
        self.live_len(&guard)
    }

    /// 是否无存活条目。
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn live_len(&self, guard: &HashMap<K, Entry<V>>) -> usize {
        guard
            .values()
            .filter(|entry| entry.inserted_at.elapsed() < self.ttl)
            .count()
    }

    fn purge_expired(&self, guard: &mut HashMap<K, Entry<V>>) {
        guard.retain(|_, entry| entry.inserted_at.elapsed() < self.ttl);
    }

    fn oldest_key(&self, guard: &HashMap<K, Entry<V>>) -> Option<K> {
        guard
            .iter()
            .min_by_key(|(_, entry)| entry.inserted_at)
            .map(|(key, _)| key.clone())
    }
}

// ── 磁盘缓存（JSON + TTL） ──

/// 磁盘缓存布局版本。
///
/// 布局变更（如键的作用域增加 OJ 维度）时递增：旧布局的条目**既不匹配新键、
/// 也不会被 TTL 懒删除**（永不读取 → 永不触发过期删除），不主动清扫就是永久孤儿。
const CACHE_LAYOUT_VERSION: u32 = 2;

/// 布局标记文件名（**每个 namespace 一个**）。
///
/// 刻意不放在 `cache/` 根下：清扫是按 namespace 进行的，全局标记会被
/// 「先构造的那个 namespace」抢先写入，导致其余 namespace 的旧条目永不清扫。
const LAYOUT_MARKER_FILE: &str = ".layout-version";

/// `cache/{namespace}/{key}.json` 形式的磁盘缓存。
///
/// 条目结构 `{ fetchedAt, value }`：`fetchedAt` 落盘后跨重启仍可判定 TTL，
/// 避免「重启即永久命中」。
pub struct JsonDiskCache {
    storage: Arc<Storage>,
    namespace: &'static str,
}

/// 磁盘条目：值 + 写入时刻（Unix 秒）。
///
/// camelCase 与项目其它落盘 JSON（`config.json`、`cache/problem_limits/*.json`）一致。
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiskEntry<T> {
    fetched_at: i64,
    value: T,
}

impl JsonDiskCache {
    /// 创建磁盘缓存（`namespace` 决定子目录，如 `problem_statement`）。
    ///
    /// 顺带做**一次性布局清扫**（best-effort）：标记文件的版本落后于
    /// [`CACHE_LAYOUT_VERSION`] 时清空本 namespace —— 布局变更后旧条目既不会被
    /// 读取也不会被 TTL 懒删除，不清理就是永久孤儿（见常量注释）。清扫失败则不写
    /// 标记，下次启动重试。
    pub fn new(storage: Arc<Storage>, namespace: &'static str) -> Self {
        purge_legacy_layout_if_needed(&storage, namespace);
        Self { storage, namespace }
    }

    /// 读取未过期条目；不存在 / 不安全键 / 解析失败 / 已过期一律返回 `None`。
    ///
    /// 已过期的文件**懒删除**（删失败只告警）：题面/元信息体积小且按比赛分目录，
    /// 不做全量清扫。
    pub fn read<T: DeserializeOwned>(&self, key: &str, ttl: Duration) -> Option<T> {
        let path = self.key_path(key)?;
        let raw = match self.storage.read_to_string(&path) {
            Ok(raw) => raw,
            // 未命中是最常见的分支，不值得记日志
            Err(_) => return None,
        };

        let entry = match serde_json::from_str::<DiskEntry<T>>(&raw) {
            Ok(entry) => entry,
            Err(e) => {
                warn!(namespace = self.namespace, key = key, error = %e, "磁盘缓存解析失败，按未命中处理");
                return None;
            }
        };

        if now_unix().saturating_sub(entry.fetched_at) >= ttl.as_secs() as i64 {
            debug!(namespace = self.namespace, key = key, "磁盘缓存已过期，删除后按未命中处理");
            if let Err(e) = self.storage.remove(&path) {
                warn!(namespace = self.namespace, path = %path, error = %e, "过期磁盘缓存删除失败");
            }
            return None;
        }

        debug!(namespace = self.namespace, key = key, hit = true, "磁盘缓存命中");
        Some(entry.value)
    }

    /// 写入条目（失败只告警：缓存写入失败不影响正确性）。
    pub fn write<T: Serialize>(&self, key: &str, value: &T) {
        let Some(path) = self.key_path(key) else {
            return;
        };
        if let Err(e) = self.storage.create_dir(self.namespace) {
            warn!(namespace = self.namespace, error = %e, "创建磁盘缓存目录失败");
            return;
        }

        // 首次写入时落地布局标记：标记存在 = 本 namespace 已是新布局，
        // 后续启动不再清扫（构造时只清扫、不建目录，见 purge_legacy_layout_if_needed）
        let marker = format!("{}/{}", self.namespace, LAYOUT_MARKER_FILE);
        if !self.storage.exists(&marker) {
            if let Err(e) =
                self.storage
                    .write_string(&marker, &CACHE_LAYOUT_VERSION.to_string())
            {
                warn!(namespace = self.namespace, error = %e, "写入缓存布局标记失败");
            }
        }

        let entry = DiskEntry {
            fetched_at: now_unix(),
            value,
        };
        match serde_json::to_string(&entry) {
            Ok(json) => {
                if let Err(e) = self.storage.write_string(&path, &json) {
                    warn!(namespace = self.namespace, key = key, error = %e, "写入磁盘缓存失败");
                }
            }
            Err(e) => warn!(namespace = self.namespace, key = key, error = %e, "磁盘缓存序列化失败"),
        }
    }

    /// 删除指定键对应的文件。
    pub fn remove(&self, key: &str) -> bool {
        let Some(path) = self.key_path(key) else {
            return false;
        };
        self.storage.remove(&path).is_ok()
    }

    /// 删除整个 namespace 目录（登出 / 换比赛等场景的粗粒度失效）。
    ///
    /// 调用方需自行判断是否可延迟：**键自带作用域**（如 `{oj}/{cid}`）时，
    /// 清理只承担空间回收，可放到 `EventBus::subscribe_deferred` 的延迟段执行；
    /// 若清理是正确性依赖，则必须同步完成。
    pub fn clear_namespace(&self) -> bool {
        if !self.storage.exists(self.namespace) {
            return false;
        }
        match self.storage.remove_all(self.namespace) {
            Ok(()) => true,
            Err(e) => {
                warn!(namespace = self.namespace, error = %e, "清空磁盘缓存目录失败");
                false
            }
        }
    }

    /// 把键映射为 `cache/{namespace}/{key}.json`。    ///
    /// 键可含 `/` 以按作用域分目录（如 `{oj}/{contest_id}/{display_id}`）；空键、
    /// 绝对路径与含 `..` 的键一律拒绝（返回 `None`，即放弃缓存而非报错）——
    /// `Storage::resolve` 也会拒绝 `..`，这里是第二道防线。
    ///
    /// **作用域是正确性的一部分**：跨维度（OJ / 比赛 / 题目）可能撞号的缓存，
    /// 必须把该维度编进键，而不是依赖「切换时清理」—— 清理可能延迟或失败。
    fn key_path(&self, key: &str) -> Option<String> {
        if key.is_empty() || key.starts_with('/') || key.starts_with('\\') || key.contains("..") {
            warn!(namespace = self.namespace, key = key, "非法缓存键，跳过磁盘缓存");
            return None;
        }
        Some(format!("{}/{}.json", self.namespace, key))
    }
}

/// 当前 Unix 秒（磁盘条目时间戳）。
fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 一次性布局清扫：本 namespace 的标记版本落后时清空该 namespace（best-effort）。
///
/// 失败路径刻意保守：**清扫失败就不写标记**，下次启动重试 —— 宁可多扫一次，
/// 也不要把「未清扫」当成「已清扫」而永久留下孤儿。
///
/// 只清扫、**不创建目录**：标记由首次 [`JsonDiskCache::write`] 落地，从而保住
/// 「没有数据就没有目录」这一可观测语义（关闭题面缓存时不应凭空多出目录）。
/// 标记落在 namespace 目录内（见 [`LAYOUT_MARKER_FILE`] 注释）；`clear_namespace`
/// 会连标记一起删掉，下次构造再扫一遍（此时目录本就空，是 no-op）。
fn purge_legacy_layout_if_needed(storage: &Storage, namespace: &str) {
    let marker = format!("{}/{}", namespace, LAYOUT_MARKER_FILE);
    let recorded = storage
        .read_to_string(&marker)
        .ok()
        .and_then(|raw| raw.trim().parse::<u32>().ok());
    if recorded == Some(CACHE_LAYOUT_VERSION) {
        return;
    }

    if !storage.exists(namespace) {
        return; // 无目录即无旧条目（标记也随目录一并消失，见 clear_namespace）
    }
    if let Err(e) = storage.remove_all(namespace) {
        warn!(
            namespace = namespace,
            error = %e,
            "布局清扫失败，下次启动重试"
        );
        return;
    }
    debug!(namespace = namespace, "已清扫旧布局缓存");
}

#[cfg(test)]
#[path = "tests/cache_tests.rs"]
mod tests;
