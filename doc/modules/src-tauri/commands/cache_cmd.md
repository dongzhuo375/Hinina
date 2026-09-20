# cache_cmd（缓存维护命令）

> 源文件：`src-tauri/src/commands/cache_cmd.rs`

## 职责

「清空缓存」命令：把**可以随时从服务端重新获取**的三层缓存一次性清掉，供设置页的维护动作调用。清完不重拉（补拉时机由前端决定）。

## 核心类型/函数

| 名称 | 签名 | 说明 |
|------|------|------|
| `clear_cache` | `async fn(State<'_, AppContext>) -> AppResult<()>` | 前端 invoke 签名 `clear_cache`（无参数）。按顺序调 `contest.clear_caches()` → `problem.clear_caches()` → `submission.clear_user_caches()` |

**清什么**（均按 OJ / 比赛维度分键，清理只让当前会话立刻回到干净状态）：

| 层 | 内容 | 载体 |
|---|---|---|
| 比赛 | 比赛列表 + 比赛元信息 | 内存（`ContestCache` / `TtlCache`）+ 磁盘 `cache/contest_meta/` |
| 题目 | 题面、题目 limits | 内存（`TtlCache` / `RwLock<HashMap>`）+ 磁盘 `cache/problem_statement/`、`cache/problem_limits/` |
| 提交 | 终态提交详情 / 测试点（**含源代码，属登录态数据**） | 内存 `TtlCache`（与登出共用 `clear_user_caches`） |

**刻意不清什么**（本地事实，删掉不可恢复，且不属于「缓存」）：

- 工作区代码与元数据（`workspaces/`）
- 提交源码快照（`submissions/`）—— OJ 不回吐代码时它是唯一本地来源
- 公告已读状态（`announcements_read/`）、配置（`config.json`）、日志（`logs/`）
- **公告基线**（`ContestService::announcement_baseline`）：它不是缓存，而是「已经告诉过用户哪些公告」的记忆，清掉会让清空之后新发布的公告被当成「首次拉取」而漏报（有单测锁定）

## 直接依赖

- `tauri`（`State`）
- `crate::core::context::AppContext`（`contest` / `problem` / `submission` 三个 Service）
- `crate::core::error::AppResult`
- `tracing`（`info!`）

## 被依赖

- `main.rs` — `generate_handler!` 注册为 `clear_cache`
- `src/bridge/system.bridge.ts` → `system.service.ts` → `views/SettingsView.vue`「缓存」区块

## 逻辑流程

```
clear_cache
  → ContestService::clear_caches()   // 列表内存 + 元信息内存 + 元信息磁盘；保留公告基线
  → ProblemService::clear_caches()   // 题面内存/磁盘 + limits 内存/磁盘
  → SubmissionService::clear_user_caches()  // 终态详情/测试点内存（含源代码）
  → Ok(())
```

设计要点：

- **同步清理，不投延迟队列**：与 `OJSwitched` 的延迟段（I/O 仅作空间回收）不同 —— 用户显式点了按钮就该等到真清完再看到「已清空」，否则「清空是否生效」不可验证。
- **清空是幂等的、对空缓存安全**：磁盘目录不存在时按「本就没有」处理（`clear_namespace` 返回 `false`、limits 目录先做存在性守卫），不告警、不报错（有单测锁定）。
- **不重拉**：清空与补拉是两件事。前端清完立刻重拉当前比赛数据，否则界面会停在前端 store 的旧内存副本上。
- 用户域缓存（提交详情/测试点）与登出共用 `clear_user_caches()`：两者语义一致（换账号/主动清空后都不得复用上一位选手的提交内容）。

## 测试

`src-tauri/src/service/contest/tests/contest_tests.rs` 与 `src-tauri/src/service/problem/tests/problem_tests.rs` 覆盖 Service 层的清理语义（命令本体依赖 `State` 无法直接单测，见 `commands/mod.md`）：内存 + 磁盘都清、清完不额外发请求、清后必须回源、公告基线被保留、空缓存下不报错。
