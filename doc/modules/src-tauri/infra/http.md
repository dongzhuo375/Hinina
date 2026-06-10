# http

## 职责
HTTP 客户端封装（基于 Reqwest），提供统一的超时、重试、User-Agent 管理能力。当前为占位 stub，待实现。

## 核心类型/函数
- **`HttpClient`** — HTTP 客户端 struct，计划封装 `reqwest::Client`

## 直接依赖
无（当前为纯 stub，未引入外部 crate 或项目内模块）

## 被依赖
- `core::context`（`AppContext` 持有 `Arc<HttpClient>` 注入到各 Service）

## 逻辑流程
无（TODO 占位，待实现 reqwest 封装逻辑）
