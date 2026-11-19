# adapter::urlenc（URL 插值段编码）

> 源文件：`src-tauri/src/adapter/urlenc.rs`

## 职责

URL 插值段的百分号编码，HOJ 与 Hydro 共用。放在 adapter 层与 `adapter::time` 同理：
「字符串标识拼进路径/查询」是两条线协议的公共需求，各写一份必然漂移。直接
`format!` 时含 `&`/`#` 的值会注入或截断 query，含空格的 URL 会被 reqwest 拒绝
解析；正常标识（hex/数字/字母）全部落在 unreserved 集合内，编码恒等，
**不改变现有线上形态**。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `percent_encode_component` | `(value: &str) -> String`（`pub(crate)`） | RFC 3986 unreserved（`A-Za-z0-9-._~`）之外的字节全部转义为 `%XX`（UTF-8 逐字节）；路径段与查询值拼 URL 前一律经它，纯数字位（页码/状态码）不经过也无妨 |

## 直接依赖

- 无（纯 std 字符串处理）

## 被依赖

- `adapter/hydro/mod.rs`（tid/rid/uid/pid 等全部字符串插值位）
- `adapter/hoj/mod.rs`（`get_problem` 的 `displayId`、`list_contest_submissions` 的 `problemID`）

## 测试

`src-tauri/src/adapter/tests/urlenc_tests.rs` 锁定：unreserved 标识（hex/数字/字母/`-._~`）恒等通过（线上形态不变的保证）、保留字与多字节转义（空格→`%20`、`&`→`%26`、`#`→`%23`、`%`→`%25`、中文按 UTF-8 字节逐个转义）。两适配器各自的**线上请求行断言**（编码真的到达 wire）分别在 `adapter/hoj/tests/mod_tests.rs` 与 `adapter/hydro/tests/mod_tests.rs`。
