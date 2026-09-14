# env（全局环境类型声明）

> 源文件：`src/env.d.ts`

## 职责

TypeScript 全局环境声明：引入 Vite 客户端类型、声明构建期注入的全局常量与 `.vue` 单文件组件的模块形状。

## 核心类型/函数

| 声明 | 用途 |
|------|------|
| `/// <reference types="vite/client" />` | Vite 客户端类型（import.meta.env、静态资源导入等） |
| `declare const __APP_VERSION__: string` | 客户端版本号，构建期由 `vite.config.ts` 的 `define` 注入（来源 package.json version）；消费方：`App` 之外的 `LoginView` 与 `StatusBar`（两处同源，避免写死后漂移） |
| `declare module "*.vue"` | `.vue` SFC 的模块声明（`DefineComponent` 默认导出），使 TS 能导入单文件组件 |

## 直接依赖

- `vite/client`（类型引用）
- `vue`（仅 `DefineComponent` 类型）

## 被依赖

- 全项目 TS 编译（ambient 声明，无显式 import）；`__APP_VERSION__` 由 `views/LoginView.vue` 与 `components/layout/StatusBar.vue` 消费

## 逻辑流程

无（纯类型声明，无运行时代码）。
