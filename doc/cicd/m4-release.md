# CI/CD M4 施工记录：Windows 发布 MVP

## 一、落地范围（分支 chore/ci-m4-release，本 PR）

- 版本一致性门禁：`scripts/check-release-versions.mjs` + `npm run release:check` + `scripts/__tests__/check-release-versions.spec.ts`。
- `.github/workflows/release.yml`：`v*.*.*` 标签触发，五 job（validate / test / build-windows / verify-artifact / draft-release），job 名沿用 m0-baseline §六稳定名。
- 发布文档三件：`CHANGELOG.md`（Keep a Changelog）、`RELEASE.md`（发布手册）、`SECURITY.md`（报告渠道为 GitHub 私密漏洞报告，PVR 随合并启用）。
- `ci.yml` 增加 `workflow_call` 触发器：release.yml 的 test job 复用完整必需 CI。
- `vite.config.ts` vitest include 扩展 `scripts/**/__tests__/**/*.spec.ts`（原 include 不覆盖 scripts）。

## 二、版本门禁设计决策

- 门禁对象为**五处版本**（package.json、package-lock.json 根与 packages[""]、Cargo.toml [package]、Cargo.lock hinina 条目、tauri.conf.json），必须与 package.json 一致。
- 脚本四种模式：无参（一致性报告）、`--tag vX.Y.Z`（格式 `^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$` 且与应用版本一致）、`--changelog`（存在对应 `## [X.Y.Z]` 段落，同时兼容无括号写法）、`--extract-notes`（stdout 只输出段落正文供草稿发布说明，诊断走 stderr）、`--print-version`（verify-artifact 断言产物文件名用）。
- Cargo.lock 解析只认 `name = "hinina"` 条目内的 version，依赖包条目不参与比较。
- 测试：正向用例直接跑真实仓库（0.1.0）；负向用例用 tmp 目录 fixture；做过变异验证（破坏 Cargo.lock 包名判据后正向用例全部变红，已还原）。

## 三、release.yml 结构与关键决策

- 顶层 `permissions: contents: read`；concurrency 按 workflow+ref 分组、cancel-in-progress: false；仅 `draft-release` job 提升 `contents: write` 并挂受保护 environment `release`。
- `validate`：fetch-depth 0 → `git merge-base --is-ancestor` 确认 tag 指向 main 历史 → 门禁脚本 `--tag --changelog`。
- `test`：`uses: ./.github/workflows/ci.yml`（方案 §7.3「调用或复用完整必需 CI」，发布不跳过检查）。
- `build-windows`：步骤照抄 integration.yml windows leg（corepack npm shim、rust-cache `src-tauri -> target`、npm ci + build + tauri build）；构建后生成 build-metadata.json（tag/commit/built_at/node/npm/rustc/cargo/target/bundle/`signed: false`）；artifact 名 `release-windows-unsigned`（如实标注未签名，签名属 M5）。
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

## 七、遗留项

- 桌面 smoke（tauri-driver 6 场景）属 M3 遗留，不在本阶段。
- OS 代码签名属 M5（按发布范围决策只做 Windows 签名，macOS/Linux 产物推迟、不进支持矩阵）；updater 属 M6（前置：至少一次稳定正式发布）。
- 升级安装覆盖验收（`RELEASE.md` §五第 4 项）自第二个版本起适用。
- `CHANGELOG.md` 0.1.0 段落日期为 TBD，首次发版时填入。
