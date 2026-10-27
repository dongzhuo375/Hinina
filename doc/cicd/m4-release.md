# CI/CD M4 施工记录：Windows 发布 MVP

## 一、落地范围（分支 chore/ci-m4-release，本 PR）

- 版本一致性门禁：`scripts/check-release-versions.mjs` + `npm run release:check` + `scripts/__tests__/check-release-versions.spec.ts`。
- `.github/workflows/release.yml`：`v*.*.*` 标签触发，五 job（validate / test / build-windows / verify-artifact / draft-release），job 名沿用 m0-baseline §六稳定名。
- 发布文档三件：`CHANGELOG.md`（Keep a Changelog）、`RELEASE.md`（发布手册）、`SECURITY.md`（报告渠道为 GitHub 私密漏洞报告，PVR 随合并启用）。
- `ci.yml` 增加 `workflow_call` 触发器：release.yml 的 test job 复用完整必需 CI。
- `vite.config.ts` vitest include 扩展 `scripts/**/__tests__/**/*.spec.ts`（原 include 不覆盖 scripts）。

## 二、版本门禁设计决策

- 门禁对象为**五处版本**（package.json、package-lock.json 根与 packages[""]、Cargo.toml [package]、Cargo.lock hinina 条目、tauri.conf.json），必须与 package.json 一致。
- 脚本四种模式：无参（一致性报告）、`--tag vX.Y.Z`（格式 `^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$` 且与应用版本一致）、`--changelog`（存在对应 `## [X.Y.Z]` 段落，同时兼容无括号写法）、`--extract-notes`（stdout 只输出段落正文供草稿发布说明，诊断走 stderr）、`--print-version`（verify-artifact 断言产物文件名用；仅读 package.json 不做一致性校验——前提是调用前已跑过完整门禁，release.yml 在同一 job 内先复跑门禁再取版本号）。
- 所有文件读取/JSON 解析失败经 runCli 统一转为一行 `error: …` 并以 exit 1 结束，不抛栈。
- Cargo.lock 解析只认 `name = "hinina"` 条目内的 version，依赖包条目不参与比较。
- 测试：正向用例直接跑真实仓库（0.1.0）；负向用例用 tmp 目录 fixture；做过变异验证（破坏 Cargo.lock 包名判据后正向用例全部变红，已还原）。

## 三、release.yml 结构与关键决策

- 顶层 `permissions: contents: read`；concurrency 按 workflow+ref 分组、cancel-in-progress: false；仅 `draft-release` job 提升 `contents: write` 并挂受保护 environment `release`。
- `validate`：fetch-depth 0 → `git merge-base --is-ancestor` 确认 tag 指向 main 历史 → 门禁脚本 `--tag --changelog`。
- `test`：`uses: ./.github/workflows/ci.yml`（方案 §7.3「调用或复用完整必需 CI」，发布不跳过检查）。
- `build-windows`：步骤照抄 integration.yml windows leg（corepack npm shim、rust-cache `src-tauri -> target`、npm ci + build + tauri build）；构建后生成 build-metadata.json（tag/commit/built_at/node/npm/rustc/cargo/target/bundle/`signed: false`）；**上传前先把 exe 与 metadata 平铺暂存进 `release-staging/`，再以单路径 `release-staging/*` 上传**为 `release-windows-unsigned`——upload-artifact v4+ 以所有搜索路径的最小公共祖先为 artifact 根且通配符后层级保留，若直接同时上传 bundle 深路径与根目录 json，exe 会嵌到 `src-tauri/…` 下、下载侧断言必空（评审一轮发现，见 §八）；artifact 名如实标注未签名，签名属 M5。
- `verify-artifact`：在标签树上复跑门禁 → 下载 artifact → 断言恰一个 exe 且文件名恰为 `Hinina_<版本>_x64-setup.exe`（M0 构建日志实证的 NSIS 命名）、`file` 识别为 Windows PE → `sha256sum` 生成 SHA256SUMS.txt → 上传 `release-windows-verified`。
- `draft-release`：needs [test, verify-artifact]，默认语义任一失败即跳过，不产生半成品草稿；发布说明由 `--extract-notes` 从 CHANGELOG 段落生成；`gh release create --draft`，同标签重跑走 `gh release upload --clobber` + `gh release edit --draft` 复用同一草稿；**遇到已发布同名 Release 显式失败**（发布后产物不可变，修复走新版本）。
- 第三方 action 全部 SHA 固定：沿用现库批（checkout v7.0.1 / setup-node v7.1.0 / upload-artifact v7.0.2 / rust-cache v2.9.3），新增 download-artifact v7.0.0（37930b1c2abaa49bbe596cd826c3c89aef350131，与 upload v7 同主版本配套）。

## 四、本地验证

- 门禁 spec 全绿，含「真实仓库全门禁 `--tag v0.1.0 --changelog` exit 0」用例；变异验证通过。
- release.yml / ci.yml 经 PyYAML 解析通过；job 名与 m0-baseline §六稳定名逐一对上。
- `npm test` / `npm run build` / `npx tsc -p tsconfig.node.json --noEmit` 全绿（计数不记入本文档）。
- Rust 零改动。

## 五、合并时动作（逐项经维护者同意后执行）

1. 启用 GitHub 私密漏洞报告（PVR）——`SECURITY.md` 的报告渠道随本 PR 合入即真。
2. 创建受保护 environment `release`（required reviewer）——draft-release 的人工批准随之生效。
3. 创建 tag Ruleset：限制 `v*` 标签的创建与更新，防止已发布标签被移动。
4. 打首个试跑标签 `v0.1.0` 跑通 Release 工作流，产出草稿 Release 供人工验收（按 `RELEASE.md` §五清单）。

## 六、首 tag 试跑结果（合并后回填）

- 试跑 run：<待回填>
- 草稿 Release 与验收：<待回填>
- 失败路径验证（validate 拒绝不指向 main / 版本漂移的标签）：<待回填>
- 嵌套 ci.yml（workflow_call）在标签运行中的并发表现复核：<待回填，见 §九第 2 条>

## 七、遗留项

- 桌面 smoke（tauri-driver 6 场景）属 M3 遗留，不在本阶段。
- OS 代码签名属 M5（按发布范围决策只做 Windows 签名，macOS/Linux 产物推迟、不进支持矩阵）；updater 属 M6（前置：至少一次稳定正式发布）。
- 升级安装覆盖验收（`RELEASE.md` §五第 4 项）自第二个版本起适用。
- `CHANGELOG.md` 0.1.0 段落日期为 TBD，首次发版时填入。

## 八、评审一轮处置（PR #60）

| 发现 | 核验与处置 |
|---|---|
| 🔴 upload-artifact 多路径上传以最小公共祖先为 artifact 根：bundle 深路径 + 根目录 build-metadata.json 双路径上传会把 exe 嵌到 `src-tauri/target/release/bundle/nsis/` 下，verify-artifact 的 `artifact/*.exe` 与 draft-release 的 glob 全部落空，首个试跑 tag 必在 verify-artifact 失败 | 属实（upload-artifact v4+ 文档语义；integration.yml 此前单路径上传故未暴露）。修复：build-windows 先把 exe 与 metadata 平铺进 `release-staging/`，以单路径 `release-staging/*` 上传——artifact 根即该目录，下载侧 `artifact/` 平铺，下游 verify-artifact / draft-release 的断言与 glob 无需改动 |
| ⚪ readDeclaredVersions 对缺失/损坏 JSON 直接抛栈（exit 1 能挡住但输出不友好） | 属实。文件读取与 JSON 解析经 readTextFile/readJsonFile 转为一行 `error: <位置> is missing or unreadable / is not valid JSON`，runCli 统一 catch 后以 exit 1 结束；新增两条用例（缺失文件、坏 JSON，并断言输出无栈帧） |
| ⚪ `--print-version` 不做一致性校验 | 设计如此（各调用点之前均已跑过完整门禁）；已在脚本头注释写明前提，verify-artifact 的用法是同 job 内先复跑门禁再取版本号 |
| ⚪ spec 的 `mkdirSync(join(target, '..'))` 依赖 `..` 归一化，稍隐晦 | 属实，改用 `dirname(target)` |
| ⚪ 合并前确认 ubuntu rust-test 转绿 | 首轮 run 40377057052 中该 job 因 azure 镜像卡死失败（两轮 apt 均挂满超时，25m37s），`gh run rerun --failed` 后 1m50s 绿——M3 已证实的按 runner 抖动，与本 PR 改动无关 |

## 九、评审二轮处置（PR #60）

| 发现 | 核验与处置 |
|---|---|
| 发布构建缺 `--locked`：`npm run tauri build` 未向 cargo 透传 `--locked`，锁文件与 Cargo.toml 不一致时安装包可能用到 CI 未测过的依赖版本 | 属实（实际是纵深防御缺口：release 的 test job 会在同 SHA 上先跑 `--locked` 的 clippy/test，正常流程下到 build-windows 时锁文件已被证明一致；但发布链路应显式断言而非依赖前置 job 的次序）。修复：`npm run tauri build -- --locked`——tauri CLI 的 `[ARGS]...` 透传给 runner（`npx tauri build --help` 实证），锁不一致时构建显式失败。integration.yml 构建矩阵为同模式但属夜间观测、不入发布链，本 PR 不动，留后续单独处理 |
| 嵌套 ci.yml 的并发组可能与发布运行互相取消（test 经 workflow_call 复用，ci.yml 对非 main ref 设 `cancel-in-progress: true`） | 官方文档核实：workflow syntax 与 reuse 页对「被调用 workflow 的顶层 concurrency 是否生效」均无记载，评审者的不确定成立。结构分析核验为**不可达**：即使该并发被以任何语义采纳，组值含 `github.ref`（= 标签 ref），而同标签的发布运行已被 release.yml 自身 `cancel-in-progress: false` 串行化——该组内永远只有一个成员，不存在可取消的并发对象；不同标签/分支的组天然不同。处置：不改 ci.yml（为不可达场景修改 M1 门禁文件违背最小变更）；试跑 tag 时在 Actions 页复核嵌套运行并发表现并回填 §六 |
| spec 硬编码真实版本 `0.1.0`/`v0.1.0`：升版后即便五个文件一致，真实仓库用例也会假红 | 属实。改为从 package.json 动态读取当前版本（`v${repoVersion}` 参与全门禁用例）。语义升级为持续不变量：任何时候五文件一致、tag 与 CHANGELOG 段落齐备即绿——将来升版 PR 若忘了同步 CHANGELOG 段落，`npm test` 会当场拦下 |
| `--print-version` 注释与行为不符：注释说只读 package.json，实际 runGate 先跑全量一致性（Cargo.lock 缺失会连带走失败） | 属实（一轮时只改了注释没改行为，二轮抓到）。修复：print-version 分支移到一致性计算之前，真正只读 package.json；新增用例「其余四个文件缺失时仍正常出版本号」——旧代码下该用例必红，兼作变异验证 |
| CHANGELOG 重复章节会被静默采用第一个（findChangelogSection 取首个匹配） | 属实。改为扫描全部匹配标题，发现重复即按段落歧义失败（reason 注明 N 个章节）；`--changelog` 与 `--extract-notes` 均受保护；新增用例（重复章节 → 两模式均 exit 1） |
