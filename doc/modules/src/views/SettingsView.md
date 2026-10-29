# SettingsView（设置页组合根）

> 源文件：`src/views/SettingsView.vue`

## 职责

设置页的页面壳与组合根：`PageHeading`（05 / PREFERENCES）、左侧分组索引、配置加载/错误门禁、七个 section 的固定顺序、底部保存/放弃栏，以及统一挂载编排。业务实现按领域拆到 controllers 与 `components/settings`，View 不再持有各领域实现细节。

入口仍默认隐藏：连点状态栏版本号 5 下才在导航末尾出现设置项（`utils/settings-access`）。

## 核心类型/函数

| 组成 | 职责 |
|------|------|
| `useSettingsForm` | 中央 reactive 草稿、baseline/dirty/errors/canSave、配置加载/保存/放弃、OJ 清单与显式/自动切换 |
| `updateForm(patch)` | 将 section 的 `SettingsFormPatch` 合并进同一 reactive 草稿，不替换对象身份、不创建父子双份表单 |
| `useAboutSettings` | About 存储信息与复制反馈；模型以 `about` 传给 AboutSection |
| `useMaintenanceSettings` | 重置、清理、占用预览与提示 timer；模型以 `maintenance` 传给 ResetPurgeSection |
| `useDataDirSettings` | 数据目录候选、迁移与通知；模型以 `dataDirSettings` 传给 DataDirSection |
| `useSettingsScrollSpy(loading)` | `activeSection/scrollerRef/updateActiveSection/scrollToSection/SECTION_LABELS`，驱动导航与滚动高亮 |
| `onMounted` | 仍按原顺序 `void load()` → `about.loadStorage()` → `maintenance.loadUsage()` → `dataDirSettings.loadDataDir()`，不把首次加载推迟到配置成功后的子组件 mounted |

七个 section 的顺序固定为：`settings-0` OJ 服务器、`settings-1` 编辑器、`settings-2` 布局、`settings-3` 主题、`settings-4` 重置与清理、`settings-5` 数据目录、`settings-6` 关于。每个真实 section 由 `SettingsSectionFrame` 输出，保留 `tabindex=-1` 和 scroll-margin 契约。

## 直接依赖

- `vue`（`onMounted`）
- 通用组件：`PageHeading` / `ErrorMessage` / `LoadingSpinner`
- `components/settings/`：OjServerSection、EditorSection、LayoutSection、ThemeSection、ResetPurgeSection、DataDirSection、AboutSection
- `composables/useSettingsForm`（控制器与 `SettingsFormPatch` 类型）
- `composables/useAboutSettings` / `useMaintenanceSettings` / `useDataDirSettings`
- `composables/useSettingsScrollSpy`

config/system service、相关 store 和 types 由对应 controllers 持有；View 不直接 import bridge。

## 被依赖

- `router/index.ts` — `Settings` 子路由（`/contest/settings`）。
- `components/settings/SettingsSectionFrame.vue` 与七个 section 由本 View 组合。

## 逻辑流程

1. 父级 setup 创建全部 controllers，再统一启动四路加载；配置加载仍控制整页 sections 的渲染，另外三路失败只局部降级。
2. OJ/Editor/Layout 发出草稿补丁，View 合并进中央表单；保存、校验和 OJ 切换由 `useSettingsForm` 处理。
3. 自治 section 只接收各自模型：维护、目录和 About 不读写中央 dirty/save。
4. 导航使用 `SECTION_LABELS` 同一顺序，scroll-spy 通过真实 `settings-N`、focus 和 scrollerRef 工作；加载结束后由 controller 做 nextTick 校正。
5. 底部保存栏继续读取 `dirty/isValid/canSave/saving/showSaved/saveError`，按钮调用 `save/discard`。

## 测试

- `__tests__/SettingsView.spec.ts`：维护区组合回归——双空拦截、日志未清 warning、重置补拉失败 warning。
- `__tests__/SettingsView.behavior.spec.ts`：四路加载顺序、七组真实 section id/导航顺序、保存与 active、未配置 OJ 自动切换、已配置切换失败回滚、目录 migrate 快照、About 复制、scroll-spy 锁与 atBottom。
- Controllers 各有独立 spec：`useSettingsForm`、`useAboutSettings`、`useMaintenanceSettings`、`useDataDirSettings`、`useSettingsScrollSpy`。
- `e2e/specs/settings.spec.ts`：真实 Chromium 的分区样式、响应式导航/滚动与 fake IPC 保存验收；原生目录选择器、真实剪贴板权限和 WebView2 桌面表现另属人工验收。
