# 有界JSON编码

plugin/bounded_json.rs 使用实现std::io::Write的计数器，serde_json写入每个片段前检查累计UTF-8字节；包含字符串转义和JSON语法开销。

encoded_len只计数、不保留输出；to_string仅在预算内积累字节，最后构造UTF-8文本。长度等于上限合法，超过上限返回limit_exceeded，其他编码错误为unavailable。ExecutionApi先检查DTO再构造Value，最终包络再次有界编码；查询用它限制native投影页及全部保留游标余项。

测试覆盖精确边界、中文/转义字节、大集合流式拒绝和桥包络导致的超限。
