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
- **覆盖率基线**（本地，2026-10-25，V8，include `src/**`、未加载文件计 0%）：**行 55.74% / 分支 54.41%**。只报告不卡线；棘轮（ratchet）待基线稳定后按 `commands`、`adapter transport`、`views/guards` 等关键区域设置。

## 三、变异验证（施工方案 §5.2 要求）

- `only_mine`：临时翻转为 `false` → `only_mine_is_forced_and_defaults_applied` 变红（断言消息显示服务端实收 `onlyMine=false`）→ 还原后全绿。测试确实锁住生产行为，不是恒真断言。

## 四、施工中发现并处置的问题

1. **测试进程的全局 tracing 订阅器冲突（E 级）**：命令测试并行构造多个 `AppContext::init`，`Logger::init` 争抢进程级唯一全局位（第二次起 panic）；改为 `try_init` 容忍后，又出现装上 DEBUG 级订阅器导致全部用例日志被格式化输出、与 libtest 捕获叠加在 Windows 上引发 STATUS_STACK_BUFFER_OVERRUN 进程崩溃（偶发，2/2 复现后定位）。最终修正：`Logger::init` 在 `cfg(test)` 下**不装全局订阅器**（文件层探测照常），五连跑 775 全绿。已同步 `doc/modules/src-tauri/infra/logger.md`。
2. **OJ 实例 id 大小写敏感**：工厂声明为 `"HOJ"`/`"Hydro"`，测试配置写 `"hoj"` 会被静默跳过注册（warn「配置了未知 OJ」）。已写入 `test_support.md` 的约束节。
3. **vitest 5 移除了 `coverage.all`**：显式给出 `coverage.include` 后未加载的匹配文件默认以 0% 计入，无需该选项（类型层面直接报错发现的）。
4. **CI 首跑暴露 main.spec 两处时序依赖（本地侥幸通过）**：① `main.ts` 的 DEV 分支在 `window` 上 `defineProperty('hininaPlugins')`（不可配置），逐用例重置模块后二次导入必抛 "Cannot redefine property"（CI 以未处理错误判红；本地因动态 import 的异步时序侥幸过）——修法：`vi.stubEnv('DEV', false)` 跳过该分支 + 单次导入；② vite.config 全局 `clearMocks: true` 在每个测试前清 mock 记录，而装配发生在 beforeAll 导入期——修法：导入后立即把调用次数与事件 handler 快照进普通对象再断言。另将 V8 覆盖器无法解析的 `CodeEditor.vue`（Monaco 壳）显式加入 coverage 排除（vitest 本就只是排除并告警，显式化保持日志干净）。

## 五、范围收窄与遗留

- **SettingsView 只测维护区**（3 例）：全页 1668 行、mock 面大，保存/切换/数据目录流留待按需补充（已在模块文档注明）。
- Rust `llvm-cov` 基线未做（用户裁定留 M3 夜间）。
- E2E / 桌面 smoke 属 M3；fmt/clippy 存量清零（E2/E3）与 749/748 计数差异核实属 PR2，本 PR 未动。
- 前端覆盖率对 `src/main.ts` 计 0%（组合根无独立可测面，装配行为已由 `main.spec.ts` 验证但 V8 对入口文件的统计口径如此）——基线已知噪音，不影响区域棘轮。

## 六、验证记录（本地，2026-10-25）

- `cargo test --lib`：775 通过 / 0 失败（并行五连跑全绿，崩溃已消除）。
- `npm test`：43 文件 / 557 通过；`npm run build`（vue-tsc + vite）通过；`npx tsc -p tsconfig.node.json --noEmit` 通过。
- `npm run test:coverage`：产出报告，基线数字见 §二。
- CI 首跑（run 40182012394）：frontend 红（上述 §四.4 的 main.spec 时序问题，本地未能复现）；rust-test 双平台绿、rust-lint 红（E2/E3 预期内）。修复后由后续运行复验 coverage 步骤与 artifact 上传。
