# 插件系统历史与关键修复

> 本文只保留里程碑和会影响行为的关键结论：**时间线每个 PR（或整理事件）一行**，验证数字是当次合并时的快照（不随后续轮次更新，当前数字以测试输出为准）；逐轮评审流水账已删除，需要细节时查 Git 历史与对应 PR。

## 1. 时间线

| 日期 | 基线 | 交付 | 结果 |
|---|---|---|---|
| 2026-10-19 | `d858e03` | 真实代码审计 | 确认上一轮只有契约/事件边界；识别凭证恢复、自动选赛、源码详情、Hydro 分页等约束 |
| 2026-10-20 | `2cef9a1` | PR #40：契约与事件边界 | Manifest/API v1、事件 v2、PluginHost 128 有界队列、仓库移至 `plugin/registry`、Runtime 端口；无 JS 引擎/执行器 |
| 2026-10-22 | `8379772` | PR #41：最小可运行宿主 | QuickJS worker、只读查询、游标、存储/CAS、HTTP、通知、Manager、开发命令、SDK；Manifest/API 升 v2 |
| 2026-10-22 | `8379772` 之后 | 远端删除 6 份本地文档 | 代码不变；`doc/plugin/` 成为新的顶层入口 |
| 2026-10-22 | 本次整理 | 文档收敛 | 审计/评审/复盘合并为 `doc/plugin/`；`Architecture.md` 插件段落精简 |
| 2026-10-24 | `1a1dd20` | PR #43：阶段 A 宿主日志出口 | `hinina.log.*` 四方法，无需权限，级别过滤不扣预算 |
| 2026-10-24 | `4d894cb` | PR #44：阶段 B 题目列表与榜单 | `contest.problems`/`contest.rank`（新权限 `contest.rank`）、`PluginContest` 封榜三字段、`teamKey` 与 `identity.userKey` 同算法 |
| 2026-10-24 | `806101d`（squash） | PR #45：阶段 C 参考业务插件（含五轮评审修复） | `sdk/examples/balloon` 首次 AC 气球派发参考实现 + 验收指南；评审修复的关键结论见 §2（content-type 黑名单、GCRA 本地记账与游标轮转、跳页门槛、逐成功落盘、激活探针与 `script_error` 口径、fake host 强制宿主规则）；无 Rust 宿主改动 |
| 2026-10-25 | `806101d` | PR #46：模块文档五节化 | `doc/modules/src-tauri/plugin/` 31 份文档重构为五节格式；评审发现 9 处签名/依赖/字段错误逐一取证修正；纯文档改动 |
| 2026-10-25 | 本次整理 | 文档整合 | `doc/modules` 的 tutorial/security/validation（无源文件、与 `doc/plugin/` 重复）分别并入 development/architecture/history；文档数字改为常量名 + 行为描述，数值断言由测试守住（见 §3） |

## 2. 验证记录（每次合并时的快照）

- PR #40：Rust 615 项、前端 36 文件/493 项通过。
- PR #41：Rust 722 项、前端 38 文件/508 项通过；`npm run build`、`plugin:sdk:check`、`plugin:scaffold:build` 通过。
- 逐提交验证：10 个触及 `src-tauri/` 的提交各自独立编译并通过 `cargo test --lib`。
- PR #45（squash 合并时）：前端 536 项通过（含 balloon 逻辑测试 28 例，fake host 强制宿主规则）；`plugin:sdk:check`、`plugin:balloon:build`、`npm run build` 通过；无 Rust 改动。
- PR #46：balloon 28/28；链接检查通过；`npm run build` 通过；无代码改动。
- 当前数字以 `cargo test` / `npm test` 输出为准，不在文档中另行维护。

## 3. 关键修复

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
| 固定窗口记账在窗口边界超领宿主预算 | 上一窗口末尾消费后几秒即开新窗口，固定窗口会重置出宿主从未回补的预算（评审反例证实）；参考插件改为与宿主 governor GCRA 同参数的本地记账（等价由构造保证），反例调度固化为回归测试 |
| 榜单配额与单页上限撞上逐页扫描 | 参考插件按宿主 GCRA 参数记账、页游标跨轮推进/榜末与契约上界回绕、每页行数按题数自适应保证页可编码；`limit_exceeded` 同页重试、连续 `SKIP_AFTER_WINDOWS` 个窗口被拒才跳页（短暂抖动不丢页、游标不钉死），已取到的页照常派发、轮次不中止 |
| `sent` 批量落盘被 handler 截止吞掉 | 参考插件每个成功立即落盘；`timeout`/`permission_denied`/`invalid_input` 归为死区停止队列，`pending`→`sent` 可收敛，端点不再承受无界重复流量 |
| 文档数字靠人工同步漂移 | **[contracts.md §10](contracts.md) 是宿主数值上限的唯一事实表**：其他文档叙述行为只引用常量名，各源文件模块文档只记录本文件常量。命名常量由契约测试自动锁定——balloon spec 解析 `limits.rs`/`api/*.rs`/`plugin_host.rs`/`fs_plugin_repo.rs`/`plugin_storage.rs` 与 §10 表格逐项比对（34 项），并锁定插件侧别名与宿主常量的镜像；表格改动不同步代码（或反之）即测试失败。仍以内联字面量存在的少量数值（桥准入 600 次/分钟、各在途信号量、存储/HTTP 操作速率等）暂由评审同步，提取为命名常量后即可纳入同一测试；时间线每个 PR（或整理事件）一行 |

## 4. 已删除/合并的详细文档

以下内容已合并进本目录或模块文档，原文件不再维护；需要逐项细节时查 Git 历史与对应 PR，不要重新引入流水账式文档：

- `doc/plugin-architecture-review.md` → 当前结论进入 [architecture.md](architecture.md) 与本文时间线
- `doc/plugin-review-fixes.md` → 关键修复进入本文 §3
- `doc/插件系统最小实现复盘与管理演进.md` → 进度/缺口/管理页/路线图进入 [roadmap.md](roadmap.md)
- `doc/modules/src-tauri/plugin/validation.md`（本 PR 删除）→ 验证数字进入本文 §2，验收缺口进入 [roadmap.md §2.5](roadmap.md)，验证命令进入 [development.md §5](development.md)
- `doc/modules/src-tauri/plugin/tutorial.md`（本 PR 删除）→ 内容并入 [development.md §1](development.md)
- `doc/modules/src-tauri/plugin/security.md`（本 PR 删除）→ 内容并入 [architecture.md §6](architecture.md)
