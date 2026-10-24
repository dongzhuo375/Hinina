# Hinina 插件系统

> 代码基线：`main@8379772`（PR #41 squash），文档整理：2026-10-22。
> 代码是事实标准；本目录是插件系统的设计、契约、施工和路线图入口，模块级细节见 [`../modules/src-tauri/plugin/`](../modules/src-tauri/plugin/README.md)。

## 当前状态

已经交付一个**最小可运行的外围 JavaScript 插件宿主**：

- QuickJS（`rquickjs`）隔离 worker，每插件一个 OS 线程、一个引擎实例
- Manifest / API / 事件协议均为 v2
- 作用域只读 API：身份、当前比赛（含 `contestType`/`sealed`）、本人提交历史（不读源码、不读题面）
- 比赛题目列表（题号 ↔ 气球颜色）与全场榜单分页（`contest.rank` 独立权限，`teamKey` 可比对自身）
- 按插件有界事件投递与通用 Resync
- 私有 JSON 存储（原子写、文件锁、CAS）
- 受限 HTTP（精确 origin、DNS 校验并固定、无 Cookie/代理/跳转/重试）
- 纯文本通知
- 宿主日志出口（`hinina.log.*`，写入宿主日志文件，无需权限）
- 参考业务插件：首次 AC 气球派发（`sdk/examples/balloon`，Resync 节拍、封榜门控、幂等派发、含逻辑测试与验收指南）
- start / stop / reload / resync 开发命令、TypeScript SDK、脚手架与离线打包

**尚未交付**：管理 UI、持久启用/禁用与配置写入、安装/更新/回滚、插件市场、配置 schema、系统凭据引用、真实 HOJ/Hydro + 外部服务联调（验收指南已就绪，待人工执行）、Linux/macOS 运行时验证。

## 文档地图

| 文件 | 内容 | 适合谁 |
|---|---|---|
| [architecture.md](architecture.md) | 分层、模块、数据流、生命周期、安全模型、设计取舍 | 宿主开发者、评审者 |
| [contracts.md](contracts.md) | 版本、Manifest、审批、API、事件、查询、Runtime、错误、限流、存储、HTTP、通知、前端 IPC | 插件作者、宿主开发者 |
| [development.md](development.md) | 插件作者快速开始、宿主施工步骤、新增能力流程、验证命令、排错 | 插件作者、宿主开发者 |
| [roadmap.md](roadmap.md) | 完成度、已知缺口、管理页/分发/业务插件优先级、未决问题 | 规划者、维护者 |
| [history.md](history.md) | 审计与评审时间线、关键修复结论 | 需要了解来龙去脉的人 |

模块级参考：

- [插件模块总览](../modules/src-tauri/plugin/README.md)
- [开发教程](../modules/src-tauri/plugin/tutorial.md)
- [Manifest v2](../modules/src-tauri/plugin/host/manifest.md)
- [Plugin API v2](../modules/src-tauri/plugin/api/mod.md)
- [事件与 Resync](../modules/src-tauri/plugin/api/event.md)
- [Runtime](../modules/src-tauri/plugin/runtime/mod.md)
- [安全模型](../modules/src-tauri/plugin/security.md)
- [资源上限](../modules/src-tauri/plugin/limits.md)
- [验证记录](../modules/src-tauri/plugin/validation.md)
- [TypeScript 声明](../modules/src-tauri/plugin/sdk/hinina.d.ts)
- [参考业务插件（首次 AC 气球派发）](../modules/src-tauri/plugin/sdk/examples/balloon/README.md)

## 先读什么

- **写插件**：先读 [development.md §1–2](development.md#1-插件作者快速开始)，再查 [contracts.md](contracts.md) 和 `hinina.d.ts`。
- **改宿主**：先读 [architecture.md](architecture.md) 与 [contracts.md](contracts.md)，再按 [development.md §3–5](development.md#3-宿主施工步骤) 施工。
- **做规划**：直接读 [roadmap.md](roadmap.md)。

## 维护规则

1. **契约变更**必须同步：Rust DTO/trait → JSON 桥 operation → `hinina.d.ts` → 模块文档 → 契约测试；破坏性变更递增对应版本号。
2. **架构/模块变更**必须同步 `architecture.md`、`doc/Architecture.md` 文件树和 `doc/modules/` 对应文档。
3. **进度/缺口变更**同步 `roadmap.md`；历史细节只追加到 `history.md`，不把评审流水账写回架构文档。
4. 代码与文档不一致时以代码为准，并把文档修正视为缺陷。