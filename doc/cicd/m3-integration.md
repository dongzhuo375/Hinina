# CI/CD 记录：M3 夜间集成（integration.yml + 浏览器 E2E + 压力重复 + Rust 覆盖率基线）

> 记录日期：2026-10-25  
> 版本锚定：`main` @ 56140c5（PR #54 合并后），分支 `chore/ci-m3-integration`  
> 文档性质：CI/CD 施工过程记录（入库，见 [doc/cicd/](README.md)）；对应施工方案 §六（M3）与 m0-baseline §六命名表的 `integration.yml` 条目  
> 状态：workflow 与 E2E 首批场景已落地；三平台构建 / stress 10 轮 / llvm-cov 基线 / E2E 的 **runner 首跑结果在合并后 workflow_dispatch 执行，回写至本文 §六**（不在此预写）

## 一、范围与相对原计划的修正

范围（施工方案 §6.2 + §6.3 + §6.5，经用户确认）：夜间三平台构建矩阵 + 并发用例定时重复 + Rust 覆盖率基线 + Playwright 浏览器 E2E 首批场景。桌面 smoke（§6.4）与真实 OJ 联调（§6.5 前两条）不在本 PR。

原计划获批后、施工前重新核对事实，做了四处修正：

1. **三平台 artifact**：`tauri.conf.json` 的 `bundle.targets` 仅 `["nsis"]`，ubuntu/macos 上该配置不产出任何包。修正为 Windows 出 nsis 安装包、ubuntu/macos 以 `--no-bundle` 验证原生编译并上传主二进制作 smoke artifact；deb/app/dmg 等 bundle 类型扩展留 M4。
2. **新增 rust-coverage job**：m2-tests §五记录的用户裁定「Rust `llvm-cov` 基线留 M3 夜间」，原计划遗漏。
3. **e2e 目录无需排除配置**：根 `tsconfig.json` 只含 `src/**`（vue-tsc 不可见）、vite.config 的 vitest `include` 是显式双 glob（不可见）；反向把 `playwright.config.ts` + `e2e/**/*.ts` 并入 `tsconfig.node.json`，使 CI frontend job 的 `tsc -p tsconfig.node.json` 类型检查 E2E 代码。
4. **观测平台的上报机制**：job 级 `continue-on-error` 会把失败吞成 success，`needs.*.result` 不可信——macOS/Linux 的真实结果由各 build leg 的末步（`if: always()`）自报进 Step Summary，汇总 job 只按阻塞口径判红。

## 二、施工内容（5 个提交）

| 提交 | 内容 |
|---|---|
| `3eedc74` | **integration.yml**：build 矩阵（windows-latest / ubuntu-24.04 / macos-latest，`fail-fast: false`）+ stress-repeat（`concurrent` 7 例 + `auto_save` 8 例，重叠 1 例，循环 10 轮，失败即止不重试）+ rust-coverage（cargo-llvm-cov，`--lib` 口径，无阈值，lcov artifact 保留 14 天） |
| `566c0fa` | **@playwright/test 1.64.1**：playwright.config.ts（chromium 单引擎、webServer 起 vite dev @1420、trace/截图/视频失败保留）、tsconfig.node.json 并入 E2E、.gitignore 补 `test-results/` 与 `playwright-report/` |
| `4f94933` | **fake internals + 首批 spec**：`e2e/fake-tauri.ts` + 四场景 9 用例（见 §三、§四） |
| `3230962` | **e2e job + integration-summary**：E2E 进夜间 workflow；汇总 job 按阻塞口径判红 |
| 本提交 | 文档：本记录 + README 索引 + Architecture.md（workflows 注释修正、根目录树补 e2e/）+ m0-baseline §六表 integration.yml 落地 |

触发与并发：push main / `cron: 30 2 * * *`（每日北京时间 10:30）/ workflow_dispatch；`cancel-in-progress: false`（定时与手动互不取消）；顶层 `permissions: contents: read`。工具链不安装步骤——runner rustup 自动兑现 `rust-toolchain.toml`（1.96.0），与 ci.yml 同模式；Linux 系统依赖清单与 ci.yml 相同（rquickjs 需 C 编译器、reqwest 需 libssl-dev）。

## 三、E2E fake 设计（施工方案 §6.3「受控 Tauri internals fake」）

**原理**：Tauri v2 的命令与事件全部经 `window.__TAURI_INTERNALS__.invoke` 单点（`listen` 也是 `invoke('plugin:event|listen',…)`，`main.ts` 在 mount 前同步消费 `metadata.currentWindow.label`）。因此 `addInitScript` 注入自包含 fake 即可承接全部 IPC——零生产代码改动、零构建开关，fake 只存在于 Playwright 会话。

- **命令表**：auth/config/contest/problem/rank/submission/workspace 域 19 个命令按取证签名实现；**未实现命令 fail-loud**（console.error + `{Unknown}` 拒绝），不得静默兜底。
- **错误形状**：拒绝载荷 `{ Variant: message }` 对象，与 bridge `parseAppError` 的单键对象路径一致（生产 sessionGuard 判定链依赖 `variant === 'Auth'`）。
- **fixtures**：比赛时间动态计算（`startTime = now-1h`、`endTime = now+8h`、status=0），保证恒处 running 赛段（路由守卫按赛段放行，否则已登录用户也会被弹回登录页）；`pollIntervalSecs=1` 快轮询；`oj.contestRef` 必须与比赛 id 一致（登录页简报按它筛选）；工作区按题目真实 pid 内存隔离，`update_workspace_file` 返回递增修订号，`save_workspace` 发 `workspace-saved` 事件。
- **控制面 `window.__e2e`**：`calls` 调用记录（断言去重/轮询次数）、`emitEvent` 事件推送、`failNext(cmd, variant, message)` 一次性错误注入。
- **数据边界**：全部虚构，无真实凭证；IPC 调用日志只留内存，不落 artifact；失败产物仅 trace/截图/报告（保留 7 天）。

## 四、首批场景与本地验证（§6.3 七条的收敛子集）

| spec | 覆盖场景 | 用例 |
|---|---|---|
| auth.spec | 场景 1 | 登录失败（错误可见、停留登录页）/ 登录成功（进入外壳与题目总览）/ 会话恢复（预置 get_session 直达赛场） |
| workspace.spec | 场景 3（部分）+ 4 | 编辑 2s 防抖推送（文件名/内容断言）、切题与回切按 pid 恢复 |
| submission.spec | 场景 5 | pending 期禁用 + 五元组去重（submit_code 恒 1 次）、轮询 Pending→Running→Accepted 终态、提交历史反映会话内提交 |
| session.spec | 场景 6 | Auth 错误 → sessionGuard 全链路（logout 调用 + 域状态清理 + 跳登录页）、失效后重新登录可再次进场 |

实测（口径：Windows 11 本地 + chromium-1248 + vite dev @1420，`PLAYWRIGHT_BROWSERS_PATH=D:\DevTool\pw-browsers`）：**9 passed**，连跑 3 轮全绿（16.2s / 16.4s / 16.4s）。

## 五、施工中的观察（如实记录，未在本 PR 处理）

1. **会话失效文案落地即被清空**：`invalidateSession` 写入的 `SESSION_INVALID_MESSAGE`（authStore，设计意图是登录页展示可执行的下一步），会被登录页挂载时的 `checkSession()` 无条件 `error = null` 清掉——该流程下用户落到登录页看到的是干净表单，文案从未展示。sessionGuard 的清理与跳转本身工作正常（E2E 以 logout 调用为凭证断言）。是否调整（如 checkSession 不清外部写入的 error、或失效跳转不触发 checkSession）留产品决策，见遗留清单。
2. **终态 pill 的双形态**：终态后本地 pill（`最新记录: #9001 Accepted`）会被紧随的摘要刷新接管为服务端 pill（`最新记录: #9001 AC (12 ms)`）；fake 摘要零延迟时本地形态一闪而过。spec 以两种形态的正则断言，属正常时序而非缺陷。

## 六、runner 首跑结果（合并后 workflow_dispatch，回写此处）

- build 矩阵（windows / ubuntu / macos）：待回写
- stress-repeat（concurrent + auto_save × 10 轮）：待回写
- rust-coverage 基线（`cargo llvm-cov --locked --lib`，口径=780 个 lib 单测）：待回写
- e2e（runner chromium）：待回写

## 七、遗留项

- 桌面 smoke 6 场景（tauri-driver + capabilities，§6.4）：真实 Rust invoke/事件往返、Monaco worker 在打包产物可用、工作区重启恢复、窗口操作、关窗前保存、目录选择器。
- 真实 OJ 联调（§6.5）：需独立测试实例与专用测试账号 secrets，仅进受保护的定时/人工环境。
- E2E 剩余 3 场景：赛前深链与开赛自动进入（场景 2）、语言切换与代码上传（场景 3 余项）、榜单/公告/设置最小主流程（场景 7）。
- mac/linux bundle 类型扩展（deb/app/dmg）：属 M4 发布链。
- 会话失效文案被 checkSession 清空的调整评估（§五第 1 条）。
- 生产 HttpClient 代理语义是否显式化（lint-gate-cleanup §五既有观察，保持）。
