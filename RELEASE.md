# 发布手册（RELEASE）

Hinina 的 Windows x64 NSIS 发布流程（M4 范围）。OS 代码签名接入为 M5 范围、自动更新为 M6 范围，本手册不涉及；两项接入后在本手册追加对应步骤。

## 一、版本准备

1. 确认五处版本一致：`npm run release:check`（package.json、package-lock.json、src-tauri/Cargo.toml、src-tauri/Cargo.lock、src-tauri/tauri.conf.json）。
2. 在 `CHANGELOG.md` 将 `## [Unreleased]` 内容整理进新版本段落，并填入发布日期。
3. 版本推进经普通 PR 合入 main，等待 ci-required 绿；打标签前保持工作树干净。

## 二、打标签

```bash
git tag vX.Y.Z          # 必须指向 main 历史上的提交，且与应用版本一致
git push origin vX.Y.Z
```

推送标签后 Release 工作流自动运行；标签受 Ruleset 保护，不可随意创建/移动。

## 三、工作流阶段（.github/workflows/release.yml）

| Job | 说明 |
| --- | --- |
| validate | 校验 tag 格式、tag 指向 main 历史、五处版本一致、CHANGELOG 段落存在 |
| test | 通过 workflow_call 复用完整必需 CI（ci.yml），发布不跳过任何检查 |
| build-windows | Windows x64 NSIS 构建，产出安装包与 build-metadata.json（tag/commit/工具链版本） |
| verify-artifact | 校验安装包文件名（`Hinina_<版本>_x64-setup.exe`）与 PE 文件类型，生成 SHA256SUMS.txt |
| draft-release | 在受保护 environment（人工批准）中创建或更新草稿 Release |

## 四、重跑

- 个别 job 失败：`gh run rerun --failed <run-id>`。
- 同一标签重跑会复用并更新**同一个草稿**，不创建重复 Release。
- 已正式发布的 Release 不可变：重跑遇到已发布的同名 Release 会显式失败，修复应走新版本。

**并发排队语义**：release.yml 的并发组是默认的 `queue: single`——组内已有排队项时，新进入的排队项会**取消并替换**较早的排队项。也就是说同一标签若在短时间内产生多个发布运行，只有最新的会实际执行，更早的排队运行被静默取消（不会报失败）。正常流程不受影响：标签受 Ruleset 保护，重跑用 `gh run rerun` 不产生新运行；但**不要为同一标签重复推送触发多次运行**，如需重新发布请用 rerun。

## 五、草稿人工验收清单

- [ ] 在未安装 Hinina 的干净 Windows 环境安装并启动。
- [ ] 应用内版本、安装包版本与关于信息一致。
- [ ] 登录夹具、题目、编辑、保存、关窗 smoke 通过。
- [ ] 覆盖旧版本升级安装（自第二个版本起适用）。
- [ ] 卸载后用户数据行为符合发布说明。
- [ ] `SHA256SUMS.txt` 与附件实际校验值一致。
- [ ] `CHANGELOG.md` 版本内容、本手册实际步骤、`SECURITY.md` 报告渠道均可用。
- [ ] 发布说明包含：变更摘要、已知问题、支持系统/架构、安装与卸载说明、数据保留说明。

## 六、正式发布

验收通过后手动公开：

```bash
gh release edit vX.Y.Z --draft=false
```

正式发布后产物视为不可变；缺陷修复通过新版本交付，不静默替换同名安装包。

## 七、回退

- 草稿验收失败：保持草稿不公开，修复后经新提交、新标签重新发版。
- 已发布版本发现严重问题：撤回下载或标注风险，发布说明记录影响范围，尽快提供修复版本。
