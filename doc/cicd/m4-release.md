# CI/CD M4 施工记录：Windows 发布 MVP

## 一、落地范围（分支 chore/ci-m4-release，本 PR）

- 版本一致性门禁：`scripts/check-release-versions.mjs` + `npm run release:check` + `scripts/__tests__/check-release-versions.spec.ts`。
- `.github/workflows/release.yml`：`v*.*.*` 标签触发，五 job（validate / test / build-windows / verify-artifact / draft-release），job 名沿用 m0-baseline §六稳定名。
- 发布文档三件：`CHANGELOG.md`（Keep a Changelog）、`RELEASE.md`（发布手册）、`SECURITY.md`（报告渠道为 GitHub 私密漏洞报告，PVR 随合并启用）。
- `ci.yml` 增加 `workflow_call` 触发器：release.yml 的 test job 复用完整必需 CI。
- `vite.config.ts` vitest include 扩展 `scripts/**/__tests__/**/*.spec.ts`（原 include 不覆盖 scripts）。

## 二、版本门禁设计决策

- 门禁对象为**五个文件的六处版本值**（package.json；package-lock.json 的根 `version` 与 `packages[""].version` 两处；Cargo.toml `[package]`；Cargo.lock 的 `hinina` 条目；tauri.conf.json），必须与 package.json 一致。
- 脚本四种模式：无参（一致性报告）、`--tag vX.Y.Z`（格式 `^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$` 且与应用版本一致；预发布标签不受支持）、`--changelog`（存在对应 `## [X.Y.Z]` 段落，同时兼容无括号写法；带日期时必须是真实存在的 `YYYY-MM-DD`）、`--extract-notes`（stdout 只输出段落正文供草稿发布说明，诊断走 stderr）、`--print-version`（verify-artifact 断言产物文件名用；仅读 package.json 不做一致性校验——前提是调用前已跑过完整门禁，release.yml 在同一 job 内先复跑门禁再取版本号）。`--tag` 与 `--print-version`/`--extract-notes` 组合会显式报错（这两条路径不消费 tag，静默忽略会掩盖打错的标签）。
- 所有文件读取/JSON 解析失败经 runCli 统一转为一行 `error: …` 并以 exit 1 结束，不抛栈。
- Cargo.lock 解析只认 `name = "hinina"` 条目内的 version，依赖包条目不参与比较。
- 测试：正向用例直接跑真实仓库（版本从 package.json 动态读取，升版不会假红）；负向用例用 tmp 目录 fixture；做过变异验证（破坏 Cargo.lock 包名判据后正向用例全部变红，已还原）。

## 三、release.yml 结构与关键决策

- 顶层 `permissions: contents: read`；concurrency 按 workflow+ref 分组、cancel-in-progress: false；仅 `draft-release` job 提升 `contents: write` 并挂受保护 environment `release`。
- `validate`：fetch-depth 0 → `git merge-base --is-ancestor` 确认 tag 指向 main 历史 → 门禁脚本 `--tag --changelog`。
- `test`：`uses: ./.github/workflows/ci.yml`（方案 §7.3「调用或复用完整必需 CI」，发布不跳过检查）。
- `build-windows`：步骤照抄 integration.yml windows leg（corepack npm shim、rust-cache `src-tauri -> target`、npm ci + build + tauri build）；构建后生成 build-metadata.json（tag/commit/built_at/node/npm/rustc/cargo/target/bundle/`signed: false`）；**上传前先把 exe 与 metadata 平铺暂存进 `release-staging/`，再以单路径 `release-staging/*` 上传**为 `release-windows-unsigned`——upload-artifact v4+ 以所有搜索路径的最小公共祖先为 artifact 根且通配符后层级保留，若直接同时上传 bundle 深路径与根目录 json，exe 会嵌到 `src-tauri/…` 下、下载侧断言必空（评审一轮发现，见 §八）；artifact 名如实标注未签名，签名属 M5。
- `verify-artifact`：在标签树上复跑门禁 → 下载 artifact → 断言恰一个 exe 且文件名恰为 `Hinina_<版本>_x64-setup.exe`（M0 构建日志实证的 NSIS 命名）、`file` 识别为 Windows PE → `sha256sum` 生成 SHA256SUMS.txt → 上传 `release-windows-verified`。
- `draft-release`：needs [test, verify-artifact]，默认语义任一失败即跳过，不产生半成品草稿；发布说明由 `--extract-notes` 从 CHANGELOG 段落生成；`gh release create --draft`，同标签重跑走 `gh release upload --clobber` + `gh release edit --draft` 复用同一草稿；**遇到已发布同名 Release 显式失败**（发布后产物不可变，修复走新版本）。
- 第三方 action 全部 SHA 固定：沿用现库批（checkout v7.0.1 / setup-node v7.1.0 / upload-artifact v7.0.2 / rust-cache v2.9.3），新增 download-artifact v7.0.0（37930b1c2abaa49bbe596cd826c3c89aef350131，与 upload v7 同主版本配套）。

## 四、本地验证

- 门禁 spec 全绿，含「真实仓库全门禁 `--tag v<当前版本> --changelog` exit 0」用例（版本动态读取）；变异验证通过。
- release.yml / ci.yml 经 PyYAML 解析通过；job 名与 m0-baseline §六稳定名逐一对上。
- `npm test` / `npm run build` / `npx tsc -p tsconfig.node.json --noEmit` 全绿（计数不记入本文档）。
- Rust 零改动。

## 五、合并后动作（已执行，逐项经维护者同意）

| 动作 | 结果 |
|---|---|
| 启用 GitHub 私密漏洞报告（PVR） | 已启用：`GET /repos/…/private-vulnerability-reporting` 返回 `enabled: true`，`SECURITY.md` 的报告渠道有效 |
| 创建受保护 environment `release` | 已创建（id 25440480289），`required_reviewers` = 维护者；v0.1.1 发布时 `draft-release` 确实停在此处等待批准，批准后才继续。后续加固：`deployment_branch_policy` 由 `null` 改为 custom，仅允许 `v*` 标签（policy id 64530431），并以单 job 重跑验证合法标签仍可部署 |
| 创建 tag Ruleset | 已创建（id 25942934）：`refs/tags/v*` 限制 creation/update/deletion，`bypass_actors` 仅 RepositoryRole 5；推 v0.1.0 时远端日志出现 `Bypassed rule violations … creations being restricted`，证明规则生效且管理员 bypass 可用 |
| 首个发布标签试跑 | `v0.1.0` 构建失败（§六）→ 按标签不可变策略改发 `v0.1.1`（§十四），全链路通过并生成草稿 Release（§六） |

## 六、试跑结果

**首轮（tag v0.1.0 @ 2f737c4，run 40474819558，合并当天）**：

- `validate`：通过（tag 格式、指向 main、五个文件六处版本值一致、CHANGELOG 0.1.0 段落日期 2026-10-27 均过门禁）。
- `test`（嵌套 ci.yml）：frontend / rust-lint / rust-test ×2 / ci-required 全绿，**且外层发布运行全程未被取消**——ci.yml 组名加 `github.event_name` 的修复在真实 tag 运行上得到实证（此前 §十 的互消推演成立）。
- `build-windows`：失败于 `Build desktop app` 步骤——runner 默认 shell 为 pwsh，把 `npm run tauri -- build -- --locked` 的两个 `--` 都吞掉，展开成 `tauri build --locked`，被 tauri 拒收。**失败是显式的**（不会静默用未锁定依赖出包），但命令链在 pwsh 下不可用，见 §十二。
- `verify-artifact` / `draft-release`：按 needs 语义跳过，未产生任何 Release。

**正式发布（tag `v0.1.1` @ `27ea3d4`，run 40483797433）**——该 run 共执行 3 次，产物与说明以最后一次为准：

| 尝试 | 触发方式 | `draft-release` 启动时间（UTC） | 说明 |
|---|---|---|---|
| 1 | 首次 tag 推送 | 11:11:52 | 全链路首次通过；草稿安装包 20,302,515 字节 |
| 2 | 完整重跑（`gh run rerun`） | 12:03:48 | 幂等验证：重建产物并覆盖同一草稿；安装包 20,306,342 字节（与第 1 次不同，NSIS 嵌入时间戳） |
| 3 | 仅重跑 `draft-release` job | 12:33:40 | 验证 `release` environment 的 `v*` 标签部署策略；不重建，附件摘要与第 2 次相同 |

- `validate`：通过（tag 格式、指向 main、五个文件六处版本值 0.1.1、CHANGELOG `## [0.1.1] - 2026-10-27` 均过门禁）。
- `test`（嵌套 ci.yml）：五项全绿，外层发布运行未被取消。
- `build-windows`：通过。日志逐行实证 `##[group]Run npm run tauri -- build -- --locked` → `> tauri build -- --locked`（内层 `--` 在 bash 下存活）。
- `verify-artifact`：通过（恰一个安装包、文件名精确匹配、`file` 识别为 Windows PE、生成 SHA256SUMS.txt）。
- `draft-release`：在 `release` environment 人工批准后通过，创建草稿 Release「Hinina v0.1.1」（`isDraft: true`）。

**草稿当前的附件（以第 3 次尝试为准）**：

| 附件 | 大小 | SHA-256 |
|---|---|---|
| `Hinina_0.1.1_x64-setup.exe` | 20,306,342 | `99071e1152939f87aa4f5a3a20aad965c6bf42bb4bce8e8c0bf56163ae2b42e4` |
| `SHA256SUMS.txt` | 93 | — |
| `build-metadata.json` | 339 | tag `v0.1.1` / commit `27ea3d4` / built_at 12:02:06Z / node 22.14.0 / npm 11.18.0 / rustc 1.96.0 / cargo 1.96.0 / target x86_64-pc-windows-msvc / bundle nsis / `signed: false` |

**发布说明的手动修正**：草稿说明由 tag 提交（`27ea3d4`）里的 CHANGELOG 生成，当时那句是「`v0.1.0` 标签是发布链路的试跑记录」。由于 tag 不移动、而 CHANGELOG 后续改成「0.1.0 是内部试跑版本（试跑标签已清理）」，已用 `gh release edit v0.1.1 --notes-file` 重写草稿说明（**草稿可改，公开后不可改**）。这暴露一条流程约束：**发布说明取自 tag 所指提交，tag 之后对 CHANGELOG 的修正不会自动反映到草稿**，需要在草稿阶段手动同步。

**验收状态**：

- 附件校验（本机）：**已执行一次**（2026-10-27）——把三个附件下载到本地临时目录（D 盘，非仓库工作区），本地 `sha256sum` 重算得 `99071e11…2b42e4`，与发布内 `SHA256SUMS.txt` 逐字符一致，文件大小 20,306,342 与 API 记录一致。
- 干净环境安装验收：**按版本策略推迟**——0.x 不要求，1.0.0 起强制执行，见 `RELEASE.md §五` 版本分级验收（维护者裁定）
- 发布运行不被 test 子调用取消的端到端确认：**三轮均已确认**（v0.1.0 首轮与 v0.1.1 的三个尝试）

**拒绝路径验证（两条，均已实测）**：

| 场景 | 做法 | 结果 |
|---|---|---|
| 标签指向不在 main 历史里的提交 | `v0.1.2` 指向 `docs/m4-v011-record` 分支提交 `1653360` | run 40499550816：`validate` 在 `git merge-base --is-ancestor "$GITHUB_SHA" origin/main` 退出 1；`test`/`build-windows`/`verify-artifact`/`draft-release` 全部 skipped，未产生 Release |
| 标签版本与应用版本漂移 | `v9.9.9` 指向 main HEAD | run 40499658577：门禁输出 `BAD tag v9.9.9: tag v9.9.9 does not match app version 0.1.1` 并 exit 1；下游同样全部 skipped |

两个测试标签验证后即删除（`v0.1.2`、`v9.9.9`），远端最终只剩 `v0.1.1`。

**同标签幂等验证**：对 run 40483797433 做**完整重跑**（`gh run rerun`，非 `--failed`），再次经 `release` environment 批准后：

- Release 列表仍为 **1 个**（未创建重复草稿）；
- 三个附件被 `gh release upload --clobber` 覆盖更新（`updatedAt` 由 11:12:04Z 变为 12:04:00Z）；
- 发布说明由 `gh release edit --notes-file` 重写，内容一致；
- 作业日志末行 `draft=true assets=3`。

**顺带观察**：两次构建的安装包字节数不同（20,302,515 → 20,306,342），NSIS 会嵌入时间戳，因此**不能用文件大小做校验**——这也是 `verify-artifact` 断言「精确文件名 + PE 类型 + SHA256」而不断言大小的原因。

## 七、遗留项

- 桌面 smoke（tauri-driver 6 场景）属 M3 遗留，不在本阶段。
- OS 代码签名属 M5（按发布范围决策只做 Windows 签名，macOS/Linux 产物推迟、不进支持矩阵）；updater 属 M6（前置：至少一次稳定正式发布）。
- 升级安装覆盖验收（`RELEASE.md` §五第 4 项）自第二个版本起适用。
- 草稿 Release 的「收尾」是两步人工动作：environment 批准（已由维护者执行）+ 安装验收后手动公开（`gh release edit v0.1.1 --draft=false`）；后者不在 CI 自动化范围内，也不应由 agent 代点。

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
| 发布构建缺 `--locked`：`npm run tauri build` 未向 cargo 透传 `--locked`，锁文件与 Cargo.toml 不一致时安装包可能用到 CI 未测过的依赖版本 | 属实（纵深防御缺口：release 的 test job 会在同 SHA 上先跑 `--locked` 的 clippy/test，正常流程下到 build-windows 时锁文件已被证明一致；但发布链路应显式断言而非依赖前置 job 的次序）。**三轮修法本身有误，四轮已纠正**，见 §十一 |
| 嵌套 ci.yml 的并发组可能与发布运行互相取消（test 经 workflow_call 复用，ci.yml 对非 main ref 设 `cancel-in-progress: true`） | 二轮处置为「不可达」（组含 tag ref + 外层串行化 → 组内单成员），**该结论于三轮被推翻并修复，见 §十** |
| spec 硬编码真实版本 `0.1.0`/`v0.1.0`：升版后即便五个文件一致，真实仓库用例也会假红 | 属实。改为从 package.json 动态读取当前版本（`v${repoVersion}` 参与全门禁用例）。语义升级为持续不变量：任何时候五文件一致、tag 与 CHANGELOG 段落齐备即绿——将来升版 PR 若忘了同步 CHANGELOG 段落，`npm test` 会当场拦下 |
| `--print-version` 注释与行为不符：注释说只读 package.json，实际 runGate 先跑全量一致性（Cargo.lock 缺失会连带走失败） | 属实（一轮时只改了注释没改行为，二轮抓到）。修复：print-version 分支移到一致性计算之前，真正只读 package.json；新增用例「其余四个文件缺失时仍正常出版本号」——旧代码下该用例必红，兼作变异验证 |
| CHANGELOG 重复章节会被静默采用第一个（findChangelogSection 取首个匹配） | 属实。改为扫描全部匹配标题，发现重复即按段落歧义失败（reason 注明 N 个章节）；`--changelog` 与 `--extract-notes` 均受保护；新增用例（重复章节 → 两模式均 exit 1） |

## 十、评审三轮处置（PR #60）：二轮「不可达」结论推翻，并发组互消实锤并修复

| 发现 | 核验与处置 |
|---|---|
| 二轮第 2 条的「不可达」推理有漏洞：被调用方 ci.yml 的 concurrency 求值本身就是组内**第二个成员**（携带对 tag 恒为 true 的 cancel-in-progress），进入与外层发布运行同名的组时会取消**正在执行的外层运行**——「组内只有一个成员」前提不成立 | 属实，二轮结论收回。实证：GitHub 社区讨论 orgs/community#30708 与本 PR 完全同型（caller 组 `workflow-ref` + cancel true 经 workflow_call 调用 callee → caller 被取消，日志 `Canceling since a higher priority waiting request for 'Build app-refs/heads/main' exists`；根因即被调用上下文 `github.workflow` 解析为调用方名）。**注意证据等级：这是社区实证（另有多方 Stack Overflow 佐证），官方文档对此语义无记载**，三轮的 §十 原文曾把它写成已核实的机制事实，四轮已修正措辞，见 §十一。套用本仓库：release.yml 组 `Release-refs/tags/vX.Y.Z`；ci.yml 被调用时组解析为相同的 `Release-refs/tags/vX.Y.Z`，其 cancel 表达式对 tag ref 为 true → test job 启动时发布运行被自己的子调用取消，首个试跑 tag 必死。**修复**：ci.yml 组名加入 `github.event_name`——但该值在 callee 中是**调用方的原始触发事件**（tag 推送时为 `push`，不是 `workflow_call`），故实际组为 `Release-push-refs/tags/vX.Y.Z`；安全性结论不变（release.yml 自身组无 event 段，恒不同）。**副作用（四轮补记）**：main 上 push 与 workflow_dispatch 由共享一组变为两组，两份 CI 可并行；PR 重推取消旧跑不受影响 |

## 十一、评审四轮处置（PR #60）

| 发现 | 核验与处置 |
|---|---|
| `--locked` 未可靠生效：`npm run tauri build -- --locked` 中 npm 会剥掉 `--`，`tauri build --locked` 被 tauri 自己拒收（exit 2），`--locked` 永远到不了 cargo；三轮 §九 用 `npx tauri build --help` 作证据不成立（只证明 `[ARGS]...` 存在，未验证 npm 转发链） | 属实。本机实证：旧写法 `npm run tauri build -- --locked` → `error: unexpected argument '--locked' found`，**exit 2**；新写法 `npm run tauri -- build -- --locked` 以无害探针 `--bogus-hinina-probe` 验证 → 前端构建通过后错误出自 cargo（**exit 1**，与 tauri 的 exit 2 可区分），证明内层 `--` 存活、参数经 tauri 送达 runner。失败模式澄清：CI 上旧写法是**显式失败**（首个试跑 tag 会在 build-windows 就红），不会静默出包；但修法仍必须换。已改为 `npm run tauri -- build -- --locked`，release.yml 步骤注释写明两种写法的差别 |
| event_name 入组带来新行为：ci.yml 触发器含 push(main) 与 workflow_dispatch，加 event_name 前二者共享组（`cancel-in-progress: false` → 串行排队），加之后分成两组可并行；「独立运行行为不变」对该边界不准确 | 属实。已在 §十 与本表补记该副作用：main 的 push 与 dispatch 现在可并行，PR 重推取消旧跑不受影响。接受此代价——发布链路正确性优先于 main 上偶发的并行（main 推送通常由合并触发，与手动 dispatch 重叠概率低） |
| callee 中 `github.event_name` 解析为调用方原始事件（tag 推送 = `push`），§十 原写的 `Release-workflow_call-…` 字面量错误 | 属实。证据等级：actions/runner#3146 的讨论明确「callee 里 `event_name` 取调用方的值」（社区回复，**无 GitHub 维护者确认**），与 #30708 同级；本轮已复核原帖。§十 已改写为 `Release-push-refs/tags/vX.Y.Z`，分组安全性结论不变 |
| `## [0.1.0] - TBD` 能通过门禁：日期后缀无格式校验，RELEASE.md 要求填日期但无强制 | 属实。门禁现在校验日期后缀必须是合法 `YYYY-MM-DD`：`TBD`/`2026-13-45` 均报 `invalid date` 并 exit 1；无日期后缀的标题仍接受。CHANGELOG 0.1.0 改为 `2026-10-27`。新增 3 条用例（TBD 拒绝、畸形日期拒绝、无后缀接受） |
| cip:false 的排队语义：`queue: single` 下新排队项取消并替换较早的排队项，RELEASE.md §四未记录 | 属实（官方文档）。RELEASE.md §四 补「并发排队语义」段：同一标签短时间内多次触发时只执行最新排队项、更早的被静默取消；正常流程用 `gh run rerun` 不产生新运行 |
| extractNotes 只以 `^##\s` 为结束条件，Keep a Changelog 文末的链接引用块（`[1.2.3]: https://…`）会被并入最后一个版本正文 | 属实（当前 CHANGELOG 无链接引用，属潜在问题）。结束条件增加 `^\[[^\]]+\]:\s` 分支；新增用例断言链接引用块不进入 notes |
| 已解决项复核：spec 26/26（4beb7cc head）、print-version 纯读、重复章节报错 | 确认，无需处置（该轮后 29 项；四轮累计 33 项） |
| §六 四项仍为 `<待回填>`：发布链路尚未端到端验证 | 当时确认；后续已由 v0.1.1 正式运行与负向/幂等验证补齐，见 §六 |

## 十二、试跑首轮发现与处置（合并后，tag v0.1.0）

| 发现 | 核验与处置 |
|---|---|
| `npm run tauri -- build -- --locked` 在本地 bash 实证可用、在 runner 上失败：windows runner 默认 shell 为 pwsh，把两个 `--` 都吞掉，实际展开为 `tauri build --locked`，被 tauri 以 `unexpected argument '--locked'` 拒收（run 40474819558，build-windows 步骤） | 属实（CI 日志逐行实证：`##[group]Run npm run tauri -- build -- --locked` 后紧跟 `> tauri build --locked`）。根因是 npm 的 `--` 转发链受 shell 影响：bash 下内层 `--` 存活、pwsh 下两个都被剥除。**修复**：该步骤显式 `shell: bash`（runner 已具备，corepack/版本校验步骤已在用），并在步骤注释里写明两种 shell 的差别与失败模式（显式失败、不静默出包）。修复需新提交进 main 后重新打 tag——tag 运行使用的是 tag 所指提交里的 workflow 文件，rerun 不会带上修复 |
| 首轮试跑同时验证了两件事（正面结论） | ① `validate` 与门禁在真实 tag 上全通过；② 嵌套 `test`（ci.yml 经 workflow_call）全绿且**外层发布运行未被取消**——§十 的并发组互消推演与修复在真实运行上得到实证 |

## 十三、试跑评审（第五轮）处置：发布链路加固

| 发现 | 核验与处置 |
|---|---|
| artifact `retention-days: 7` 短于 `draft-release` 的人工审批等待期；超期后 `gh run rerun --failed` 仍会失败 | 属实。**恢复路径按评审纠正**：`rerun --failed` 确实无效（产物过期），但**完整重跑 `gh run rerun <id>`（不带 `--failed`）会重建全部产物，不需要重推标签**。处置：两个 artifact 的 retention 提到 **30 天**，并在 `RELEASE.md §四` 增「审批超期」段写明完整重跑方式 |
| `gh release view … > /dev/null 2>&1` 把认证/网络/限流故障也当成「无 release」，随后 create 报出误导性错误 | 属实。改为保留 stderr 并按内容区分：命中 `release not found` / `HTTP 404` 才走 create，其它故障显式失败并打印 stderr |
| `Date.parse` 接受不存在的日期：`2026-02-31` 解析成功并滚到 `2026-03-03`，能过门禁，与「合法日期」说法矛盾 | 属实（本地实证：`Date.parse('2026-02-31T00:00:00Z')` 返回合法时间戳、`toISOString()` 为 `2026-03-03T00:00:00.000Z`）。改为 UTC 往返校验（`toISOString().startsWith(date)`）。新增用例：`2026-02-31` 拒绝、闰年 `2028-02-29` 接受、平年 `2026-02-29` 拒绝 |
| `--tag` 与 `--print-version` / `--extract-notes` 组合被静默忽略（两条路径提前 return） | 属实（实证：`--print-version --tag v0.0.9` exit 0，标签未参与校验）。当前无调用方这么用，但静默忽略会掩盖打错的标签。改为组合即报错 exit 1，新增用例 |
| 「五处版本」措辞与实际六处版本值不符（package-lock.json 查 root 与 `packages[""]` 两处） | 属实。`RELEASE.md §一/§三`、本文件 §二 统一改为「**五个文件的六处版本值**」 |
| `v*.*.*` 会匹配预发布标签（如 `v0.1.0-rc1`），门禁拒绝后产生一个看不出原因的红 run | 属实（workflow 的 tag 过滤无法排除预发布）；但失败本身是正确的。处置：门禁错误文案补明「pre-release and other tag formats are not supported by the release workflow」，`RELEASE.md §三` 同步；新增用例断言该说明存在 |
| （评审新发现，Medium）`integration.yml` windows leg 的 `npm run tauri build -- ${{ matrix.bundleArgs }}` 同样未钉 shell，`--` 透传在 pwsh 下会被剥除，当前仅因 windows 的 `bundleArgs` 为空而侥幸可用 | 属实。已给该步骤加 `shell: bash` 与注释；与 `release.yml` 的注释「步骤照抄 integration.yml」保持同源约束 |

## 十四、标签策略决定：不重指 v0.1.0，改发 v0.1.1

首轮试跑的 `v0.1.0` 指向 `2f737c4`（build 步骤在 pwsh 下不可用的提交）。两条出路：

| 方案 | 代价与风险 |
|---|---|
| 绕过 Ruleset 删除并重建 `v0.1.0` | 需要管理员 bypass 一个被设计为防篡改的标签；把「已推送即不可变」的规则用在第一次试跑上就破例，规则形同虚设 |
| 升版 `v0.1.1`，保留 `v0.1.0` 为失败记录 | 需要改动五个文件的六处版本值 + CHANGELOG；但符合 Ruleset 的设计意图，也让「修复以新版本交付」这条纪律从第一版就成立 |

**决定：采用后者。** 处置：

- 版本值 0.1.0 → 0.1.1（package.json、package-lock.json 两处、Cargo.toml、Cargo.lock 的 `hinina` 条目、tauri.conf.json）；
- CHANGELOG 的版本段落改为 `## [0.1.1] - 2026-10-27`，并注明 `v0.1.0` 是试跑记录、从未产生 Release；
- `RELEASE.md §二` 增加「标签不可变策略」：标签不删除、不重指，发布运行失败时该标签保留为记录，修复以新版本交付；
- `v0.1.0` 标签与其失败 run 保留，作为发布链路的首次真实演练证据。

**后续修订（同日晚，维护者裁定）**：`v0.1.0` 标签已被删除。理由：不可变策略保护的是**已产生 Release 的发布标签**；`v0.1.0` 从未发布过（无 Release、无可下载产物、无外部依赖），把它留在 Tags 里只会让「一版一标签」的语义变模糊。失败证据由 Actions run 40474819558 与本文件 §六 承担。删除经管理员 bypass 执行，属治理决定，非日常流程。

规范含义：`v0.1.0` 永远不会成为已发布版本，首个正式版本是 `v0.1.1`。


## 十五、发布后复审处置（第六轮）

| 发现 | 核验与处置 |
|---|---|
| §六 仍写「待回填」而已有 v0.1.1 运行与草稿 | 属实（合并版本尚未含补齐内容）。本 PR 已将 §六 改为「验收状态」并写入 v0.1.1 逐 job 结果、两条拒绝路径与幂等验证；§十一 的历史行补注「后续已补齐」 |
| 文档数字与措辞过期：§十一 写「29 项」（实际 33）；`Architecture.md` 的 scripts 说明仍写「五处版本」、文件头仍是 2026-10-24 | 属实。改为「该轮后 29 项；四轮累计 33 项」；Architecture 改为「五个文件六处版本值」并把最后更新日期改为 2026-10-27 |
| `RELEASE.md §四` 的「`queue: single`」不是 GitHub 语法 | 属实。改为描述实际语义：并发组默认「同时最多一个运行中 + 一个排队」，新排队项替换较旧的排队项（旧的被取消、不报失败）；并注明 GitHub 没有 `queue:` 配置项 |
| `release` 环境没有部署分支/标签限制（`deployment_branch_policy: null`） | 属实，已加固：改为 custom 策略，仅允许 `v*` 标签部署（policy id 64530431，`required_reviewers` 保留）。验证方式：单独重跑 run 40483797433 的 `draft-release` job，先进入 waiting（说明标签通过了策略校验），批准后成功——合法 tag 部署未被挡住 |
| §十/§十一 引用的 orgs/community#30708 与 actions/runner#3146 未独立核实 | 出处与证据等级：#30708 是 **GitHub Discussions 帖子**（`gh` 读不了 Discussion，只能网页/WebFetch 读取），其正文含 caller 被取消的日志原文；#3146 是 actions/runner 的**用户提问 issue**，内容与文档论断一致，但**无维护者确认**，两人都属社区证据、非官方语义。文档已删除「已复核原帖」这类第三方无法验证的过程陈述，只保留出处、内容要点与证据等级；结论本身另有本仓库三轮真实运行独立佐证（发布运行未被嵌套 test 取消） |

## 十六、第七轮复审处置（发布后）

| 发现 | 核验与处置 |
|---|---|
| §六 记录的是第 1 次尝试的安装包大小（20,302,515），而草稿当前文件是第 2 次构建的 20,306,342；PR 未记录当前校验和，也未说明草稿附件已被替换 | 属实。§六 改为**三次尝试表**（`draft-release` 启动时间 11:11:52Z / 12:03:48Z / 12:33:40Z；此前误用了 deployment 记录创建时间）并列出草稿当前附件表：安装包 20,306,342、SHA-256 `99071e11…2b42e4`、`build-metadata.json` 的关键字段。第 1 次与第 2 次大小不同源于 NSIS 嵌入时间戳；第 3 次不重建、摘要不变 |
| PR 描述称「负向路径仍待回填」，与 diff 矛盾 | 属实（描述写于补齐之前）。已更新 PR 描述，补上负向与幂等结果 |
| 第 3 次尝试（仅重跑 `draft-release`，用于验证部署策略）未记入文档 | 属实。已并入 §六 尝试表与 §五 的环境加固说明 |
| 草稿说明仍是 tag 提交里的旧文案（「v0.1.0 标签是发布链路的试跑记录」），而该标签已删除 | 属实。已用 `gh release edit v0.1.1 --notes-file` 重写草稿说明（草稿可改）。并在 §六 记录这条流程约束：发布说明取自 tag 所指提交，tag 之后修改 CHANGELOG 不会自动同步到草稿 |
| 「已复核原帖」无法被第三方验证 | 属实。见上一行处置：改为标注出处与证据等级，不再写过程性断言 |
| 本机核验此前一次也没做过，而政策说 0.1.1 以 CI 为准 | 维护者要求补一次实测：已把三个附件下载到本地临时目录（D 盘，非仓库工作区），本地重算 SHA-256 = `99071e1152939f87aa4f5a3a20aad965c6bf42bb4bce8e8c0bf56163ae2b42e4`，与发布内 `SHA256SUMS.txt` 逐字符一致。结论写入 §六「验收状态」；**干净环境安装验收仍按政策留到 1.0.0** |
