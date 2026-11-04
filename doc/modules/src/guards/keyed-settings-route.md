# keyed-settings-route（钥匙设置页判据）

> 源文件：`src/guards/keyed-settings-route.ts`（2026-11-04，G2 复审后引入）

## 职责

**路由级唯一判据**：本次启动是 `--settings` 管理方钥匙 **且** 目标路由是设置页。
判断组合了两个来源——进程级钥匙状态（`systemService.isKeyed()`）与路由身份
（`routeName === 'Settings'`），属名副其实的**跨层装配**，故住在 `guards/`。

历史：`guards/keyed-settings-policy` 曾是它的纯转发前身（零逻辑、把 `isKeyedSettings`
原样转给 service），G2 并入 `systemService` 时把「路由名」也带进了服务层——服务层
不认识路由，属分层倒置（2026-11-04 复审）。现在服务层只暴露进程级的 `isKeyed()`，
路由知识回到本模块。

## 核心类型/函数

| 名称 | 签名 | 说明 |
|---|---|---|
| `isKeyedSettingsRoute` | `(routeName: string \| symbol \| null \| undefined) => boolean` | `systemService.isKeyed() && routeName === 'Settings'`；缺失/符号路由名一律 false |

## 直接依赖

- `@/services/system.service`（`systemService.isKeyed()` —— 进程级判据；不直接读 `launchFlags`）

## 被依赖

- `router/index.ts`（`beforeEach` 条件豁免 / `fromKeyedSettings` / `afterEach` 落地不变量）
- `guards/sessionGuard.ts`（钥匙设置页 401 旁路）
- `views/ContestLayout.vue`（最小壳与阶段闸门跳转豁免）
- `composables/useSettingsForm.ts`（切换后的「停留设置页」判据）

**services 不 import 本模块**（服务层只用 `systemService.isKeyed()`），保持 guards
不反向进入服务层。

## 逻辑流程

```
isKeyedSettingsRoute(routeName)
  → systemService.isKeyed()（启动标志快照）
  → && routeName === 'Settings'
```

## 测试

`src/guards/__tests__/keyed-settings-route.spec.ts`：钥匙+Settings 才为真、非钥匙
一律假、缺失/符号路由名一律假。变异验证：去掉 `isKeyed()` 条件 → 用例红。
