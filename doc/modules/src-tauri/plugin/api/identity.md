# api/identity

## 职责
本人身份投影：返回稳定用户键，供插件把榜单 `teamKey` 与自身比对。

## 核心类型/函数
- `PluginIdentity` — `{ojId, userKey}`
- `IdentityApi`（trait）— `current(context) -> Option<PluginIdentity>`

## 直接依赖
- `plugin::api`（PluginQueryContext / PluginResult）
- `async_trait`、`serde`

## 被依赖
- `plugin::execution_api`（能力转发）、`plugin::query`（取数；`user_key` 同算法）

## 逻辑流程
- 需要 `identity.read`；未登录返回 null
- `userKey` 是（实际 OJ endpoint, OJ ID, 稳定用户 ID）元组的 SHA-256——不返回用户名、Session 或 Token；不使用用户名回退生成稳定身份，缺少稳定 ID 时视为无可用身份
- 身份读取只走 `AuthService.identity_metadata`，**不触发 get_session 的凭证恢复副作用**；它标识本地已保存身份，不能代替远端会话有效性校验，后续远端查询仍可能失败
