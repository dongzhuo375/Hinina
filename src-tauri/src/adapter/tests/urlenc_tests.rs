// adapter::urlenc 单元测试：URL 插值段编码的恒等与转义边界。

use super::*;

/// unreserved 集合内的正常标识（hex/数字/字母/`-._~`）必须恒等通过：
/// 编码不得改变现有线上形态。
#[test]
fn percent_encode_passes_unreserved_ids_through_unchanged() {
    for id in ["P1000", "659b1f0a4c7d2e3f8a9b0c1d", "7", "A", "A-1_b.c~d"] {
        assert_eq!(percent_encode_component(id), id, "正常标识应恒等: {}", id);
    }
}

/// 保留字与多字节字符必须转义：`&`/`#` 会注入或截断 query，空格让 reqwest
/// 拒绝解析 URL，`%` 防二次解码歧义，中文按 UTF-8 字节逐个转义。
#[test]
fn percent_encode_escapes_reserved_and_multibyte_bytes() {
    assert_eq!(percent_encode_component("a b"), "a%20b");
    assert_eq!(percent_encode_component("a&b"), "a%26b");
    assert_eq!(percent_encode_component("A#B"), "A%23B");
    assert_eq!(percent_encode_component("100%"), "100%25");
    assert_eq!(percent_encode_component("题"), "%E9%A2%98");
}
