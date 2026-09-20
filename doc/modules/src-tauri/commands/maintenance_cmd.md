# maintenance_cmd（客户端维护命令：重置与清理）

> 源文件：`src-tauri/src/commands/maintenance_cmd.rs`

## 职责

设置页「重置与清理」的两个动作，**定位刻意分开**：

- **重置客户端**（`reset_client`）：把客户端拉回「干净状态」—— 清掉一切**可以重新从服务端获取**的东西。安全、可反复点。
- **清理本地数据**（`local_data_usage` / `purge_local_data`）：删除**不可重建**的本地事实（日志内容、过期提交留档）。不可逆，故先给预览、再由用户显式勾选确认。

两者都**不动**：工作区代码（`workspaces/`）、配置（`config.json`）、登录会话（`sessions/`）。

## 核心类型/函数

| 名称 | 签名 | 说明 |
|------|------|------|
| `reset_client` | `async fn(State<'_, AppContext>) -> AppResult<()>` | 前端 invoke `reset_client`（无参数）。清缓存 → 忘公告基线 → 清公告已读 → 兜底清扫 `cache/` 根目录 |
| `local_data_usage` | `async fn(State<'_, AppContext>) -> AppResult<LocalDataUsage>` | 前端 invoke `local_data_usage`。统计日志字节数与留档占用（总/过期），供**先看再删** |
| `purge_local_data` | `async fn(State<'_, AppContext>, logs: bool, stale_snapshots: bool) -> AppResult<PurgeReport>` | 前端 invoke `purge_local_data`({ logs, staleSnapshots })。两个开关都由调用方显式传入；**逐项容错**（见下） |
| `run_purge` | `pub(crate) fn(logs, stale_snapshots, clear_log, purge_snapshots) -> PurgeReport` | 逐项容错的编排（依赖以闭包注入，故可单测）。**两个勾选项互不牵连** |
| `LocalDataUsage` | VO（`Serialize`，camelCase） | `logBytes` / `logPath` / `snapshotTotalCount` / `snapshotTotalBytes` / `snapshotStaleCount` / `snapshotStaleBytes` / `keepDays` |
| `PurgeReport` | VO（`Serialize` / `Default`，camelCase） | `freedBytes` / `removedSnapshots` / `logCleared` |

### 重置清什么 / 不清什么

| 清 | 载体 |
|---|---|
| 比赛列表 + 比赛元信息 | 内存 + 磁盘 `cache/contest_meta/` |
| 题面、题目 limits | 内存 + 磁盘 `cache/problem_statement/`、`cache/problem_limits/` |
| 终态提交详情 / 测试点（**含源代码，属登录态数据**） | 内存（与登出共用 `clear_user_caches`） |
| 公告基线（`announcement_baseline`） | 内存 |
| 公告已读状态（`announcements_read/`） | 磁盘 |

| 不清 | 为什么 |
|---|---|
| `workspaces/` | 选手唯一作品本体，**不可重建**（OJ 只有提交过的版本，未提交的编辑全没了） |
| `submissions/` | OJ 不回吐代码时的唯一来源；过期留档的回收走 `purge_local_data` |
| `config.json` | 手填的配置（OJ 地址 / 比赛引用 / 编辑器 / 布局 / 主题） |
| `sessions/` | 重置不该把选手踢回登录页（赛场上是事故级体验）；换账号有独立的登出路径 |
| `logs/` | 排障唯一线索；由 `purge_local_data` 按用户意愿清理 |

## 直接依赖

- `tauri`（`State`）、`serde`（VO 序列化）、`tracing`
- `crate::core::context::AppContext`（`contest` / `problem` / `submission` / `storage` / `logger`）
- `crate::infra::cache::CACHE_ROOT_DIR`（兜底清扫的根目录）、`crate::infra::logger::LOG_RELATIVE_PATH`
- `crate::service::submission::snapshot`（`inspect_snapshots` / `purge_stale_snapshots` / `SNAPSHOT_KEEP_DAYS`）

## 被依赖

- `main.rs` — `generate_handler!` 注册为 `reset_client` / `local_data_usage` / `purge_local_data`
- `src/bridge/system.bridge.ts` → `system.service.ts` → `views/SettingsView.vue`「重置与清理」区块

## 逻辑流程

```
reset_client
  → ContestService::clear_caches()                 // 列表/元信息内存 + 元信息磁盘
  → ProblemService::clear_caches()                 // 题面内存/磁盘 + limits 内存/磁盘
  → SubmissionService::clear_user_caches()         // 终态详情/测试点内存（含源代码）
  → ContestService::clear_announcement_baseline()  // 重置语义：连「已告知过哪些公告」也忘掉
  → ContestService::clear_announcement_read_state()// 已读状态落盘文件（红点复亮）
  → storage.remove_all("cache")                    // 兜底：覆盖将来新增的 namespace

local_data_usage
  → fs::metadata(logs/hinina.log)                  // 日志字节数
  → snapshot::inspect_snapshots(keep_days=30)      // 留档总/过期条数与字节数

purge_local_data(logs, staleSnapshots)
  → run_purge(逐项容错编排)
       logs            → Logger::clear_log_file()   // 「重开 + 截断」，只清内容不删文件
                          失败 → warn + log_cleared=false，**继续执行下一项**（不 ? 冒泡）
       staleSnapshots  → snapshot::purge_stale_snapshots(keep_days=30)
                          // 删 mtime 早于窗口的留档，回收空目录
```

## 设计要点

- **两个动作分开，是刻意的**：重置安全可反复点，清理不可逆必须显式勾选。把不可逆删除混进「重置」，风险是用户以为自己点的是安全按钮。
- **兜底清扫 `cache/` 根目录**：各 Service 只清自己那部分，清扫整个根目录才能保证**将来新增的 namespace 也会被重置覆盖**（否则重置会静默漏掉新缓存，用户以为干净了）。各 Service 的清理仍然保留 —— 它们要独立负责自己的内存缓存。
- **公告基线的处理与「清缓存」相反**：`clear_caches` **保留**基线（清空后新发的公告要能报出来），重置则**忘掉**它（重置后一切皆未见，留着没有意义）。两者分成两个方法，各有单测锁定，避免语义漂移。
- **日志用「重开 + 截断」而不是句柄 `set_len`**：日志句柄是追加模式打开的（只有 `FILE_APPEND_DATA`），Windows 上 `set_len` 会被拒（实测 `Os code 5, PermissionDenied`）。详见 `infra/logger.md`。
- **只清日志内容、不删日志文件**：删了要等重启才重建，中间这段排障信息就彻底没了。
- **过期留档按 mtime 判定，不比对服务端列表**：后者要网络、要分页、还可能因赛制隐藏记录而误判；而留档价值本就随时间衰减。
- **不可逆动作必须给预览**：`local_data_usage` 存在的唯一理由就是让用户在删除前看到确切范围与体积。
- **两个勾选项互不牵连（逐项容错）**：日志清理失败（文件被占用、权限不足）只降级为 `log_cleared=false` 并继续，**不中断**留档清理 —— 用户勾了留档就该拿到留档的结果。曾用 `?` 冒泡，日志一失败整个命令报错返回、留档一条都没删。编排抽成 `run_purge` 纯函数正是为了锁定这条（命令本体依赖 `State` 测不了）。
- **`log_cleared` 的三种 `false` 成因都要如实回报**：文件层不可用（`clear_log_file` 返回 `Ok(0)`，故用 `has_log_file()` 判定）、截断失败（`Err` 分支）、本次没勾选日志。界面据此提示「日志未清理」，而不是笼统报「已清理：释放 0 B」。
- **锁中毒统一 `into_inner` 取回内部数据**（与 `TtlCache` / `provider_registry_impl` 同款约定）：相关容器是 `Option` / `HashMap`，panic 不会让它们结构不一致，而「静默跳过」会让重置留下脏缓存 —— 那是比重置失败更糟的静默后果。

## 测试

命令本体依赖 `tauri::State`（无公开构造器）无法直接单测（见 `commands/mod.md`），故实质逻辑分两处覆盖：

- `infra/tests/logger_tests.rs`：`truncate_log_file` 在**追加句柄仍打开**时也能清空（这正是不能用 `set_len` 的约束）、释放字节数如实回报、清空后写入从 0 开始、空文件清理返回 0。
- `commands/tests/mod_tests.rs`：`run_purge` 的逐项容错 —— **日志失败时留档清理照常执行**（曾用 `?` 冒泡导致留档一条都不删）、文件层不可用时 `log_cleared=false` 且释放 0 字节、两项都成功时数值如实汇总、未勾选的项不执行（闭包 `unreachable!` 即证明）。
- `service/submission/tests/snapshot_tests.rs`：`inspect_snapshots` 的总量与过期判定（用 `File::set_times` 伪造 mtime）、`purge_stale_snapshots` **保留窗口内留档**（本功能唯一会丢数据的地方）、释放字节数与预览一致、空目录回收、无留档时全零且不报错。
- `service/contest/tests/contest_tests.rs`：`clear_announcement_baseline` 后下次拉取重新建基线（不报新公告）、`clear_announcement_read_state` 删净且幂等。
