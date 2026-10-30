# 发布手册（RELEASE）

Hinina 的 Windows x64 NSIS 发布流程（M4 范围）。OS 代码签名接入为 M5 范围、自动更新为 M6 范围，本手册不涉及；两项接入后在本手册追加对应步骤。

## 一、版本准备

1. 确认五个文件里的六处版本值一致：`npm run release:check`（package.json；package-lock.json 的根 `version` 与 `packages[""].version` 两处；src-tauri/Cargo.toml；src-tauri/Cargo.lock 的 `hinina` 条目；src-tauri/tauri.conf.json）。
2. 在 `CHANGELOG.md` 将 `## [Unreleased]` 内容整理进新版本段落，并填入发布日期。
3. 版本推进经普通 PR 合入 main，等待 ci-required 绿；打标签前保持工作树干净。

## 二、打标签

```bash
git tag vX.Y.Z          # 必须指向 main 历史上的提交，且与应用版本一致
git push origin vX.Y.Z
```

推送标签后 Release 工作流自动运行；标签受 Ruleset 保护，不可随意创建/移动。

**标签不可变策略**：**已产生 Release 的标签**不删除、不重指（Ruleset 对 `refs/tags/v*` 禁止创建/更新/删除，仅仓库管理员可 bypass，且正常发布流程不使用 bypass）。若某个标签的发布运行失败，修复后**以新版本号重新发布**——不在旧标签上重试、不移动标签。

例外只针对**从未发布过的试跑标签**（没有 Release、没有可下载产物、无外部引用）：可以经维护者决定后删除，避免留下永远没有 Release 的标签污染 Tags 页面。示例：`v0.1.0` 是试跑失败版本（构建失败、未产生 Release），按此规则删除；首个正式版本为 `v0.1.1`。已发布版本的标签不适用该例外。

## 三、工作流阶段（.github/workflows/release.yml）

| Job | 说明 |
| --- | --- |
| validate | 校验 tag 格式（仅 `v<major>.<minor>.<patch>`，预发布标签不受支持）、tag 指向 main 历史、五个文件六处版本值一致、CHANGELOG 段落存在（含真实日期） |
| test | 通过 workflow_call 复用完整必需 CI（ci.yml），发布不跳过任何检查 |
| build-windows | Windows x64 NSIS 构建，产出安装包与 build-metadata.json（tag/commit/工具链版本） |
| verify-artifact | 校验安装包文件名（`Hinina_<版本>_x64-setup.exe`）与 PE 文件类型，生成 SHA256SUMS.txt |
| draft-release | 在受保护 environment `release`（人工批准 + 仅允许 `v*` 标签部署）中创建或更新草稿 Release |

## 四、重跑

- 个别 job 失败：`gh run rerun --failed <run-id>`。
- 同一标签重跑会复用并更新**同一个草稿**，不创建重复 Release。
- 已正式发布的 Release 不可变：重跑遇到已发布的同名 Release 会显式失败，修复应走新版本。

**审批超期**：`draft-release` 会在受保护 environment `release` 等待人工批准，期间构建产物需存活。产物保留 30 天；若草稿步骤等待超过该期限，`gh run rerun --failed` 会因产物已过期而再次失败，此时用**完整重跑** `gh run rerun <run-id>`（不带 `--failed`）重建全部产物——不需要重新推标签。

**并发排队语义**：并发组默认「同一组同时最多一个运行中 + 一个排队」（官方语法 `concurrency.queue` 默认 `single`；GitHub 提供该配置项，本仓库 integration.yml 的夜间运行即显式用 `queue: max` 让定时运行排队而非被替换）；新排队项进入时会**替换掉较旧的排队项**——旧的被取消、不会报失败。也就是说同一标签若在短时间内触发多个发布运行，只有最新的会实际执行。正常流程不受影响：标签受 Ruleset 保护，重跑用 `gh run rerun` 不产生新运行；但**不要为同一标签重复推送触发多次运行**，如需重新发布请用 rerun。

## 五、草稿人工验收清单

- [ ] 在未安装 Hinina 的干净 Windows 环境安装并启动。
- [ ] 应用内版本、安装包版本与关于信息一致。
- [ ] 直接启动不出现控制台黑窗；从终端带 `--console` 启动出现调试控制台（崩溃信息写入 `%TEMP%\hinina-panic.log`，不经 `RUST_LOG` 过滤、不依赖日志文件是否可用，仅临时目录不可写时例外；日志文件可写且未被 `RUST_LOG` 关闭时同时写入 `logs/hinina.log`）。
- [ ] 登录夹具、题目、编辑、保存、关窗 smoke 通过。
- [ ] 覆盖旧版本升级安装（自第二个版本起适用）。
- [ ] 卸载后用户数据行为符合发布说明。
- [ ] `SHA256SUMS.txt` 与附件实际校验值一致。
- [ ] `CHANGELOG.md` 版本内容、本手册实际步骤、`SECURITY.md` 报告渠道均可用。
- [ ] 发布说明包含：变更摘要、已知问题、支持系统/架构、安装与卸载说明、数据保留说明。

### 版本分级验收

- **0.x（开发预览）**：以 CI 验证为准——版本门禁、复用全量 CI、产物命名与 PE 校验、SHA256 生成、草稿创建全部自动完成；本机复核（重算校验和、干净环境安装）**推迟到 1.0.0**，维护者可自行决定是否提前做。记录：`v0.1.1` 于 2026-10-27 补做过一次附件校验（下载三个附件、本地重算 SHA-256 与 `SHA256SUMS.txt` 比对），安装验收仍未做。
- **1.0.0 起（正式稳定版）**：发布前必须完成本机核验——下载 Release 附件重算 SHA256 与 `SHA256SUMS.txt` 比对，并在未安装过 Hinina 的干净 Windows 环境完成上表安装验收。此后每个正式版的验收要求不回退。

## 六、正式发布

验收通过后手动公开：

```bash
gh release edit vX.Y.Z --draft=false
```

正式发布后产物视为不可变；缺陷修复通过新版本交付，不静默替换同名安装包。

**发布说明的来源**：草稿说明由 tag 所指提交里的 `CHANGELOG.md` 段落生成，tag 之后再修改 CHANGELOG **不会**自动更新草稿。若草稿阶段发现说明有误（例如引用了已清理的标签），用 `gh release edit vX.Y.Z --notes-file <文件>` 修正——**草稿可改，公开后不可改**。

## 七、回退

- 草稿验收失败：保持草稿不公开，修复后经新提交、新标签重新发版。
- 已发布版本发现严重问题：撤回下载或标注风险，发布说明记录影响范围，尽快提供修复版本。
