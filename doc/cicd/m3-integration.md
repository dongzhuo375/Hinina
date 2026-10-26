# CI/CD 记录：M3 夜间集成（integration.yml + 浏览器 E2E + 压力重复 + Rust 覆盖率基线）

> 记录日期：2026-10-25  
> 版本锚定：`main` @ 56140c5（PR #54 合并后），分支 `chore/ci-m3-integration`  
> 文档性质：CI/CD 施工过程记录（入库，见 [doc/cicd/](README.md)）；对应施工方案 §六（M3）与 m0-baseline §六命名表的 `integration.yml` 条目  
> 状态：夜间链路已建立并经多轮验证（§六首跑/恢复/首验）；评审一~五轮处置分别见 §七/§八/§九/§十/§十一；当前 E2E 14 用例

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

- **命令表**：auth/config/contest/problem/rank/submission/workspace 域 26 个命令按取证签名实现；**未实现命令 fail-loud**（console.error + `{Unknown}` 拒绝），不得静默兜底。
- **错误形状**：拒绝载荷 `{ Variant: message }` 对象，与 bridge `parseAppError` 的单键对象路径一致（生产 sessionGuard 判定链依赖 `variant === 'Auth'`）。
- **fixtures**：比赛时间动态计算（`startTime = now-1h`、`endTime = now+8h`、status=0），保证恒处 running 赛段（路由守卫按赛段放行，否则已登录用户也会被弹回登录页）；`pollIntervalSecs=1` 快轮询；`oj.contestRef` 必须与比赛 id 一致（登录页简报按它筛选）；工作区按题目真实 pid 内存隔离，`update_workspace_file` 返回递增修订号，`save_workspace` 发 `workspace-saved` 事件。
- **控制面 `window.__e2e`**：`calls` 调用记录（断言去重/轮询次数）、`emitEvent` 事件推送、`failNext(cmd, variant, message)` 一次性错误注入。
- **数据边界**：全部虚构，无真实凭证；`calls` 正常仅留内存，**失败时经 `fixtures.ts` 附件写入 `test-results/**/ipc-calls.json`**（§6.3 要求的 IPC 日志，评审三轮补齐），随 trace/截图/报告保留 7 天。

## 四、首批场景与本地验证（§6.3 七条的收敛子集）

| spec | 覆盖场景 | 用例 |
|---|---|---|
| auth.spec | 场景 1 | 登录失败（错误可见、停留登录页）/ 登录成功（进入外壳与题目总览）/ 会话恢复（预置 get_session 直达赛场）+ 事件注销前置全局哨兵 |
| workspace.spec | 场景 3（部分）+ 4 | 编辑 2s 防抖推送（文件名/内容断言）、切题与回切按 pid 恢复 |
| submission.spec | 场景 5 | pending 期禁用 + 五元组去重（submit_code 恒 1 次）、轮询 Pending→Running→Accepted 终态、提交历史反映会话内提交、**pending 跨页往返复用同一提交且不重复发送 IPC** |
| session.spec | 场景 6 | Auth 错误 → sessionGuard 全链路（logout 调用 + 域状态清理 + 跳登录页）、**失效后评测轮询停止（get_judgement 停增，clearDomainState 的可观测凭证）**、失效后重新登录可再次进场 |

实测（口径：Windows 11 本地 + chromium-1248 + vite dev @1420，`PLAYWRIGHT_BROWSERS_PATH=D:\DevTool\pw-browsers`）：评审四轮 **14 passed**（新增「刷新后当前工作区仍在」）；评审三轮 **13 passed**（auth 新增登录后 reload 恢复、workspace 新增编辑后 reload 恢复与全等/负向断言，详见 §九）；此前评审修复轮 **11 passed** 连续 2 轮全绿（20.0s / 18.2s）；M3 合并前为 9 用例 3 轮全绿（16.2s / 16.4s / 16.4s）。

## 五、施工中的观察（如实记录，未在本 PR 处理）

1. **会话失效文案落地即被清空**：`invalidateSession` 写入的 `SESSION_INVALID_MESSAGE`（authStore，设计意图是登录页展示可执行的下一步），会被登录页挂载时的 `checkSession()` 无条件 `error = null` 清掉——该流程下用户落到登录页看到的是干净表单，文案从未展示。sessionGuard 的清理与跳转本身工作正常（E2E 以 logout 调用为凭证断言）。是否调整（如 checkSession 不清外部写入的 error、或失效跳转不触发 checkSession）留产品决策，见遗留清单。
2. **终态 pill 的双形态**：终态后本地 pill（`最新记录: #9001 Accepted`）会被紧随的摘要刷新接管为服务端 pill（`最新记录: #9001 AC (12 ms)`）；fake 摘要零延迟时本地形态一闪而过。spec 以两种形态的正则断言，属正常时序而非缺陷。

## 六、runner 首跑结果（run 40225126527，main @ cd35edd，2026-10-25）

全程约 12.5 分钟（19:15–19:27 UTC）。注：合并 push 与 workflow_dispatch 双触发同 SHA，dispatch 运行已手动取消，以 push 运行为首跑记录。

- **build 矩阵：三平台全绿**（windows 12m / ubuntu 8m / macos 11m）。macOS 首验达成：插件运行时（rquickjs QuickJS C 编译）在该平台首次编译通过。观测注解：macOS leg 的 rust-cache 步骤打出「could not find Cargo.toml」commandFailed 注解（仓库根无清单，cargo metadata 探测落空），未影响构建，保持原样。
- **rust-coverage：✅ 基线 7883/9498 行 = 83.00%**（`cargo llvm-cov --locked --lib`，780 个 lib 单测，ubuntu runner；只报告不卡线）。
- **e2e：✅ 9 passed**（runner chromium，干净环境可重复）。
- **stress-repeat：❌ 第 7/10 轮抓到真实抖动**——`infra::cache::tests::ttl_cache_supports_concurrent_reads_and_writes`（src/infra/tests/cache_tests.rs:197）`assert!(hits > 0)` 失败。根因是**测试设计竞态**，非产品缺陷：4 个读线程先 spawn，主线程后插入 32 个键；runner 高负载下读线程可能在首次插入前就跑完全部 800 次读，hits=0。前 6 轮两过滤串均全绿。按 §6.5 不默认重试，修复走后续 PR（测试侧：插入先于 spawn 或加栅栏对齐，保并发语义不变）。
- **integration-summary：红**（stress-repeat 在阻塞口径内）——判红路径按设计生效；Step Summary 表与 build leg 自报行如实呈现各 job 结果。

**恢复验证**（PR #56 修复后，run 40231321064，main @ ce69dd4，2026-10-25）：6 个 job + integration-summary 全绿，stress-repeat 20 轮过滤跑无抖动——夜间链恢复可用。合并前本地已连跑 10 轮验证确定性。

## 七、遗留项

- ~~**ttl_cache 并发测试竞态修复**（stress-repeat 首跑发现，见 §六）~~ **已处置**（65cd097，评审修复 PR）：种子条目先于读线程插入 + barrier 对齐覆盖写，「至少命中一次」改为确定性下界；本地连跑 10 轮全绿。
- ~~**PR #55 评审跟进项**（另会话评审核实属实）~~ **已全部处置**（评审修复 PR）：
  - ~~fake 缺 `__TAURI_EVENT_PLUGIN_INTERNALS__`~~（3facd84）：补该全局并接到事件注册表清理；auth.spec 增设存在性哨兵。
  - ~~e2e job 缺 node 版本校验步~~（7c0697d）：与 build/frontend job 对齐。
  - ~~fake 保真度~~（3facd84）：get_session/login/logout/validate_session 共享同一 currentUser；16 个命令按 Rust 签名取证必填参数表，缺参以 Tauri 形态裸字符串拒绝（参数回归不再假绿）。
  - ~~spec 强度~~（2166f02）：pending 跨页往返复用同一提交（不发新 submit_code）；失效后 get_judgement 停增断言。store 内部五元组去重行由 submissionStore.spec 单测覆盖（「相同内容评测期间复用编号/修改代码或语言仍能提交/终态释放」），E2E 只断言可观测面——UI 全部调用方都以同一 getter 预守卫，store 去重行不可直达。
- **会话失效文案被 checkSession 清空**（§五第 1 条）：待产品决策，本修复轮未动。
- 桌面 smoke 6 场景（tauri-driver + capabilities，§6.4）：真实 Rust invoke/事件往返、Monaco worker 在打包产物可用、工作区重启恢复、窗口操作、关窗前保存、目录选择器。
- 真实 OJ 联调（§6.5）：需独立测试实例与专用测试账号 secrets，仅进受保护的定时/人工环境。
- E2E 剩余 3 场景：赛前深链与开赛自动进入（场景 2）、语言切换与代码上传（场景 3 余项）、榜单/公告/设置最小主流程（场景 7）。
- mac/linux bundle 类型扩展（deb/app/dmg）：属 M4 发布链。
- 生产 HttpClient 代理语义是否显式化（lint-gate-cleanup §五既有观察，保持）。

## 八、评审二轮处置（PR #57，全部 8 条核实属实并修复）

| 发现 | 处置 |
|---|---|
| build 自报步骤无表头（孤立表格行渲染错乱）且在产物上传前执行，上传失败不被记录 | 自报移到上传之后并把 `artifact_upload.outcome` 一并计入（skipped 视为正常跳过）；表格行改列表行 `build (<os>): <outcome>`（各 leg 完成时间不定，没有共享表头） |
| integration-summary「见上方」措辞误导（leg 摘要在各 job 页面） | 改为明确说明观测平台明细由各 build leg 以列表行写入**本运行摘要页**（各 job 的 Step Summary 汇聚到 run 级摘要） |
| 设了 `PLAYWRIGHT_BROWSERS_PATH` 却无 cache 步骤，每夜重下 Chromium | `actions/cache@v6.1.0`（SHA 固定）restore 该路径，key=`playwright-<os>-<package-lock 哈希>`（Chromium 版本随 @playwright/test） |
| 三处 upload 无 `if-no-files-found: error`，glob 落空只警告、leg 照样绿 | build 产物 / lcov / e2e 失败产物三处全部加 error |
| Tauri apt 依赖清单 5 份拷贝（integration.yml 3 + ci.yml 2） | 抽成复合 action `.github/actions/tauri-system-deps`，5 处调用收敛为单一来源（rust-test 矩阵保留 `if: runner.os == 'Linux'` 守卫） |
| playwright.config 缺 `forbidOnly`，CI 上提交 test.only 只跑一个用例仍退 0 | `forbidOnly: !!process.env.CI` |
| fake 工作区变更命令以空 pid 兜底建幻影工作区（真实后端会拒绝） | `requireLoadedProblemId()` 守卫：未 load_workspace 即以 `{Workspace}` 拒绝（fail-loud 纪律同样适用于此边界） |
| workspace.spec 防抖断言只等 count>=1，可能读到不完整中间快照 | 改为 `expect.poll` 等「携带预期内容」的那次调用出现，再断言最后一次（打字结束后无新防抖）为最终快照 |
| 新增 e2e/ 与 playwright.config.ts 未按 CLAUDE.md 规则补 doc/modules 文档 | 新增 `doc/modules/e2e/`（README：fake 保真度规则 6 条 + 索引；playwright.config.md；fake-tauri.md；specs/ 4 篇）+ Architecture.md 两处树形条目 |

**验证**：本地 YAML 解析（两个 workflow + action.yml）/ tsc node / 561 单测 / 11 E2E 全绿；PR #57 的 ci-required 绿（composite action 在 rust-lint 与 rust-test ubuntu 经实证）后 squash 合并（7d400d8）；合并 push 首验 **run 40276943895 全 7 job 绿**——自报列表行与汇总新措辞生效、Playwright 缓存首次保存（key `playwright-Linux-<lock 哈希>`，后续运行复用）、三处 `if-no-files-found: error` 未误报。

## 九、评审三轮处置（2 个高优先生产竞态 + CI/fake/E2E 缺口 + 文档一致性）

| 发现 | 处置 |
|---|---|
| **P1 旧 submit_code 在清理后重建提交并重启轮询**（submissionStore.submitCode 成功后无条件写条目 + startPolling；startPolling 的存在性检查恰被刚重建的条目满足） | `src/stores/session.ts` 新增模块级**会话代际** `sessionGeneration` / `getSessionGeneration()`，`clearDomainState()` 先递增（OJ 切换同覆盖）；`submitCode` 在请求发起时捕获，响应写回前比对，不一致直接返回（不写条目、不启动轮询），失败分支也只在代际未变时写 error；`startPolling` 读配置前后双校验。Vitest 手动延迟响应回归：`请求在途 → 清理 → 旧响应成功` 后 submissions/pending/polls 全为 0（变异验证：暂存修复时复现 submissions=1/pending=1/polls=1） |
| **P2 迟到 load_workspace 在清理后恢复旧会话代码**（loadGeneration 只管并发切题，clearDomainState 不使在途加载失效） | 同一代际机制接入 `workspaceStore.loadWorkspace`：turn 后与 apply 前置 `getDefaultLanguage` await 后双检查；catch 也只在两代际未变时才写锁状态。回归复现修正前 `workspace=ws-old、code="old session code"` → 修复后 workspace/activeFile/code 保持重置态 |
| 观测平台前置步骤失败被自报为 success（skipped 被视为中性，checkout/setup-node/corepack/apt/rust-cache 失败时三步全 skipped） | 三个采样步骤**必须全部 success**——它们没有「成功路径下正常跳过」的情形；macOS/Linux 的唯一真实结果渠道因此闭合盲区（Windows 本就有 needs.build.result） |
| fake 后端状态随 reload 重建（addInitScript 每次导航重跑，工作区/提交/currentUser 全重置，与真实 Tauri 相反，场景 4 刷新路径未覆盖） | 后端状态（workspaces/submissions/currentUser/revisionCounter/submitSeq/config）序列化到 sessionStorage，bootstrap 先恢复；`PERSIST_CMDS` 成功后写回；文档级 `calls`/`failQueue`/事件回调不持久化。新增 auth「登录后刷新恢复会话」与 workspace「编辑后刷新恢复」两用例（变异验证：无持久化时恰红） |
| fake 与真实契约相反的成功路径（switch_oj no-op、load_configured_contest 固定比赛、漏 switch_oj.ojId 与类型校验、匿名可 submit_code） | switch_oj 校验实例并更新 `oj.active`（未注册/未启用 → ProviderNotFound）；load_configured_contest 校验 contestRef（空 → Config、不匹配 → Contest）；必填表补 `switch_oj.ojId` 并加 ARG_TYPES/OPTIONAL_ARG_TYPES 类型校验（缺参与类型错均拒绝）；submit_code 要求 currentUser（匿名 → Auth，对应 HOJ require_token） |
| #57 的「最终快照」仍是子串；隔离只有正向包含 | workspace.spec 改为等「完整目标内容」的调用并断言最后一次**全等**；隔离用例加双向负向（B 不含 A 的编辑、A 不含 B 的初始代码） |
| `corepack enable` 不含 npm shim，注释却声称按 packageManager 固定版本（M3 复制 M1 的错误假设） | 三处改为 `corepack enable npm`（ci.yml frontend + integration build/e2e）。本地隔离 shim 目录实测：npm/npx shim 建立且 `npm --version` 解析到 packageManager 的 11.18.0 |
| §6.3 要求失败保留 IPC 日志，但 `calls` 只留内存 | 新增 `e2e/fixtures.ts` 共享 test 入口：失败时把 `window.__e2e.calls` 写入 `testInfo.outputPath('ipc-calls.json')` 并 attach（临时失败用例实测落盘 14 条，顺序正确），随 test-results/HTML 报告上传 |
| 文档漂移：doc/cicd/README 仍写 9 用例；m3 状态行仍写「待回写」；失效文案遗留项重复；doc/modules 缺生产代际与 fake 新机制 | README 更正为 13；m3 状态行改为多轮验证已记录；删除重复遗留项；session.md/submissionStore.md/workspaceStore.md 补会话代际与回归；doc/modules/e2e/ 补 fixtures 索引、fake 持久化与契约、spec 新用例与全等/负向断言 |

**验证**：两条生产回归均经**变异验证**（暂存生产修复后恰好对应用例失败）；reload 两用例同样经变异验证（无持久化时恰红）；`corepack enable npm` 与 IPC 附件均本地实测；`npx tsc -p tsconfig.node.json`、561+2 新增单测、13 E2E 全绿（详见 PR 描述）。

## 十、评审四轮处置（会话代际覆盖面、锁复位、CI 证据与 fake 保真度）

| 发现 | 核实与处置 |
|---|---|
| **#1 会话代际覆盖不完整**：只有 submitCode/startPolling/loadWorkspace 使用，fetchHistory、loadMyStatus、loadRank、loadContest、announcement、getProblem 等写入方未守卫 | 属实的部分全部补守卫：problemStore（openProblem/loadLimits/loadMyStatus）、contestStore.loadContest、rankStore（loadRank/fetchAllRows）、announcementStore（load/markAllRead）、submissionStore.fetchHistory 与收尾。**一处修正**：`fetchHistory` 的迟到写入落在 `$reset()` 原替换掉的旧 history 对象引用上，实际不泄漏——守卫保留为纵深防御，且不写误导性回归用例（已实测去掉守卫该用例仍绿，说明它证明的不是守卫）。`loadMyStatus` 与 finally 锁两条新用例经**变异验证**（去守卫恰红）。`loadBrief`（登录页匿名简报）按设计跨登出保留，不比对代际（文档已写明边界） |
| **#2 submitCode finally 无条件复位锁** | 属实：`finally` 改为仅代际未变时复位 `isSubmitting`；新增「旧提交 finally 不得清掉新会话在途提交锁」用例并变异验证 |
| **#3 round-3 工作流改动未在 runner 验证**（integration.yml 无 PR 触发，最近运行 SHA 是 main 9c1a5cd） | 属实。处置：在 PR 分支上 `gh workflow run integration.yml --ref fix/m3-review-round3` 取得 runner 证据。**run 40296962913（94959f8）**：build 三平台 / e2e / rust-coverage **全部绿**（composite action、`corepack enable npm`、三步全 success 自报、Playwright 缓存恢复均在 runner 上实证）；stress-repeat 在 `sudo apt-get update` 上无响应、耗满 15 分钟预算后被 job 超时取消（非测试失败）。已修：apt 加 `Acquire::Retries=3` + 20s 连接超时，stress job 超时 15→30 分钟。**重跑 run 40299460935（344a095）：7 job + integration-summary 全绿**——stress-repeat 通过；e2e 日志实证 `Cache restored from key: playwright-Linux-<lock 哈希>`。 |
| **#4 e2e 类型检查不在任何门禁** | **不属实**：`ci.yml` frontend job 早已执行 `npx tsc -p tsconfig.node.json --noEmit`，且 `tsconfig.node.json` 的 include 含 `e2e/**/*.ts`（`--listFiles` 实证 6 个 e2e 文件在编译程序内）。为可发现性把步骤改名为 `Type-check config and e2e sources` 并加注释 |
| **#5 fake 未持久化 lastWorkspaceId** | 属实：`lastWorkspaceId` 纳入 `PersistedBackend`；`__e2e.backendState()` 暴露快照；新增「刷新后当前工作区仍在」（在题目总览 reload，该页不 load_workspace）用例可区分持久化与否 |
| **#6 session.ts ↔ store import 环** | 属实：计数器移到叶子模块 `src/stores/sessionGeneration.ts`（`getSessionGeneration`/`bumpSessionGeneration`），session.ts 只负责递增、store 只读取；新增模块文档，store 文档依赖清单同步 |

**验证**：前端 **568 单测**（565 + 跨会话去重/保存收尾/轮询收尾三条）、**14 E2E**、`npm run build`、`tsc -p tsconfig.node.json`（`--listFiles` 实证 6 个 e2e 文件在编译程序内）、fmt/clippy 全绿；本轮三条修复均经**变异验证**（还原旧实现后对应用例恰红）。四轮的三处守卫/持久化同理。

## 十一、评审五轮处置（跨会话去重回归 + 两处收尾残留）

| 发现 | 核实与处置 |
|---|---|
| **回归：`loadContest` 去重复用跨会话旧请求**（`loadInFlight` 模块级、`clearSessionData()` 不清；旧 promise 的结果被代际守卫丢弃 → 新会话静默空数据，无错误提示） | 属实。双保险：① 登记在途请求时一并记录**所属代际**，`whenLoaded`/`loadContest` 只在代际相同时复用；② `clearSessionData()` 同时注销 `loadInFlight`/token/代际登记。新增回归用例（旧请求在途 → 清理 → 新会话加载必须发新请求并写入新数据），**变异验证**（还原为「无条件复用 + 不注销」即红） |
| 残留：`pollResult` 无守卫，旧会话在途轮询返回时 `error = null` 抹掉新会话错误提示 | 属实。`pollResult` 代际未变才写条目/清 error；`pollOnce` 代际未变才做收尾（`invalidateMyStatus`/失败计数）。新增用例（旧轮询返回后新会话 error 保持），**变异验证**通过 |
| 残留：`saveWorkspace` 在 await 后写 `isDirty = false`，旧会话保存落定可能清新会话脏标记（假「已保存」） | 属实。`saveWorkspace` 与 `flushPendingSync` 均加代际守卫（旧会话推送落定不推进修订水位、不改 `syncPending`/`isDirty`）。新增用例（旧保存落定不清新会话脏标记），**变异验证**通过 |

**一处自查撤回**：曾顺带给 `markPersisted`（`workspace-saved` 事件）加「当前工作区加载代际」守卫，但事件载荷不携带代际、既有 6 条用例（手工构造状态、不经 loadWorkspace）因此全红，属超出本轮范围的推测性改动——已撤回，仅保留被点名的 flush/save 与轮询修复。事件路径的既有防线（workspaceId 过滤 + revision 幂等/水位）不变。

## 十二、评审五轮补充核验（markPersisted 不变量 + 跨会话端到端覆盖）

| 事项 | 核验与处置 |
|---|---|
| `markPersisted` 跨会话安全性的依据 | **修正前提**：`find_or_create` 会恢复同一 contest+problem 的既有工作区（**同一 id**），因此「已加载且 id 不同会被过滤」不是常见路径。真正的保证是：后端 `revision` 是 WorkspaceManager 进程级 `Arc<AtomicU64>`（从不随登出重置）、且只在磁盘确已写入该 revision 的快照且快照仍最新时才发布事件 —— 「事件 revision ≥ 已推送 revision」等价于「磁盘已含推送内容」。再叠加 `workspaceId` 过滤与 `syncPending` 推迟。**处置**：不新增代际守卫（会误挡新会话自己的事件），把不变量写进 `markPersisted` 文档注释、workspaceStore 模块文档与 sessionGeneration 边界，并新增「同 id 旧会话迟到事件」专项用例（水位归零后 rev 3 不清脏、rev 5 清脏） |
| 跨会话守卫只有 store 单测、E2E 无端到端路径 | 已补：fake 新增 `delayNext(cmd, ms)`；session.spec 新增「提交在途时失效」端到端用例。**过程中发现并修正两个会掩盖回归的问题**：① fake 的 `submit_code` 登录校验在延迟之后执行，导致延迟期间登出后旧请求被拒、永远构造不出「迟到成功」——校验改到命令发起时（先于延迟）；② 原用例只等 2s，轮询首 tick（1s±20%）可能晚于断言——改为等 `submit_code` 真正记账后再等两个周期。修正后**变异验证**：移除 submitCode 守卫，用例恰红（`get_judgement = 2`） |
| 当前验证口径 | 前端 **569 单测**（568 + markPersisted 跨会话核验）、**15 E2E**、build/tsc/fmt 全绿 |
