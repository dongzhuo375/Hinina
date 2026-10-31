# DataDirSection（数据目录分组）

> 源文件：`src/components/settings/DataDirSection.vue`

## 职责

展示目录来源、候选路径、迁移勾选与重启提示。组件只调用 `useDataDirSettings` 模型，不直接打开选择器或写目录指针。

**维护者层整节锁**：`locked` 时禁用全部操作按钮（更改目录/确认更改/取消/恢复默认）与迁移勾选，并在区块头部显示锁提示；提权唯一来源是 `--settings` 启动参数。数据目录属连接与存储基础设施，选手层不需要也不应该改。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `model` | prop `ReturnType<typeof useDataDirSettings>` | 目录信息、候选、迁移勾选、执行态与通知 |
| `locked` | prop `boolean`（默认 false） | 维护者层锁定：按钮与迁移勾选 `:disabled` + 头部锁提示 |

模型包含 `dataDir/dataDirFailed/dataDirSourceMeta`、`pendingDir/migrateData/changingDir`、`changeNotice/changeError`、`loadDataDir/cancelDataDirChoice/chooseDataDir/confirmDataDir/restoreDefaultDataDir`。

## 直接依赖

- `SettingsSectionFrame`
- `@/composables/useDataDirSettings`（模型类型）

## 被依赖

- `views/SettingsView.vue`

## 逻辑流程

选择器返回候选后由用户确认；取消候选复位 migrate=true。确认和恢复默认都由 controller 先取 migrate 快照再写指针、重读并提示重启。`fallbackTemp` 与 `restartRequired` 仍按原样式展示。

## 测试

`useDataDirSettings.spec.ts` 覆盖 migrate 快照与取消回归；`SettingsView.behavior.spec.ts` 覆盖真实选择/确认接线（`--settings` 模式运行——正常模式按钮锁定，另有禁用态护栏回归：locked 时点击更改目录不触达选择器）。
