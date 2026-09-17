# cache（客户端缓存原语）

> 源文件：`src-tauri/src/infra/cache.rs`

## 职责

提供两个**与业务无关**的缓存组件，供 Service 层按需组合：带 TTL 与容量上限的内存缓存（`TtlCache`）、`cache/{namespace}/{key}.json` 形式的磁盘缓存（`JsonDiskCache`）。本模块只负责「怎么存」，**不决定缓存什么、缓存多久** —— 那是各 Service 的职责（见 `doc/Architecture.md` 的「客户端缓存策略」）。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `TtlCache<K, V>` | `new(ttl: Duration, capacity: usize) -> Self` | 内存缓存；`capacity` 为 0 时按 1 处理 |
| | `get(&K) -> Option<V>` | 读锁；未命中或已过期返回 `None`。**过期条目不在读路径删除**（读锁不写），由 `insert` 的清理步骤回收 |
| | `insert(K, V)` | 写锁；同键视为更新（刷新计时、不占新容量）；容量满时先清过期、仍满则淘汰 `inserted_at` 最旧的一条 |
| | `invalidate(&K) -> bool` / `clear()` / `len()` / `is_empty()` | `len()` 只统计存活条目（不含残留过期项） |
| `JsonDiskCache` | `new(storage: Arc<Storage>, namespace: &'static str) -> Self` | 磁盘缓存，落盘路径 `cache/{namespace}/{key}.json` |
| | `read<T: DeserializeOwned>(&self, key, ttl: Duration) -> Option<T>` | 不存在 / 不安全键 / 解析失败 / 已过期一律 `None`；**过期文件懒删除**（删失败只告警） |
| | `write<T: Serialize>(&self, key, &T)` | 自动建目录；失败只告警（缓存写入失败不影响正确性） |
| | `remove(&key) -> bool` / `clear_namespace() -> bool` | 单键删除 / 整个 namespace 目录递归删除（幂等） |
| `DiskEntry<T>` | `{ fetched_at: i64, value: T }`（camelCase） | 磁盘条目自带写入时刻，使 TTL **跨重启继续计时**，避免「重启即永久命中」 |
| `now_unix()` | `() -> i64` | 磁盘条目时间戳（系统时间异常时回退 0） |

## 直接依赖

- `serde`（`Serialize` / `Deserialize` / `DeserializeOwned`）
- `tracing`（`debug` 命中/淘汰、`warn` 写失败与解析失败）
- `crate::infra::storage::Storage`（相对路径读写 + 目录穿越校验）

## 被依赖

- `service::contest::ContestService` — 比赛元信息（内存 + 磁盘，TTL 120s）
- `service::problem::ProblemService` — 题面（内存 + 磁盘，TTL 30min；受 `oj.cacheProblemStatement` 开关控制）
- `service::submission::SubmissionService` — 终态提交详情/测试点（**仅内存**，TTL 2h，容量 200/100）

## 逻辑流程

```
TtlCache::get(key)
  读锁 → 命中且 elapsed < ttl ? Some(clone) : None      // 不加写锁，并发读无竞争

TtlCache::insert(key, value)
  写锁 → 键不存在且存活数 >= capacity
        → 先 purge 过期条目 → 仍满 → 淘汰 inserted_at 最旧
        → 写入/覆盖（刷新 inserted_at）

JsonDiskCache::read(key, ttl)
  key_path(key)（拒绝空键 / 绝对路径 / 含 ".."）
    → storage.read_to_string 失败 → None（未命中，不记日志）
    → 反序列化 DiskEntry 失败 → warn + None
    → now - fetched_at >= ttl → debug + 删文件（懒清理）+ None
    → 否则 debug(hit) + Some(value)

JsonDiskCache::write(key, value)
  key_path → create_dir(namespace) → 序列化 DiskEntry{fetchedAt, value} → write_string
    任一步失败 → warn（不影响调用方）
```

设计要点：

- **近似 FIFO 而非精确 LRU**：精确 LRU 需在命中时更新访问序 → 读路径变写锁；
  本仓库缓存容量仅数百条、TTL 分钟级，收益不抵复杂度。淘汰顺序「先过期、再最旧插入」
  已保证内存有界。
- **磁盘缓存必须带 `fetchedAt`**：只存值会让重启后的条目永远「新鲜」，
  等于把 TTL 变成「进程生命周期」。
- **键的作用域由调用方保证**：本模块只做路径安全校验（拒绝 `..`/绝对路径/空键，
  与 `Storage::resolve` 形成两道防线），`contest_id` / `submit_id` 等作用域前缀
  由各 Service 拼装。
- **用户域数据禁止落盘**：`JsonDiskCache` 只用于公共数据（比赛元信息、题面）；
  用户域（提交详情/测试点）只用 `TtlCache` + 登出清理，避免同机换账号串号。
- **只缓存成功结果**：本模块无「缓存错误」的入口，调用方也不得把 `Err` 写入 ——
  否则会话失效（401/403）会被缓存掩盖，前端 `sessionGuard` 拿不到 `Auth` 变体。

## 测试

`src-tauri/src/infra/tests/cache_tests.rs`（由 `cache.rs` 底部 `#[cfg(test)] #[path = "tests/cache_tests.rs"] mod tests;` 引用）锁定 15 例：TTL 内命中/过期未命中/同键重插刷新计时、容量淘汰最旧、满容量时先清过期（不挤掉存活条目）、容量 0 钳为 1、键隔离、`invalidate`/`clear`、并发读写下不死锁且有命中、磁盘往返含 `fetchedAt`、未命中与非法键（`..`）不落盘、按 `fetchedAt` 过期并懒删除文件、损坏 JSON 视为未命中、跨实例（模拟重启）命中 + `remove`/`clear_namespace` 幂等、嵌套键按比赛分目录互不覆盖。

