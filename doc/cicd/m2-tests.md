# CI/CD M2 施工记录：关键测试补位与覆盖率基线

> 记录日期：2026-10-25  
> 版本锚定：`main` @ b686ef6，分支 `chore/ci-m2-tests`  
> 文档性质：CI/CD 施工过程记录（入库，见 [doc/cicd/](README.md)）；前置：[M1 PR 门禁](m1-pr-gate.md)  
> 状态：四个工作包全部落地并本地验证；PR 待评审

## 一、落地范围（4 个工作包提交 + 记录提交）

| 提交 | 工作包 | 内容 |
|---|---|---|
| `23234c1` | A：真实 Command 测试 | 8 个命令提取 `*_impl(&AppContext)`（logout/switch_oj/reset_client/purge_local_data/local_data_usage/list_contest_submissions/get_contest_rank/list_contest_announcements + 公告已读两命令）；全真上下文测试（`AppContext::init` 跑在 TempDir、HOJ 指向 loopback）；删除 4 个复制等价逻辑用例 |
| `a1c4f17` | B：HTTP/适配器契约测试 | `test_support::loopback` 脚本 server；HOJ 契约（登录/token 头/轮换落盘+事件/提交体/500 不重试）；Hydro 契约（Set-Cookie sid/序列化重定向/身份降级/Bearer）；http.rs 真实传输 5 用例 |
| `8fe65bd` | C：前端关键组合测试 | ProblemSolveView（切题落盘/迟到响应/防错题提交/卸载落盘）、sessionGuard（并发防重入等 5 例）、SettingsView 维护区（3 例）、main.ts 装配（2 例）、problemStore 失败语义（4 例）+ 共享挂载 helper |
| `b6ad769` | D：覆盖率基线 | `@vitest/coverage-v8@^5`；vite.config coverage 配置；`test:coverage` 脚本；ci.yml frontend job 改跑 coverage 并上传 artifact（upload-artifact v7.0.2 SHA 固定） |

## 二、测试计数（口径：vitest / cargo test 实际输出）

- **Rust**：748 → **775**（净 +27 = 新增 31 − 删除 4）。新增构成：loopback 底座 2、命令层 16、适配器契约 8（HOJ 6 + Hydro 2）、http 真实传输 5。删除：`mod_tests.rs` 的 4 个复制等价逻辑用例。
- **前端**：539 → **557**（+18）。构成：sessionGuard 5、ProblemSolveView 4、SettingsView 3、main 2、problemStore 4。
- **覆盖率基线**（2026-10-25，V8，include `src/**`、未加载文件计 0%；列名按报告口径标注）：**行 57.25% / 分支 54.48% / 语句 55.74%**（CI 首跑报告为行 57.27%，环境差异 ±0.02，以同口径重跑为准）。只报告不卡线；棘轮（ratchet）待基线稳定后按 `commands`、`adapter transport`、`views/guards` 等关键区域设置。

## 三、变异验证（施工方案 §5.2 要求）

- `only_mine`：临时翻转为 `false` → `only_mine_is_forced_and_defaults_applied` 变红（断言消息显示服务端实收 `onlyMine=false`）→ 还原后全绿。测试确实锁住生产行为，不是恒真断言。

## 四、施工中发现并处置的问题

1. **测试进程的全局 tracing 订阅器冲突（E 级）**：命令测试并行构造多个 `AppContext::init`，`Logger::init` 争抢进程级唯一全局位（第二次起 panic）；改为 `try_init` 容忍后，又出现装上 DEBUG 级订阅器导致全部用例日志被格式化输出、与 libtest 捕获叠加在 Windows 上引发 STATUS_STACK_BUFFER_OVERRUN 进程崩溃（偶发，2/2 复现后定位）。最终修正：`Logger::init` 在 `cfg(test)` 下**不装全局订阅器**（文件层探测照常），五连跑 775 全绿。已同步 `doc/modules/src-tauri/infra/logger.md`。
2. **OJ 实例 id 大小写敏感**：工厂声明为 `"HOJ"`/`"Hydro"`，测试配置写 `"hoj"` 会被静默跳过注册（warn「配置了未知 OJ」）。已写入 `test_support.md` 的约束节。
3. **vitest 5 移除了 `coverage.all`**：显式给出 `coverage.include` 后未加载的匹配文件默认以 0% 计入，无需该选项（类型层面直接报错发现的）。
4. **CI 首跑暴露 main.spec 两处时序依赖（本地侥幸通过）**：① `main.ts` 的 DEV 分支在 `window` 上 `defineProperty('hininaPlugins')`（不可配置），逐用例重置模块后二次导入必抛 "Cannot redefine property"（CI 以未处理错误判红；本地因动态 import 的异步时序侥幸过）——修法：`vi.stubEnv('DEV', false)` 跳过该分支 + 单次导入；② vite.config 全局 `clearMocks: true` 在每个测试前清 mock 记录，而装配发生在 beforeAll 导入期——修法：导入后立即把调用次数与事件 handler 快照进普通对象再断言。另将 V8 覆盖器无法解析的 `CodeEditor.vue`（Monaco 壳）显式加入 coverage 排除（vitest 本就只是排除并告警，显式化保持日志干净）。
5. **（评审 P1，生产缺陷）快速切题竞态可绕过提交配对**：`openProblem` 对迟到响应无过期防护（无条件写 currentProblem），视图 loadToken 只丢弃旧加载编排、无法撤销 store 写入 —— 「配对标记 B、currentProblem 被改回 A」的脏状态下 displayId 配对放行，B 的代码会以 A 的 problem.id 提交。修复：视图记录本次成功加载的真实 `problemId`，`handleSubmit` 增加 `currentProblem.id === loadedProblemId` 第二道校验；竞态测试的迟到 mock 改为复刻真实写入语义（原 mock 只返回 undefined，测试对实际竞态失明）。
6. **（评审 P2，生产缺陷）Hydro 降级身份入缓存致会话校验恒真**：登录探测失败/匿名时把空 id 占位用户写入已验证用户缓存，`validate_session` 经缓存短路无条件判 `Ok(true)`（匿名判有效、网络失败判有效，违反三态契约）。修复：降级身份只用于登录返回值展示、不写缓存；契约测试延伸到 validate_session（探测失败判 Err、服务端恢复判 Ok(true)、真实回访服务端）。

## 五、范围收窄与遗留

- **SettingsView 只测维护区**（3 例）：全页 1668 行、mock 面大，保存/切换/数据目录流留待按需补充（已在模块文档注明）。
- Rust `llvm-cov` 基线未做（用户裁定留 M3 夜间）。
- E2E / 桌面 smoke 属 M3；fmt/clippy 存量清零（E2/E3）与 749/748 计数差异核实属 PR2，本 PR 未动。
- `src/main.ts`（组合根）与 Monaco 壳 `CodeEditor.vue` 在 coverage 配置中显式排除：组合根无独立可测面（装配行为已由 `main.spec.ts` 验证），后者 V8 覆盖器无法解析（不排除只会得到「排除并告警」的噪音）。

## 六、验证记录（本地，2026-10-25）

- `cargo test --lib`：775 通过 / 0 失败（并行五连跑全绿，崩溃已消除）。
- `npm test`：43 文件 / 557 通过；`npm run build`（vue-tsc + vite）通过；`npx tsc -p tsconfig.node.json --noEmit` 通过。
- `npm run test:coverage`：产出报告，基线数字见 §二（评审修正：初版把 Statements 列误标为 Lines）。
- CI 首跑（run 40182012394）：frontend 红（上述 §四.4 的 main.spec 时序问题，本地未能复现）；rust-test 双平台绿、rust-lint 红（E2/E3 预期内）。修复后由后续运行复验 coverage 步骤与 artifact 上传。

## 十一、评审第二轮（2026-10-25，9 条全部处置）

| # | 意见 | 处置 | 提交 |
|---|---|---|---|
| 1 | Hydro 身份缓存令 validate_session 进程内恒真（服务器吊销 sid 不可见） | `probe_user_context` 拆出 `probe_user_context_uncached`，校验走全新探测（与 HOJ 每次回访同语义）；缓存仅供展示查询 | `256a065` |
| 2 | login 未清旧身份缓存（换号后旧 uid/uname 配新 sid） | `set_session` 后 `set_user(None)`；换号身份来自新探测 | `256a065` |
| 3 | openProblem 无过期响应保护（视图补丁挡提交挡不住显示） | store 内置请求序号，迟到响应丢弃（不写 currentProblem/不占错误态/isLoading 由最新请求收尾）；视图 `loadedProblemId` 校验保留为失败路径第二道防线 | `9127a4d` |
| 4 | logger `try_init` 失败仍置 log_path（has_log_file 虚报） | 安装失败时 log_path=None | `fde71af` |
| 5 | 提交 ID 不匹配静默失败 | 经 EditorConsoleBar 现有错误面给出可见提示 | `9127a4d` |
| 6 | logger 测试构建未使用导入（clippy 阻塞） | 导入与 SharedFileWriter 按 cfg 门控 | `fde71af` |
| 7 | oj 缓存清理测试只覆盖 contest | 扩展到 contest+submission 内存缓存（请求计数观察）；problem 题面磁盘缓存键带 OJ 维度、磁盘命中不发请求为设计行为（已注明） | `6b9f4fd` |
| 8 | 测试文件规范 | commands 测试补 `use super::*;`；contract 测试并入各 `mod_tests.rs`（单一测试模块约定） | `6b9f4fd` |
| 9 | *_impl 重复参数签名 | `list_contest_submissions_impl` 改收 `ListSubmissionsParams` 结构体；其余 3-4 参命令收益不成比例、保持 | `6b9f4fd` |

验证：Rust 779 / 前端 560 全绿；clippy 相对存量 E3 零新增；build/TSC 通过。

### 第二轮复核补严（2026-10-25，用户逐条复核后）

- **#4 补直接测试**：`try_init` 失败分支在单元测试不可达（测试构建不装订阅器），新增**集成测试二进制** `src-tauri/tests/logger_try_init_{fail,ok}_tests.rs`——集成测试链接 lib 时 `cfg(test)`=false，`init` 走生产分支，成败两分支各自真实覆盖（占位订阅器→has_log_file=false；空闲→接管且日志有写入）。两文件必须分离：全局订阅器是进程级单例。
- **#7 补 problem 内存缓存断言**：磁盘题面缓存键带 OJ 维度会被保留，故先 `remove_all` 磁盘层排除干扰——此后 open_problem 是否打网络**只**取决于内存 statement_cache 是否被切换清空（请求计数断言 +1）。
- **#9 补齐参数结构体**：`RankParams`（7 参）/`AnnouncementsParams`/`PurgeParams` 全部结构体化，不再逐参转发。
- **#5 覆盖核实**：`loadedProblemId` 第二道守卫的拒绝分支**已有测试覆盖**（ProblemSolveView.spec 的竞态用例断言 error 含「不一致」），复核时被看漏；「切换中」为第一道守卫的独立分支。
- **#6 clippy 取证**：本地实跑 `clippy --all-targets -- -D warnings`，RC=101 的 7 个错误全部为 M0 存量（config.rs 可 derive impl / hoj mod_tests / cache_tests），本 PR 文件零命中；rust-lint 的 fmt 失败同属 E2 既有。

**施工教训（两次踩坑）**：对已 rustfmt 重排过的文件做字符串替换必须以重新读取的内容为准——本轮两次 python 批量替换因目标串与磁盘文本不符而静默无效（其一导致 git restore 误覆盖未提交修复，其二导致测试断言与扩展代码三处未写入、诊断走了弯路）。多段替换要么用带断言的精确工具（Edit），要么逐段 `assert in` 后再写盘。

**CI 补课**：上述教训的直接代价——CI 首跑 frontend 红（本地 vitest 全绿但 vue-tsc 红）：追加测试段时重犯 matcher 消息参数错误 + 引用未导入类型，而 `vitest run` 不做类型检查、全量 build 又跑在追加之前。修复后以 `npm run build`（vue-tsc）作为追加测试后的**必跑项**，不因单文件 vitest 绿而省略。

## 十二、评审第三轮（2026-10-25，15 条 + 2 条文档，逐条验证后处置）

| # | 意见 | 处置 |
|---|---|---|
| 1 | probe 无条件写共享身份缓存（慢响应晚到跨账号串号） | 代际保护：探测记录发起时会话，响应回来会话已变即整体作废（Err Unknown），不写缓存不作结论（`9705acc`） |
| 2 | validate 晚到的「匿名」判定误踢新登录者 | 同一代际保护覆盖：#1 的作废路径使 Ok(false)/Auth 分支只在会话未变时可达；「服务层与 store 无代际校验」为全局既有窗口（HOJ 同样存在），记录为本 PR 之外遗留 |
| 3 | hydro mod_tests 3 处 fmt | cargo fmt 该文件（0 残留） |
| 4 | 登录降级注释语义相反 | 改回「不得否定已经建立好的会话」并并入降级说明 |
| 5 | problemStore 文档注释拼接错位 | 修复换行 |
| 6 | isLoadingPage 守卫静默 | 三道守卫统一可见反馈（经 store action，见 #14） |
| 7 | 提交缺正向断言 | 新增正向用例：配对正确时 submitCode 以正确题目 ID 调用且无错误（守卫恒拒时旧测试全绿的问题被堵住） |
| 8 | 断言中含赋值 | 状态准备拆为独立语句 |
| 9 | 固定次数 Promise.resolve 等待 | 全部改为 vi.waitFor 等可观察条件（mockRejectedValueOnce 路径不经过默认实现的限制已注明） |
| 10 | 失败用例残留挂载实例 | afterEach 统一卸载（ProblemSolveView 当轮落地；SettingsView 在复核指出后补齐——此前处置表述与事实不符） |
| 11 | ok 集成测试依赖宿主 RUST_LOG | 测试内显式 `set_var("RUST_LOG","info")`（RUST_LOG=warn 下实测复现后修复） |
| 12 | log_path 应反映实际安装路径 | 新增 `ACTIVE_LOG_PATH`（成功安装时记录）：同路径二次 init 沿用已安装路径，进程内从未成功安装则 None |
| 13 | 参数结构体过度 | 回退 5/4/2 参三处为逐参转发（低于 clippy 阈值）；仅 7 参的 RankParams 保留 |
| 14 | 视图直写 submissionStore.error | 新增 `reportSubmitBlocked(message)` action，三道守卫改经 action；展示面契约断言不变 |
| 15 | view-mount 注释与事实不符 | 注释如实化（2 个新 spec 在用、5 个既有 spec 保留手写样板）；settleView 导出 |
| 16 | 四处引用不存在的 contract_tests.rs | 全部改为 mod_tests.rs 契约段 |
| 17 | Architecture.md 缺 src-tauri/tests/ | 补条目并注明「刻意例外」及理由 |

验证：Rust 780+2 集成 / 前端 561 全绿；clippy 存量外零新增；build/TSC 通过。竞态测试经 loopback 底座的响应延迟能力构造（录制与弹出原子化修复了延迟导致的脚本错位）。
