# useAsyncResource（一次性加载 + failed 标志）

> 源文件：`src/composables/useAsyncResource.ts`

## 职责

设置页各区块辅助信息的「一次性加载」壳：成功写值并复位 failed，失败置 failed 且不动已有数据。三份同形加载（About 存储信息 / 数据目录 / 本地占用）的机制单点；失败只影响本区块、不参与配置表单的 dirty/save。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `useAsyncResource` | `<T>(loader: () => Promise<T>) => { data, failed, load }` | 泛型资源壳 |
| `data` | `Ref<T \| null>` | 加载结果 |
| `failed` | `Ref<boolean>` | 局部失败标志（成功复位） |
| `load` | `() => Promise<void>` | 执行加载；失败静默置 failed |

## 直接依赖

- `vue`（`ref`）

## 被依赖

- `composables/useAboutSettings.ts`
- `composables/useDataDirSettings.ts`（目录更改/恢复默认后重载）
- `composables/useMaintenanceSettings.ts`（清理后重载占用）

## 逻辑流程

加载失败的具体原因不落 UI：调用方在失败态只展示「获取失败」回退行，排障走前端日志转发。

## 测试

经三个消费方 composable 的 spec 覆盖（读取成功/失败分支与重载路径）。
