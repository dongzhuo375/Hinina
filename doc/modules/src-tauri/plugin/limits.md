# plugin/limits

## 职责
Runtime、Manager 和查询桥共用的内部常量模块——执行期限、字节预算、并发与速率上限的唯一事实来源；不向插件开放可调策略。

## 核心类型/函数
纯常量模块，无类型与函数。常量分组如下（均为 `pub(crate)`，改动需同步 [limits 上游文档](../../../plugin/contracts.md)）：

| 分组 | 常量 | 值 |
|---|---|---|
| 执行期限 | `ACTIVATION_TIMEOUT` / `EVENT_TIMEOUT` / `START_TIMEOUT` / `STOP_TIMEOUT` | 10s / 30s / 激活+2s / 5s |
| Provider | `PROVIDER_TIMEOUT`（单请求）/ `SUBMISSION_LOOKUP_TIMEOUT`（get 整体扫描） | 5s / 8s |
| 错误宽限 | `API_ERROR_GRACE`（宿主调用提前返回 timeout 的余量） | 1s |
| 告警节流 | `HANDLER_WARN_INTERVAL` | 60s |
| 桥 | `MAX_BRIDGE_INPUT_BYTES` / `MAX_BRIDGE_OPERATION_BYTES` / `MAX_BRIDGE_RESPONSE_BYTES` | 140 000 B / 64 B / 1 MiB |
| 游标 | `MAX_CURSOR_PAGE_BYTES` / `MAX_CURSOR_BUFFER_BYTES` / `MAX_CURSOR_RESPONSE_BYTES` / `BRIDGE_ENVELOPE_HEADROOM` | 1 MiB / 4 MiB / 1 MiB−256 B / 256 B |
| 查询句柄 | `QUERY_HANDLE_TTL` / `MAX_QUERY_CONTEXTS` / `QUERY_HANDLE_REUSE_AGE` | 300s / 32 / 150s |
| Provider 预算 | `PLUGIN_QUERIES_PER_MINUTE` / `PLUGIN_QUERY_CONCURRENCY` / `SHARED_QUERIES_PER_MINUTE` / `SHARED_QUERY_CONCURRENCY` | 60/min 并发 2 / 120/min 并发 4 |
| 日志出口 | `PLUGIN_LOG_BYTES` / `PLUGIN_LOGS_PER_MINUTE` | 1024 B / 30/min |
| 榜单 | `MAX_RANK_PAGE` / `MAX_RANK_ROWS` / `MAX_RANK_PROBLEMS` / `MAX_RANK_CELLS` / `MAX_RANK_NAME_BYTES` | 100 / 200 / 64 / 6 000 / 96 B |
| 题目列表 | `MAX_PROBLEM_TITLE_BYTES` / `MAX_PROBLEM_COLOR_BYTES` | 256 B / 64 B |
| 榜单速率 | `PLUGIN_RANK_QUERIES_PER_MINUTE` | 6/min |

## 直接依赖
- `std::time::Duration`

## 被依赖
- `plugin::query`、`plugin::execution_api`、`plugin::runtime::quickjs`、`plugin::manager`
- `plugin::api::log`（`MAX_LOG_BYTES` 引用 `PLUGIN_LOG_BYTES`）

## 逻辑流程
- **启动截止 fail closed**：调用方未及时观察到 ready，即使引擎刚完成激活也撤销并记 timeout——启动截止边界宁可误杀不可漏放
- **handler 失败告警节流**：按 (plugin, code) 首次立即输出，`HANDLER_WARN_INTERVAL` 内静默累计，到期合并一条带累计计数，避免持续失败的 handler 按事件速率刷屏
- **宿主调用提前截止**：每次宿主调用以当前入口硬截止前 1 秒（`API_ERROR_GRACE`）为截止点，及时返回可捕获的 timeout，为 JS 的 catch/finally 保留处理时间；晚发起的调用不能越过入口窗口。死循环或执行超出硬期限仍使 worker 失败；单个事件 handler 的异常被隔离，不影响 worker
- **字节预算不是进程 RSS 上限**：桥响应含包络最多 1 MiB；单 native 提交投影页最多 1 MiB；返回给 JS 的整页按 `MAX_CURSOR_RESPONSE_BYTES`（1 MiB 减 256 字节包络余量）校验后才发布游标；每次激活全部保留游标余项合计最多 4 MiB，同时保留 32 游标/1000 条 native 页上限。超限统一 `limit_exceeded`
- **日志出口**：单条 ≤1024 UTF-8 字节、每激活 30 条/分钟；换行/制表写入前拍平为空格，超限返回 `limit_exceeded` 而非静默丢弃
- **榜单分页边界可组合**：page 1..100、每页 ≤200 行、单行 ≤64 列、单页总 cells ≤6 000（行×列乘积单独设界），行文本按**字节**截断（≤96 UTF-8 字节，字符边界安全——256 个汉字是 768 字节，按字符截断会让顶满名字的 200×30 页超过 1 MiB 桥上限），边界内的页保证可完整编码；病态内容（超长 displayId、类型上限数字）由桥上限兜底拒绝。Hydro 单页返回全量榜单，超限返回 `limit_exceeded`。题目列表 title/color 同样按字节截断（≤256/≤64），千题级比赛仍可编码
- **榜单独立速率**：HOJ 每次全榜重算，不能按通用 60/min 计费，否则两个插件即可打满共享预算饿死其他查询；6/min 突发等于配额，覆盖初始多页同步
