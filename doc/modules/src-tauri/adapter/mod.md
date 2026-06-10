# mod

## 职责
adapter 模块的根文件，声明并公开 HOJ、QDUOJ、HUSTOJ 三个 OJ 适配器子模块。每个适配器各自组合实现 `AuthProvider`、`ContestProvider`、`ProblemProvider`、`SubmissionProvider` 等拆分 trait。

## 核心类型/函数
- `pub mod hoj` — 声明 HOJ 适配器子模块
- `pub mod qduoj` — 声明 QDUOJ 适配器子模块
- `pub mod hustoj` — 声明 HUSTOJ 适配器子模块

## 直接依赖
无（仅声明子模块）

## 被依赖
- `lib.rs`（`pub mod adapter`）

## 逻辑流程
无（仅模块声明）
