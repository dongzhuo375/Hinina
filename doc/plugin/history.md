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

验证记录：

- PR #40：Rust 615 项、前端 36 文件/493 项通过。
- PR #41：Rust 722 项、前端 38 文件/508 项通过；`npm run build`、`plugin:sdk:check`、`plugin:scaffold:build` 通过。
- 逐提交验证：10 个触及 `src-tauri/` 的提交各自独立编译并通过 `cargo test --lib`。

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
| 插件 IPC 错误形态 | 只在恰好 `{code, permission?}` 时识别为插件错误；保留真实 message 兜底 |

## 3. 已删除的详细文档

以下内容已合并进本目录，原文件不再维护：

- `doc/plugin-architecture-review.md` → 当前结论进入 [architecture.md](architecture.md) 与本文时间线
- `doc/plugin-review-fixes.md` → 关键修复进入本文 §2
- `doc/插件系统最小实现复盘与管理演进.md` → 进度/缺口/管理页/路线图进入 [roadmap.md](roadmap.md)

如需逐项评审细节，使用 PR #41 描述与分支历史；不要重新引入流水账式文档。