# 插件接口施工验证记录

施工分支：feat/plugin-contracts。真实审计先提交于 a2563ef；按协议 → API/权限 → Host 队列 → 仓库/Runtime → 文档与验证分阶段施工。工作区原有 Cargo.toml 标记、AGENTS.md、截图与缓存指南不纳入提交。

## 当前验证

- Rust 全量测试：615 通过、0 失败（含主程序编译和 doc-tests）。
- 前端 Vitest：36 个文件、493 项通过。
- 前端 vue-tsc + Vite build：通过；已有大资源包提示，不涉及本轮 UI 改动。
- 文档 SDK：TypeScript strict 类型检查通过；清单示例同时由 Rust 测试校验。
- 修改范围内 Rust 文件：rustfmt 检查通过；没有对仓库其他模块做全局格式化。
- Clippy（offline/all-targets）：检查通过；插件修改范围无告警，现有核心/测试代码共 6 条告警（Default 可派生、布尔断言/比较及无用类型转换）。
- 插件文档本地链接、编码检查及按 Windows CRLF 规则的 git diff --check：通过。

阶段插件相关测试从基线 21 项，逐步覆盖清单、授权、DTO/输入边界、安全错误、队列、并发注册、重连/Resync 和 !Send Runtime 类型接入。测试中的 Runtime 是本地测试替身，不能证明 JS 可执行。

执行命令：cargo test --offline --manifest-path src-tauri/Cargo.toml；cargo clippy --offline --manifest-path src-tauri/Cargo.toml --all-targets；npm test；npm run build。声明检查使用 node node_modules/typescript/bin/tsc --noEmit --strict --target ES2022 --module ESNext --moduleResolution bundler --skipLibCheck doc/modules/src-tauri/plugin/sdk/typecheck.ts。rustfmt 使用 --check --edition 2021 --config skip_children=true，仅传入修改范围文件。

## 文件与模块

协议/清单：plugin/api/event.rs 及协议测试，plugin/host/manifest.rs 及清单测试。清单/API v1，事件协议 v2，点分 type，拒绝未知字段/权限/事件和入口路径逃逸语法。

API/权限：plugin/api/{mod,error,submission,contest,identity,storage,http,notification}.rs 及契约测试；plugin/permission 的 PluginGrant/权限测试。独立 DTO、分页/上下文、稳定错误与纯边界验证，不存在生产执行器。problem/workspace/ui、host/extension 只保留带标注的远期草案。

Host：plugin/host/{plugin_host,event_adapter,api_error}.rs 与测试，main.rs 处理重复启动结果。每插件有界队列 128，白名单/声明/权限过滤，非阻塞入队；满/关闭注销单插件，重连先 Resync，不创建第二套业务总线。

接入：plugin/registry/mod.rs，core/repository 删除旧插件端口，infra/fs_plugin_repo.rs 及占位行为测试；plugin/runtime/mod.rs 及非 Send 接入测试。文件仓库明确 Unsupported，Runtime/Bridge 只有契约，无 Manager 或引擎。

文档：根目录 ARCHITECTURE_REVIEW.md，doc/开发手册.md、Architecture.md、todo.md，插件模块文档（含 README、Manifest/API/事件/权限/Runtime/安全/教程、SDK 声明与类型检查样例），及受影响的 core/context、仓库、infra/main 模块文档。

## 未实现与后续路径

当前没有业务插件、目录发现/入口读取、授权 UI、API Adapter、JS 引擎或安全 I/O 执行器，因为本轮明确只预留接口。替代交付是可编译 trait/DTO/声明、真实事件边界和配套开发文档，不能把完整参考方案的运行验收项标为完成。

后续先实现绑定 PluginGrant/上下文的 Service Adapter，再做私有持久化/CAS 与专用无 Cookie 网络客户端，随后实测 JS 引擎的 Windows/Tauri 兼容、Promise、中断/内存能力，再接加载/停止和外围业务测试。气球首次 AC 判断留给插件，核心 Service 不负责派发或首次 AC 判断。

本轮无 Mock 气球服务器或重启派发集成测试：运行时、HTTP 与存储执行器不存在，这些测试应随实际执行器与业务插件一同落地。
