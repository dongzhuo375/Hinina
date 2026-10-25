# CI/CD M1 施工记录：最小 PR CI 与分支保护

> 记录日期：2026-10-25  
> 版本锚定：`main` @ 84cb7f8，分支 `chore/ci-m1-workflow`，PR #50  
> 文档性质：CI/CD 施工过程记录（入库，见 [doc/cicd/](README.md)）；前置：[M0 基线](m0-baseline.md)  
> 状态：CI 已落地（PR #50 合并于 `29b5d6a`）；分支保护、Actions 只读权限与红路径验收均已完成（§五）

## 一、落地范围（PR #50）

| 提交 | 内容 |
|---|---|
| `2bbe7af` | `.gitattributes`（`* text=auto eol=lf` + 图片显式 binary）+ 10 个历史 CRLF 文件的独立规范化提交：仅对已知清单逐文件 `git add --renormalize`，提交后核对全部 `i/lf`，工作区副本刷新为 LF |
| `1e114aa` | E1 修复：`@types/node@^22` devDependency + tsconfig 作用域收敛——`tsconfig.node.json` 显式 `types:["node"]`，app `tsconfig.json` 加 `types:[]` 保持 DOM 语义（不收敛时 `plugin.service.ts` 的 timer 句柄被 node 语义污染，`npm run build` 编译失败）；顺带移除 `vite.config.ts` 无必要的 async 包装（`defineConfig` 重载不匹配，M0 日志中该错误曾被模块缺失错误掩盖） |
| `88a519d` | `.github/workflows/ci.yml`（§二）+ `package.json` 增加 `packageManager: "npm@11.18.0"` |
| `0a354d3` | 文档同步：Architecture.md（.github 树 + 根目录树）、开发手册（分支保护/CI 检查/jsdom/发布流程四处按实际改写）、todo.md CI 项、copilot-instructions |
| `b7ae7ff` | 本记录（含首跑结果）+ doc/cicd 索引更新 + m0 记录闭环标注（npm 决策、E1 已修） |
| `bae74e3` | 评审修复：分支保护措辞改为「M1 收尾时经 API 启用」（避免把目标写成现状） |

## 二、ci.yml 结构与三项施工决策

| Job | Runner | 超时 | 阻塞性 | 内容 |
|---|---|---|---|---|
| `frontend` | ubuntu-24.04 | 15m | **必需**（经汇总） | npm ci → tsc-node → SDK 检查 → vitest → 生产构建 |
| `rust-lint` | ubuntu-24.04 | 25m | **首版非阻塞**（不在汇总 needs 内） | rustfmt --check → clippy --locked -D warnings |
| `rust-test` | ubuntu-24.04 + windows-latest 矩阵 | 45m | **必需**（经汇总） | cargo test --locked --all-targets + --doc；fail-fast: false |
| `ci-required` | ubuntu-24.04 | 3m | **唯一必需检查名** | 汇总 frontend + rust-test，任一 failure/cancelled 即红 |

三项决策（用户裁定）：

1. **E1 随本 PR 修**：frontend 四步骤全绿，必需检查设计完整；实际修复比预期多两层（tsconfig 作用域收敛 + async 包装移除），均在提交 `1e114aa`。
2. **npm 固定**：`packageManager: "npm@11.18.0"` + CI `corepack enable`，runner 与本地同版本，闭环 M0 遗留决策项；本地未启用 corepack 不受影响。
3. **分支保护宽松模式**：必需检查 `ci-required` + 禁 force push，管理员直推保留（单人仓库渐进收紧）。

其它要点：concurrency 按 workflow+ref（PR 可取消旧跑、main 不取消）；顶层 `contents: read`；全部 action 固定 commit SHA（checkout v7.0.1 / setup-node v7.1.0 / rust-cache v2.9.3，2026-10-25 解析）；工具链由 runner 预装 rustup 读 `rust-toolchain.toml` 自动安装（1.96.0 + rustfmt/clippy），无第三方 Rust action；Ubuntu 两个 job 的 apt 步骤照抄 M0 记录 §四。

## 三、首跑结果（run 40173734801，2026-10-25，无缓存冷跑）

| Job | 结果 | 耗时 | 说明 |
|---|---|---|---|
| frontend | ✅ | 70s | 全链路绿，E1 修复在 CI 复验通过 |
| rust-lint | ❌（预期内） | 43s | 失败于 `cargo fmt --check`（E2 存量 220 块），clippy 步骤未执行——**Ubuntu 的 clippy 编译路径本轮实际未验证**，待 PR2 清零 fmt 后首次覆盖 |
| rust-test (ubuntu-24.04) | ✅ | 3m24s | **Rust 套件首次在 Linux 编译并全部通过**（含 apt 依赖步骤、QuickJS C 编译）；M0 §七.3 标记的最大未知项以通过收尾 |
| rust-test (windows-latest) | ✅ | 5m32s | 冷编译 + 全部测试通过 |
| ci-required | ✅ | 4s | 汇总语义实证：frontend + rust-test 全绿 → 绿；同时 rust-lint 红**不**波及汇总——非阻塞设计按预期工作 |

运行整体结论为 failure（rust-lint 红），属预期；合并门禁以 `ci-required` 为准。

## 四、验证边界

- 绿路径由 PR #50 自身验证；红路径已由临时失败 PR 验收（§五.3）：ci-required 红 → 合并 BLOCKED。
- Ubuntu clippy 编译路径未验证（见 §三）；`npm --version` 未加显式断言，由 setup-node（.nvmrc）+ corepack（packageManager）双机制保证。
- 宽松保护下管理员仍可越过红检查合并——单人仓库的已接受边界，如需收紧改 enforce_admins 即可。

## 五、合并后动作（已于 2026-10-25 执行完毕）

1. **分支保护已配置（宽松模式）**：`required_status_checks` contexts=["ci-required"]、strict=true（要求与 main 同步）、`allow_force_pushes=false`、`enforce_admins=false`（管理员直推保留）。
2. **Actions 默认权限已设为只读**：`default_workflow_permissions=read` + 禁止 PR 审批。注意该字段在 `/actions/permissions/workflow` 子端点，主端点 `/actions/permissions` 只管 enabled/allowed_actions。
3. **红路径验收通过（PR #51，run 40175020303）**：故意失败用例使 `frontend` 红 → `ci-required` 红（`rust-test` 双平台仍绿，fail-fast: false 的故障隔离按设计工作）→ PR `mergeStateStatus=BLOCKED`，合并被必需检查阻止。验收后 PR 已关闭、临时分支已删。边界：宽松模式下管理员仍可越过红检查合并（enforce_admins=false），已在 §四 声明。
4. **本节结果回写 + 开发手册/copilot-instructions 中分支保护措辞由「待启用」翻转为现状**（本提交直推 main，宽松模式允许管理员直推）。

## 六、遗留项

- ~~**rust-lint 升必需**~~：条件已达成（lint 门禁清理 PR：E2/E3 清零），rust-lint 已加入 ci-required 的 needs；分支保护 contexts 不变（仍是 ci-required）。
- M0 遗留的 Rust 测试 749/748 计数差异仍待 PR2 核实。
- 前端测试计数基线更新为 39 文件 539 测试（M0 记录为 538；PR #49 为 balloon 示例新增 1 个）。
