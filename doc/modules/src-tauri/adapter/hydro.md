

## 契约测试（M2）

`tests/contract_tests.rs`：登录 `Set-Cookie: sid` 提取（Accept: application/json 序列化重定向、不跟随 302）、`rememberme` 开启、探测失败身份降级不否定会话、后续请求 Cookie sid + `Bearer` 双形态携带、缺 sid 判 Auth。

### 降级身份与已验证用户缓存（M2 评审 P2）

登录时上下文探测失败/匿名产生的**降级身份（空 id 占位用户）只用于登录返回值展示，绝不写入 `self.user` 已验证用户缓存** —— 缓存命中即短路 `probe_user_context`，占位用户入缓存会让 `validate_session` 无条件判 `Ok(true)`（明确匿名的会话判有效、本应 Unknown 的网络失败也判有效）。服务端已验证的身份仅由 probe 成功路径缓存。契约测试 `validate_session_does_not_trust_degraded_cached_identity` 锁定：降级登录后 validate 必须真实回访（探测失败判 Err、服务端恢复后判 Ok(true)）。
