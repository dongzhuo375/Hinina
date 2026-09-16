# system（客户端存储信息类型）

> 源文件：`src/types/system.ts`

## 职责

设置页「关于」展示的客户端存储信息跨端契约，对应 Rust `commands` 层 `StorageInfo`。

## 核心类型/函数

| 名称 | 结构 | 说明 |
|------|------|------|
| `StorageInfo` | `{ baseDir, logPath, version }` | 本地存储根目录 / 日志文件完整路径 / 客户端版本号（构建期注入） |

## 直接依赖

无（纯类型模块）。

## 被依赖

- `bridge/system.bridge.ts`、`services/system.service.ts`、`views/SettingsView.vue`
