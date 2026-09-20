# user（用户与会话校验契约类型）

> 源文件：`src/types/user.ts`

## 职责

用户实体与会话校验三态的跨端契约类型，与 Rust `core::entity::user::User` / `service::auth::SessionValidity` 对齐。

## 核心类型/函数

| 名称 | 形状 | 关键语义 |
|------|------|----------|
| `User` | `{ id: string; username: string; token: string }` | 对应 Rust User；`token` 为 HOJ JWT（前端仅透传/缓存，认证头由 Rust Provider 携带） |
| `SessionValidity` | `'valid' \| 'invalid' \| 'unknown'` | 对应 Rust 侧 serde snake_case 序列化。三态语义：**`valid`** 服务端确认有效；**`invalid`** 本地无会话或服务端已判定失效（磁盘会话已被清除），须重新登录；**`unknown`** 网络异常等无法判定——**本地会话保留，应稍后重试而非踢出用户**（赛前误踢回登录页的代价远大于多等一轮校验，且反复重登可能触发 HOJ 暴力破解锁定） |

## 直接依赖

无（纯类型声明文件）

## 被依赖

- `bridge/auth.bridge.ts`、`services/auth.service.ts`、`stores/authStore.ts`、`views/LoginView.vue`（间接经 store）——均仅类型引用

## 逻辑流程

无（纯类型定义）。

设计要点：

- 三态而非布尔是刻意设计：把「服务端明确失效」与「无法判定」分开，是赛前会话预检
  （`utils/session-check`）与全局 401 兜底（`guards/sessionGuard`）不误伤选手的前提；
  Rust 侧动机详见 `doc/modules/src-tauri/service/auth/mod.md`。
- `unknown` 的消费契约：authStore.validateSession 保持登录态不变，LoginView 预检只重试
  一次（2–5s），避免重试风暴。
