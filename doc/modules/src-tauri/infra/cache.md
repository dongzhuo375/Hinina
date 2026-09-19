# cache（客户端缓存原语）

> 源文件：`src-tauri/src/infra/cache.rs`

## 职责

提供两个**与业务无关**的缓存组件，供 Service 层按需组合：带 TTL 与容量上限的内存缓存（`TtlCache`）、`cache/{namespace}/{key}.json` 形式的磁盘缓存（`JsonDiskCache`）。本模块只负责「怎么存」，**不决定缓存什么、缓存多久** —— 那是各 Service 的职责（见 `doc/Architecture.md` 的「客户端缓存策略」）。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `TtlCache<K, V>` | `new(ttl: Duration, capacity: usize) -> Self` | 内存缓存；`capacity` 为 0 时按 1 处理 |
| | `get(&K) -> Option<V>` | 读锁；未命中或已过期返回 `None`。**过期条目不在读路径删除**（读锁不写），由 `insert` 的清理步骤回收 |
| | `insert(K, V)` | 写锁；**先无条件清理过期条目**（既回收无压力时残留的过期项，也让容量判定以存活数为准），再在满员且键不存在时淘汰 `inserted_at` 最旧的一条；同键视为更新（刷新计时）。**容量上界恒成立** —— 容量检查不能以 `!contains_key` 为前提，否则「已过期但未回收」的键被重新插入时会复活并突破上界 |
| | `invalidate(&K) -> bool` / `clear()` / `len()` / `is_empty()` | `len()` 只统计存活条目（不含残留过期项） |
| `JsonDiskCache` | `new(storage: Arc<Storage>, namespace: &'static str) -> Self` | 磁盘缓存，落盘路径 `cache/{namespace}/{key}.json`。构造时做**一次性布局清扫**（best-effort）：本 namespace 的 `.layout-version` 标记落后于 `CACHE_LAYOUT_VERSION` 时清空该 namespace —— 布局变更（如键新增 OJ 维度）后旧条目**既不匹配新键也不会被 TTL 懒删除**，不主动清扫就是永久孤儿。只清扫**不建目录**（标记由首次 `write` 落地），以保住「没有数据就没有目录」这一可观测语义；清扫失败不写标记，下次启动重试 |
| | `read<T: DeserializeOwned>(&self, key, ttl: Duration) -> Option<T>` | 不存在 / 不安全键 / 解析失败 / 已过期一律 `None`；**过期文件懒删除**（删失败只告警） |
| | `write<T: Serialize>(&self, key, &T)` | 自动建目录 + 首次写入时落地布局标记；失败只告警（缓存写入失败不影响正确性） |
| | `remove(&key) -> bool` / `clear_namespace() -> bool` | 单键删除 / 整个 namespace 目录递归删除（幂等；连布局标记一并删除，下次构造重扫一次，此时目录本就空是 no-op） |
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
  写锁 → 无条件 purge 过期条目（回收无压力时的残留）
       → 键不存在且 guard.len() >= capacity → 淘汰 inserted_at 最旧
       → 写入/覆盖（刷新 inserted_at）              // 存活数恒 <= capacity

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
- **容量上界是硬约束**（用户域缓存的内存边界）：`insert` 每次先清过期再判容量，
  使「存活数 ≤ capacity」恒成立。曾经的写法把容量检查挂在 `!contains_key` 之后，
  让「已过期但未回收」的键被重新插入时绕过检查（复活）而突破上界。
- **磁盘缓存必须带 `fetchedAt`**：只存值会让重启后的条目永远「新鲜」，
  等于把 TTL 变成「进程生命周期」。
- **键的作用域由调用方保证**：本模块只做路径安全校验（拒绝 `..`/绝对路径/空键，
  与 `Storage::resolve` 形成两道防线），`{oj}/{contest_id}` / `{oj}/{submit_id}` 等
  作用域前缀由各 Service 拼装。**作用域是正确性的一部分** —— 可能撞号的维度必须编进
  键，而不是依赖「切换时清理」（清理可能延迟或失败）。
- **用户域数据禁止落盘**：`JsonDiskCache` 只用于公共数据（比赛元信息、题面）；
  用户域（提交详情/测试点）只用 `TtlCache` + 登出清理，避免同机换账号串号。
- **只缓存成功结果**：本模块无「缓存错误」的入口，调用方也不得把 `Err` 写入 ——
  否则会话失效（401/403）会被缓存掩盖，前端 `sessionGuard` 拿不到 `Auth` 变体。

## 测试

`src-tauri/src/infra/tests/cache_tests.rs`（由 `cache.rs` 底部 `#[cfg(test)] #[path = "tests/cache_tests.rs"] mod tests;` 引用）锁定 18 例：TTL 内命中/过期未命中/同键重插刷新计时、容量淘汰最旧、满容量时先清过期（不挤掉存活条目）、**复活已过期键仍受容量约束（回归）**、**无压力时也回收过期条目**、容量 0 钳为 1、键隔离、`invalidate`/`clear`、并发读写下不死锁且有命中、磁盘往返含 `fetchedAt`、未命中与非法键（`..`）不落盘、按 `fetchedAt` 过期并懒删除文件、损坏 JSON 视为未命中（**先构造再写损坏文件** —— 反过来会被布局清扫删掉，用例退化为「文件缺失」）、跨实例（模拟重启）命中 + `remove`/`clear_namespace` 幂等、嵌套键按比赛分目录互不覆盖、**布局清扫按 namespace 各扫一次且标记就位后不再清**（否则跨重启缓存失效）。

