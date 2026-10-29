# useAboutSettings（About 存储信息与复制）

> 源文件：`src/composables/useAboutSettings.ts`

## 职责

持有 About 分组的存储信息、局部读取失败状态，以及版本/目录/日志路径复制反馈。复制回弹直接复用共享 `useTransientValue`，不重新实现 timer。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `storage` / `storageFailed` | `ref<StorageInfo \| null>` / `ref<boolean>` | 存储信息与局部失败降级 |
| `loadStorage` | `() => Promise<void>` | 读取 `systemService.getStorageInfo()`；失败只影响 About |
| `copiedKey` | `Readonly<Ref<string \| null>>` | 当前复制项（version/baseDir/logPath 等），2 秒后回弹 |
| `copyText` | `(key, text) => Promise<void>` | 调用剪贴板，成功后 `showCopied(key)`；失败保持静默 |

## 直接依赖

- `vue`
- `@/services/system.service`
- `@/composables/useTransientValue`
- `@/types/system`

## 被依赖

- `views/SettingsView.vue`
- `components/settings/AboutSection.vue`
- `composables/__tests__/useAboutSettings.spec.ts`

## 逻辑流程

父级挂载时调用 `loadStorage()`，与配置、占用预览和目录读取并行启动。复制成功才显示对应 key 的“已复制”；连续复制由共享 composable 取消旧 timer，scope dispose 统一清理。

## 测试

`src/composables/__tests__/useAboutSettings.spec.ts` 覆盖存储读取/局部失败、复制 key、2 秒回弹和剪贴板失败静默。
