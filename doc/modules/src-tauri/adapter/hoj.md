

## 契约测试（M2）

`tests/contract_tests.rs` 经 `test_support::loopback` 打真实网络栈：登录凭证与响应头 token 提取、裸 `Authorization`（无 Bearer）、`Refresh-Token` 轮换（新 token 落盘 + `TokenRotated` 事件 + 后续请求换用）、提交体以展示题号为 pid、服务端 500 不重试且带摘录。
