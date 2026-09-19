# oj（OJ 类型域）

> 源文件：`src/utils/oj.ts`

## 职责

OJ 类型域的**唯一权威模块**：维护「已知 OJ 类型」枚举清单，并为设置页组装 OJ 下拉候选。

值域约定（跨端契约）：OJ 身份是**字符串 id**，与 Rust 侧三处同源 ——
`core::provider::oj_id::OjId`、适配器工厂的 `id()`、配置里的 `oj.instances[].id`
（`OjId` 同时决定会话文件名 `sessions/{id}.json`）。

设计取舍：**固定枚举而非通用实例管理**。用户可从清单里挑一个尚未配置的 OJ（如 Hydro），
填地址保存即创建实例 —— 解决「接入新 OJ 必须手改 config.json」；但不做实例增删 UI：
本项目面向校内赛的少数几个已知 OJ，固定枚举更简单，也避免用户在 UI 里自由拼 id
（id 会拼进会话文件名，拼错即会话错位）。枚举外的 id（手改配置的私有部署）仍会被
如实显示，不丢信息、不悄悄改写用户配置。

## 核心类型/函数

- `OJ_TYPES: readonly string[]` — 已知 OJ 类型（顺序即下拉展示顺序）。**新增 OJ 时与后端
  `adapter::factories()` 同步加一项**（当前两处：`HOJ` / `Hydro`）
- `ojBaseUrlHint(ojId) -> string` — 地址输入框的占位提示（`HOJ` → `https://hoj.example.com`、
  `Hydro` → `https://hydro.ac`）；未知类型回退通用示例。仅用于 placeholder，不写进配置
- `OjOption` — 下拉候选项：`{ id, configured, disabled }`
  - `configured: false` = 配置里尚无该实例（需先填地址保存才能切换）
  - `disabled: true` = 已有实例但被禁用（禁用实例不会被注册，切过去只会得到 `ProviderNotFound`）
- `ojSelectOptions(instances) -> OjOption[]` — 候选组装：**已知枚举在前**（含尚未配置者）、
  配置里出现的其它 id 追加在后；同 id 不重复。纯函数，便于单测锁定顺序与取值域契约

## 直接依赖

无（纯类型与纯函数；不依赖 bridge / store，符合「utils 只放纯逻辑」的约定）

## 被依赖

- `views/SettingsView.vue` — OJ 分组的下拉候选（`ojSelectOptions`）与地址占位提示（`ojBaseUrlHint`）

## 逻辑流程

```
SettingsView 读配置 → ojSelectOptions(config.oj.instances)
  → 已知枚举（HOJ / Hydro）逐个判定 configured/disabled
  → 追加枚举外的实例 id（手改配置的私有部署）
  → 用户选中未配置类型 → 引导填地址 → 保存建实例 → 后端按需注册 → 自动切换
```

## 测试

`src/utils/__tests__/oj.spec.ts` 锁定：枚举里的类型**全部出现**（未配置者也列出 ——
那正是「启用新 OJ」的入口）、未配置者标 `configured: false`、禁用者标 `disabled: true`
但仍显示（配置不得在 UI 里凭空消失）、枚举外 id 追加在后且不重复、空实例清单时
仍列出全部已知类型、`ojBaseUrlHint` 的已知/未知回退。
