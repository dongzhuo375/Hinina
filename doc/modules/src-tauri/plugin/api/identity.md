# identity

`identity.current(context)` 需 identity.read；未登录返回 null。返回 `{ojId, userKey}`，userKey 是实际 OJ endpoint、OJ ID、稳定用户 ID 元组的 SHA-256，不返回用户名、Session 或 Token。不使用用户名回退生成稳定身份，缺少稳定 ID 时视为无可用身份。

身份读取只走 AuthService.identity_metadata，不触发 get_session 的凭证恢复副作用。它标识本地已保存身份，不能代替远端会话有效性校验；后续远端查询仍可能失败。
