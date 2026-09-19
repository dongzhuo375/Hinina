# snapshot（提交源码快照）

> 源文件：`src-tauri/src/service/submission/snapshot.rs`

## 职责

把「我当时提交的那份代码」留在本地：提交成功后落盘源码，详情页在 OJ 未回吐代码时回落到它。路径为 `submissions/{oj_id}/{submit_id}.{ext}`。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `SNAPSHOT_DIR` | `"submissions"` | 快照根目录（相对存储根） |
| `FALLBACK_EXT` | `"txt"` | 未知语言的扩展名回退值 |
| `starts_with_word` | `(s, word) => bool` | 独立词前缀判定（规避 `"c#"` 被 `"c"`、`"javascript"` 被 `"java"` 吞并） |
| `language_family` | `(language) => Option<&'static str>` | HOJ 显示名 → 语言族（与前端 `monacoIdStrict` 同识别面） |
| `source_extension` | `(language) => &'static str` | 语言 → 源文件扩展名（与前端 `sourceFileNameOf` 的后缀一致） |
| `snapshot_path` | `(oj_id, submit_id, language) => AppResult<String>` | 精确路径（含路径字符校验） |
| `write_snapshot` | `(storage, oj_id, submit_id, language, source_code)` | best-effort 落盘：失败只 `warn`，不向上传播 |
| `read_snapshot` | `(storage, oj_id, submit_id, language) => Option<String>` | 读取（精确路径未命中时按 `{submit_id}.*` 扫描回退） |

## 直接依赖

- `core::error::{AppError, AppResult}`
- `infra::storage::Storage`（`read_to_string` / `write_string` / `list`）

## 被依赖

- `service/submission/mod.rs` — `submit` 成功后落盘；`get_submission_detail` 在 `code` 为空时回落
- `service/submission/tests/snapshot_tests.rs` — 单元测试

## 逻辑流程

```
submit 成功
  └ write_snapshot(storage, oj_id, submit_id, language, source_code)
       snapshot_path(...) ──校验 oj_id / submit_id（拒绝 / \ : ..）──▶ submissions/{oj}/{id}.{ext}
       storage.write_string(...)      // 自带父目录创建
       失败 → warn!("提交源码快照保存失败（不影响提交本身）")

get_submission_detail
  └ provider 返回 detail
       detail.code 为空？
         └ read_snapshot(...)   // 精确路径 → 未命中则扫 submissions/{oj}/ 找 {submit_id}.*
              命中 → detail.code = 快照内容
```

## 设计要点

- **为什么需要**：详情页的代码来自 OJ，而 OJ 会在多种情形下不回吐代码 —— 比赛隐藏本人记录、题目 `codeShare=false`、赛后回收。更常见的是**提交本身失败**：那时根本没有 `submission_id`，服务端一行记录都没有，「我刚才交的是什么」彻底无从查起（实测 HOJ 提交失败返回 HTTP 500，服务端不留痕）。快照是**本地事实**，不依赖服务端是否愿意回吐。
- **带 OJ 维度**：`submit_id` 是各 OJ 自增的资源号，跨 OJ 必然重号（与 `SubmissionService::cache_key` 同款约定）。有测试锁定两个 OJ 的同号提交互不覆盖。
- **扩展名推导与前端同源**：`source_extension` 的后缀与前端 `utils/language.sourceFileNameOf` 一致（`main.cpp` → `cpp`、`Main.java` → `java`…），未知语言回退 `txt` —— **绝不猜 `.cpp`**：判题端按后缀判语言，猜错等于让人误以为交的是另一种语言。语言前缀归一覆盖部署变体（`"C++17 (GCC 13.2)"` / `"Python 3.10"` / `"PyPy3"` / `"Golang"`），C 用词边界判定（与前端 `/^c\b/` 对齐）。
- **读取时支持扫描回退**：提交时与查询时的语言写法可能不同（服务端归一、选手改语言），精确路径会落空而快照明明就在那儿，故按 `{submit_id}.*` 扫一遍该 OJ 的快照目录。
- **路径安全**：`oj_id` / `submit_id` 来自会话与服务端响应，必须拒绝路径分隔符与 `..`（与 `ContestService::read_state_path` 同款防线）。
- **best-effort**：落盘失败只记 `warn`。快照是便利特性，丢一份快照远比丢一次提交轻。

## 测试

`src-tauri/src/service/submission/tests/snapshot_tests.rs` 锁定：扩展名与前端 `sourceFileNameOf` 逐一对齐、部署变体归一、`C#`/`JavaScript` 不被前缀吞并、未知语言回退 `txt`、路径越权拒绝、落盘读取往返、语言变更后扫描回退、跨 OJ 不撞号、非法 `submit_id` 不 panic 且不写出任何文件。
