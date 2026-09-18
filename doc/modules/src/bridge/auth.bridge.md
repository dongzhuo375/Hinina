# auth.bridge（认证 IPC 桥接）

> 源文件：`src/bridge/auth.bridge.ts`

## 职责

认证相关 Tauri IPC 的薄封装：login / logout / get_session / validate_session 四个 Command 的参数透传。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `login` | `(username, password) => Promise<User>` | invoke `login`（OJ 切换走显式 `switchOj`，见 `config.bridge`；**参数含明文密码**——HOJ 服务端自行 MD5 比对，客户端不哈希；日志安全由 `ipcInvoke` 的「不记 args」约束保障） |
| `logout` | `() => Promise<void>` | invoke `logout` |
| `getSession` | `() => Promise<User \| null>` | invoke `get_session`（null = 无本地会话） |
| `validateSession` | `() => Promise<SessionValidity>` | invoke `validate_session`，返回三态 `valid / invalid / unknown` |

## 直接依赖

- `@/bridge`（`ipcInvoke`）
- `@/types/user`（仅类型）

## 被依赖

- `services/auth.service.ts` — 唯一调用方

## 逻辑流程

```
auth.service → 各桥接函数 → ipcInvoke(cmd, args) → Rust commands::auth_cmd → AuthService
```

设计要点：

- `SessionValidity` 是跨端契约（Rust 侧 snake_case 序列化，见 `types/user.md`）；
  桥接层不解释三态语义，分流在 service/store 完成。
