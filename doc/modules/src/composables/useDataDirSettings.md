# useDataDirSettings（数据目录控制器）

> 源文件：`src/composables/useDataDirSettings.ts`

## 职责

持有数据目录信息、候选路径、迁移勾选、执行状态与通知。只校验并记录目录指针；真正搬迁在下次启动完成，不在当前进程热切目录。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `dataDir` / `dataDirFailed` / `loadDataDir` | refs / fn | 当前目录、默认目录、来源、restartRequired 与局部失败 |
| `pendingDir` / `migrateData` | refs | 原生选择器返回的候选与迁移勾选（默认 true） |
| `chooseDataDir` / `cancelDataDirChoice` | fn | 选择/取消候选；取消时复位 migrate=true |
| `confirmDataDir` / `restoreDefaultDataDir` | fn | 先取 migrate 快照，再写指针、复位、重读并提示重启生效 |
| `dataDirSourceMeta` | computed | 默认/自定义/临时目录的文案与配色 |
| `changingDir` / `changeNotice` / `changeError` | refs | 执行态与 5 秒反馈 |

## 直接依赖

- `vue`（`ref` / `computed`）
- `@/composables/useTransientValue`（5s 目录提示）
- `@/services/system.service`
- `@/utils/error`
- `@/types/system`

## 被依赖

- `views/SettingsView.vue`
- `components/settings/DataDirSection.vue`
- `composables/__tests__/useDataDirSettings.spec.ts`

## 逻辑流程

选择器只返回候选，确认才写指针。确认和恢复默认都先读取当时的 migrate 值，再复位共享勾选，避免通知与实参不一致。成功后重新读取目录信息并显示 5 秒提示；失败只显示后端错误。5 秒目录提示复用 `useTransientValue`，由 scope dispose 清理。

## 测试

`src/composables/__tests__/useDataDirSettings.spec.ts` 覆盖 migrate=false 的确认快照、成功后复位/重读，以及取消后恢复默认仍按 true 调用。
