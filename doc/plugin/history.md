# 插件系统历史与关键修复

> 本文只保留里程碑和会影响行为的关键结论。逐轮评审流水账已删除；需要细节时查 Git 历史和 PR #41。

## 1. 时间线

| 日期 | 基线 | 交付 | 结果 |
|---|---|---|---|
| 2026-10-19 | `d858e03` | 真实代码审计 | 确认上一轮只有契约/事件边界；识别凭证恢复、自动选赛、源码详情、Hydro 分页等约束 |
| 2026-10-20 | `2cef9a1` | PR #40：契约与事件边界 | Manifest/API v1、事件 v2、PluginHost 128 有界队列、仓库移至 `plugin/registry`、Runtime 端口；无 JS 引擎/执行器 |
| 2026-10-22 | `8379772` | PR #41：最小可运行宿主 | QuickJS worker、只读查询、游标、存储/CAS、HTTP、通知、Manager、开发命令、SDK；Manifest/API 升 v2 |
| 2026-10-22 | `8379772` 之后 | 远端删除 6 份本地文档 | 代码不变；`doc/plugin/` 成为新的顶层入口 |
| 2026-10-22 | 本次整理 | 文档收敛 | 审计/评审/复盘合并为 `doc/plugin/`；`Architecture.md` 插件段落精简 |
| 2026-10-24 | `1a1dd20` | PR #43：阶段 A 宿主日志出口 | `hinina.log.*` 四方法，无需权限，1024B/30 次每分钟，级别过滤不扣预算 |
| 2026-10-24 | `4d894cb` | PR #44：阶段 B 题目列表与榜单 | `contest.problems`/`contest.rank`（新权限 `contest.rank`）、`PluginContest` 封榜三字段、`teamKey` 与 `identity.userKey` 同算法 |
| 2026-10-24 | `62613a8` | 阶段 C：参考业务插件 | `sdk/examples/balloon` 首次 AC 气球派发参考实现 + 13 例逻辑测试（mock Hinina）+ 验收指南；vitest include 扩展到 `doc/modules/**/__tests__`；无 Rust 宿主改动 |
| 2026-10-24 | PR #45 评审修复 | 参考插件状态机修复 | 移除被宿主黑名单拒绝的 `content-type` 头；榜单分页适配 6 次/分钟硬配额（每轮 3 页，`limit_exceeded` 只结束翻页不中止轮次）；`sent` 逐成功落盘，`timeout`/`permission_denied`/`invalid_input` 归为死区停止队列；激活期补端点 origin GET 探测，失败原因写入宿主日志（宿主错误码统一 `script_error`）；日志全部经 `note()`；mock 升级为强制宿主规则的 fake host（header 黑名单/URL 形状/配额/30s 截止，20 例）；无 Rust 宿主改动 |
| 2026-10-24 | PR #45 评审修复（二） | 榜单翻页滑动窗口 | 配额从每轮 3 页上限改为 60s 窗口共享（模块态，与宿主预算同生命周期，多轮共花 6 次、无每轮上限）+ 页游标跨轮推进/榜末回绕，深榜单以每分钟 6 页轮转覆盖全榜；timeout 注释区分单请求超时与 handler 截止两个来源；fake host 补日志配额拒绝（`logQuota`）；22 例 |
| 2026-10-24 | PR #45 评审修复（三） | 榜单页上限与游标钉死修复 | 每页行数按题数自适应（`min(200, 6000÷(题数+1))`），40 题比赛的页不再恒被 6000 格上限拒绝；`limit_exceeded` 区分首拒（同页重试）与再拒（判定超限页、跳页推进游标，不再钉死），>200 队 Hydro 整榜被拒时游标仍能轮转；榜单深于 100 页回绕时告警；fake host 补页内容硬限与查询越界校验、配额改用 envelope 时钟（24 例）；README 按 HOJ/Hydro 分开写边界并给出存储写配额量级（约 30 球）；validation/Architecture/todo/roadmap 同步 |
| 2026-10-24 | PR #45 评审修复（四） | 跳页门槛与文档数字修正 | 跳页门槛从「两次被拒」提高到「连续 3 个窗口被拒」（同窗口重试不计数，短暂配额/共享预算抖动不再误跳健康页；上一轮"配额抖动保持旧行为"的声明不成立，已修正）；重试日志不再断言「下一窗口」与「超限页」；跳页路径在第 100 页回绕同样告警；README 修正存储写配额数字（每球约 1 次写、首轮约 56 个，测试证实）、Hydro 补 6000 格上限（自适应行数对 Hydro 无效）、契约行数随题数自适应、轮转中途加题的页号错位；fake host 补存储操作配额（60 次/分钟，get/set/CAS 共享）与收敛测试（25 例）；Architecture.md 文件树补 sdk 目录 |
| 2026-10-24 | PR #45 评审修复（五） | 限流模型与结构修正 | fake host 配额从滚动窗口改为 GCRA 令牌桶（governor `Quota::per_minute`：突发 n、每 60s/n 回补一个，已对照 0.10.4 源码确认）；插件本地记账从固定 60s 窗口改为**相同参数的 GCRA**（固定窗口在窗口边界可超领宿主预算——上一窗口末尾消费后几秒即开新窗口，评审反例证实，"同一时钟下恒保守"论证不成立），等价改由构造保证；信封时间即适配时刻的宿主墙钟，与处理时刻的差异仅为入队排队延迟（至多约一个 handler 时长，只让本地桶滞后、方向保守）；`round()` 拆分为 `pageRank()` 与 `dispatchQueue()`；新增头黑名单契约测试（解析 `api/http.rs` 源码与手抄副本比对，已变异验证）与窗口边界零超领、宿主桶更严格两例回归；README 补日志突发限流说明、GCRA 措辞与存储配额限定；28 例 |

验证记录：

- PR #40：Rust 615 项、前端 36 文件/493 项通过。
- PR #41：Rust 722 项、前端 38 文件/508 项通过；`npm run build`、`plugin:sdk:check`、`plugin:scaffold:build` 通过。
- 逐提交验证：10 个触及 `src-tauri/` 的提交各自独立编译并通过 `cargo test --lib`。
- 阶段 C：前端 39 文件/521 项通过（含 balloon 逻辑测试 13 例）；`plugin:sdk:check`、`plugin:balloon:build`、`plugin:scaffold:build`、`npm run build` 通过；无 Rust 改动。
- PR #45 评审修复：前端 528 项通过（含 balloon 20 例，fake host 强制宿主规则）；`plugin:sdk:check`、`plugin:balloon:build` 通过；无 Rust 改动。
- PR #45 评审修复（一/二两次提交）：前端 530 项通过（含 balloon 22 例，补日志配额与滑动窗口翻页）；`plugin:sdk:check`、`plugin:balloon:build`、`npm run build` 通过；无 Rust 改动。
- PR #45 评审修复（三）：前端 532 项通过（含 balloon 24 例，补页上限跳页与自适应 limit）；`plugin:sdk:check`、`plugin:balloon:build` 通过；无 Rust 改动。
- PR #45 评审修复（四）：前端 533 项通过（含 balloon 25 例，补存储配额收敛）；`plugin:sdk:check`、`plugin:balloon:build` 通过；无 Rust 改动。
- PR #45 评审修复（五）：前端 536 项通过（含 balloon 28 例，本地 GCRA 记账 + 头黑名单契约测试 + 窗口边界/自愈回归）；`plugin:sdk:check`、`plugin:balloon:build` 通过；无 Rust 改动。

## 2. 关键修复

| 问题 | 结论 |
|---|---|
| 提交归属用展示名 | 改为宿主私有 `owner_id`（HOJ uid / Hydro rdoc.uid）；`owner_id` 不序列化，缺失即整页 `provider_error`（fail closed） |
| 转换期间 `current_context` 错误码不一致 | 统一返回 `context_expired`；旧句柄、旧游标、旧授权一起失效 |
| 激活前订阅导致事件窗口丢失 | 成功 activate 后才建立订阅；初始 Resync 补偿激活窗口 |
| `switch_oj` 栅栏过宽 | 栅栏只覆盖 Registry 发布 + Service 缓存清理；持久化/事件发布在栅栏外，不阻塞插件查询 |
| 存储无操作上限 | 每激活 60 次/分钟、工厂合计 240 次/分钟、在途 16；超限在 I/O 前拒绝 |
| start/stop 超时残留 worker | 统一 `failed/timeout` + reaper；旧 worker 未结束前拒绝同 ID 重启；迟到退出不覆盖失败诊断 |
| 游标并发复用/失败丢页 | 锁内原子领取；可重试失败按调用前快照回填；只有 `context_expired` 不回填 |
| 预算先扣令牌再取并发 | 先取并发许可，再扣速率令牌；并发拒绝不扣令牌 |
| 每个 handler 重新取 context 撞 32 上限 | 同 scope/revision 下 150 秒内复用未过期句柄；超龄/scope 变化才新签发 |
| 单个 handler 异常杀死 worker | 逐 handler try/catch；失败计数回传，Manager 主动触发一次 Resync；worker 继续 |
| 无关目录/坏清单阻断所有插件 | 逐条目隔离；扫描上限 128 目录/1024 条目；只有“未找到目标”才报超限 |
| `plugin/` 依赖具体 infra | 存储/HTTP/配置改为端口注入；`plugin` 不引用 `crate::infra`；方向 `infra → plugin` |
| 通知策略在 Store | 通知校验、5 项上限、6 秒过期、单一活动订阅下沉到 `plugin.service`；Store 只应用状态 |
| 激活失败被 stop 改判 | 在发布 ready 前快照取消标志；`stop_racing_readiness_does_not_relabel_a_failure_as_clean` 锁定 |
| 游离 Promise 被误认为支持 | 明确契约：所有宿主 Promise 必须 await 或进入入口返回链；不 drain、不后台调度 |
| IP 字面量 origin | Manifest/审批阶段拒绝 IPv4/IPv6 字面量；运行时仍检查全部 DNS 地址 |
| 插件存储键跨比赛碰撞 | 参考插件记录键带 `contestId` 作用域（`balloon.records:<contestId>`），偏离计划字面 `teamKey:displayId`——teamKey 无比赛维度，同队同题号跨场会漏派 |
| 封榜解封后的批量重放 | 参考插件持久化封榜窗口 `[sealedSince, revealedAt)`，AC 时间落在窗口内的首 A 永久跳过；`revealedAt` 取解封首轮 `envelope.occurredAt`（插件无时钟，时间引用只能来自事件信封） |
| 审批不足的插件静默空转 | 参考插件 activate 用无副作用探针（哑上下文 + 不可能命中的 CAS，有查询上下文时再加一次端点 GET）前置校验权限与 origin 审批；缺失时 `failed` 并把具体原因写入宿主日志——宿主把 activate 的任何 reject 都记为 `script_error`，不读插件抛出的 code，指认以日志为准 |
| 插件 IPC 错误形态 | 只在恰好 `{code, permission?}` 时识别为插件错误；保留真实 message 兜底 |
| 派发请求自带 `content-type` | 宿主对 `content-type` 等头有硬黑名单、并为带体请求自动设置 JSON content-type；插件自带该头即 `invalid_input`，参考插件已移除 |
| 榜单 6 次/分钟配额与单页 200 行/6000 格上限撞上逐页扫描 | 参考插件本地以与宿主相同的 GCRA 参数记账（突发 6、每 10 秒回补 1；模块态，随激活与宿主预算一同重置，等价由构造保证），预算跨轮连续回补，页游标跨轮推进、榜末回绕，深榜单以每分钟 6 页轮转覆盖；每页行数按题数自适应保证页可编码；`limit_exceeded` 同页重试，连续 3 个窗口被拒才跳页（短暂抖动不丢页、游标不钉死），已取到的页照常派发、轮次不中止 |
| `sent` 批量落盘被 30s handler 截止吞掉 | 参考插件每个成功立即落盘；`timeout`/`permission_denied`/`invalid_input` 归为死区停止队列，`pending`→`sent` 可收敛，端点不再承受无界重复流量 |

## 3. 已删除的详细文档

以下内容已合并进本目录，原文件不再维护：

- `doc/plugin-architecture-review.md` → 当前结论进入 [architecture.md](architecture.md) 与本文时间线
- `doc/plugin-review-fixes.md` → 关键修复进入本文 §2
- `doc/插件系统最小实现复盘与管理演进.md` → 进度/缺口/管理页/路线图进入 [roadmap.md](roadmap.md)

如需逐项评审细节，使用 PR #41 描述与分支历史；不要重新引入流水账式文档。