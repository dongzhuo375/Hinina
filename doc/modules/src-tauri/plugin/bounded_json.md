# plugin/bounded_json

## 职责
流式有界 JSON 编码：在序列化过程中逐片段检查累计 UTF-8 字节，保证超限数据在产生完整字符串之前即被拒绝，而不是编码后再量长度。

## 核心类型/函数
- `CountingWriter`（内部）— 实现 `std::io::Write` 的计数器；serde_json 写入每个片段前检查累计字节，计数包含字符串转义和 JSON 语法开销
- `encoded_len(value, max) -> PluginResult<usize>` — 只计数、不保留输出
- `to_string(value, max) -> PluginResult<String>` — 仅在预算内积累字节，最后构造 UTF-8 文本

## 直接依赖
- `plugin::api::{PluginError, PluginResult}`
- `serde::Serialize`
- `std::io::{self, Write}`

## 被依赖
- `plugin::execution_api`（DTO 与最终桥包络的有界编码）
- `plugin::query`（native 投影页及全部保留游标余项的尺寸校验）

## 逻辑流程
- 长度**等于**上限合法，超过上限返回 `limit_exceeded`；其他编码错误为 `unavailable`
- 调用纪律：ExecutionApi 先检查 DTO 再构造 `Value`，最终包络再次有界编码；查询用它限制单页与聚合预算（见 [limits](limits.md) 的字节预算一节）
- 测试覆盖精确边界、中文/转义字节、大集合流式拒绝和桥包络导致的超限
