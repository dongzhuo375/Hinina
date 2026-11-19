// URL 插值段百分号编码（各 Adapter 共用）。
//
// **为什么单独成模块**：与 `adapter::time` 同理——HOJ 与 Hydro 都要把字符串
// 标识（cid 之外的 tid/rid/uid/pid/displayId 等）拼进路径或查询串，各写一份
// 必然漂移。直接 `format!` 时含 `&`/`#` 的值会注入或截断 query，含空格的
// URL 会被 reqwest 拒绝解析；正常标识（hex/数字/字母）全部落在 unreserved
// 集合内，编码恒等，不改变现有线上形态。

/// URL 插值段的百分号编码（RFC 3986 unreserved 之外的字节全部转义）。
///
/// 路径段（如 `/contest/:tid`、`/p/:pid`）与查询值（如 `uidOrName=`、
/// `problemID=`）拼 URL 前一律经它；纯数字位（页码/状态码）不经过也无妨。
pub(crate) fn percent_encode_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{:02X}", byte));
        }
    }
    out
}

#[cfg(test)]
#[path = "tests/urlenc_tests.rs"]
mod tests;
