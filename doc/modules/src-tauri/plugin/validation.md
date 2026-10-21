# 本轮验证与交付记录

基线main 2cef9a1，新分支feat/plugin-runtime。先追加ARCHITECTURE_REVIEW真实增量审计与选型，再实现引擎验证、访问基础层、查询/I/O/运行时及开发入口。未编写或加载气球业务插件。

## 验证结果

| 检查 | 结果 |
|---|---|
| cargo test --offline --manifest-path src-tauri/Cargo.toml | Rust单元测试通过；历史基线bin/doc-tests通过，当前数量以命令输出为准 |
| cargo clippy --offline --manifest-path src-tauri/Cargo.toml --all-targets | 通过；原有6条warning，涉及config默认实现、旧测试布尔断言/类型转换 |
| rustfmt --edition 2021 --config skip_children=true（本轮Rust文件） | 通过；未全仓格式化 |
| npm test | 前端测试通过，数量以命令输出为准 |
| npm run build | vue-tsc + Vite通过；保留原有大chunk提示 |
| npm run plugin:sdk:check | SDK调用类型和非法能力负例通过 |
| npm run plugin:scaffold:build | 使用Vite锁定的esbuild离线bundle成ES module |

真实引擎覆盖ES module、Rust Future/JS Promise、中断不可捕获循环、内存限制及无ambient I/O。Manager真实JS覆盖身份/比赛/提交元数据/历史、存储读写删/CAS、通知、未授权HTTP拒绝、事件、注销、重载数据保留、多插件失败隔离、同步循环/悬挂Promise停止、无业务事件时周期Resync（测试注入短间隔），以及JS同步批量提交64个Promise时16个桥调用的前置配额。

Query测试覆盖native页余项、不确定total、空native页末尾、ABA/in-flight撤销、错误owner/伪造context/无权限拒绝及metadata重复查询无游标泄漏、scope快照中发生撤销的并发回归。Service测试覆盖同比赛重选不失效、切OJ清选赛、连接保存失败仍撤销、会话删除失败仍撤销身份。

Storage真实文件覆盖跨实例CAS单胜者、missing/null区分、命名空间、重启保留、value/total配额和损坏保留。HTTP策略覆盖特殊IPv4/IPv6、私网额外批准、未授权origin/HTTP拒绝；Mock HTTP server通过私有transport测试真实reqwest的固定地址、无凭证/无redirect、5xx/响应体限额、总超时与取消。测试专用直连loopback只在模块内部调用transport，生产公开API仍拒绝loopback，未开放测试后门。

## 修改文件与职责

- core/access、entity/submission、provider/{registry,submission}：通用访问revision与真实native历史证据。
- service/{auth,config,contest,submission}、infra/provider_registry_impl、adapter/hydro：显式撤销、只读Service及Hydro真实末页语义。
- plugin/api、host、permission、registry、query、execution_api：v2契约、权限快照、不透明句柄/游标、JSON能力桥。
- infra/{fs_plugin_repo,plugin_storage,plugin_http,plugin_notification}：审批/代码读取、锁+原子JSON、隔离网络、文本通知。
- plugin/runtime/{quickjs,bootstrap}、manager、core/context、main、commands/plugin_cmd：真实执行、启停/重载/Resync/退出装配。
- src/types/plugin、bridge/plugin.bridge、services/plugin.service、stores/pluginStore、components/PluginNotifications、App/main：开发入口与纯文本出口，保留既有分层。
- Cargo依赖/锁、package脚本、scripts/build-plugin-scaffold、SDK/scaffold、架构/手册/todo和对应模块文档同步。

## 未进行的验收及后续

未启动完整桌面应用进行人工UI截图验收；通过前端测试/构建和Rust实际worker/网络测试验证。通知未进行截图验收，不声称已完成视觉联调。没有在真实HOJ/Hydro账号/比赛和实际外部气球服务上测试，避免制造真实派发；下一步在专用测试环境按开发教程加载作者插件联调。

当前没有完整JS→真实远端成功请求的生产网络集成：成功/500/跳转/上限/超时在Mock HTTP transport验证，JS集成验证拒绝未授权网络。DNS解析和CIDR策略分别由生产代码与地址策略测试覆盖，尚未做受控DNS服务器重绑定演练。跨OS构建、OS进程沙箱、引擎体积/性能对照、DevTools debugger和source map栈不在本轮。

首次AC/可见历史完整性/外部幂等/重试恢复属于插件业务，故不实现。只交付通用脚手架和能力，不以不存在的气球端到端测试冒充业务验收。Marketplace、暂停/恢复、WASM/UI扩展未实现且没有占位运行路径。

## 审查回归更新

新增Hydro展示名不同/udict缺失的owner校验、转换中签发上下文、OJ组合切换、慢激活期间256条事件、启动超时清理、两个真实JS插件共享预算、并发游标领取、approval隔离、订阅日志锁外写出、存储限流、Client复用、稳定脚本/IPC错误和revision内会话读取去重。公开PluginHttp::request成功路径使用无网络HttpTransport fixture；实际reqwest固定地址和响应策略继续由本地transport测试验证。Provider与HTTP超时用Tokio虚拟时间，周期测试注入短间隔；详见[逐项记录](review-fixes.md)。

PR #41 MEDIUM跟进验证：增加stop超时状态与reaper历史、基础设施日志阶段/脱敏、PluginError穷尽wire契约、域名origin正例及带scheme的IPv4/IPv6负例；通知校验/队列/过期/手动关闭/卸载及订阅失败清理改由Service测试锁定。所有宿主Promise必须await是文档契约，本轮未实现drain或未await检测。数量以当轮测试输出为准。
