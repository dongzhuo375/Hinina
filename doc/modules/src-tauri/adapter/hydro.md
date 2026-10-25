

## 契约测试（M2）

`tests/contract_tests.rs`：登录 `Set-Cookie: sid` 提取（Accept: application/json 序列化重定向、不跟随 302）、`rememberme` 开启、探测失败身份降级不否定会话、后续请求 Cookie sid + `Bearer` 双形态携带、缺 sid 判 Auth。
