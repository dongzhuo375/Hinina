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
| `SNAPSHOT_KEEP_DAYS` | `30` | 留档保留窗口（天）：更早的留档视为「过期」，可由设置页清理 |
| `SnapshotUsage` | struct（`Default` / `PartialEq`） | 占用统计：`total_count` / `total_bytes` / `stale_count` / `stale_bytes` |
| `inspect_snapshots` | `(storage, keep_days) => SnapshotUsage` | 统计留档占用（含过期部分），供清理前的**预览** |
| `purge_stale_snapshots` | `(storage, keep_days) => (usize, u64)` | 删除过期留档，返回（删除条数, 释放字节数），并回收删空的 OJ 子目录 |
| `stale_cutoff` / `walk_snapshots` | 私有 | 过期判据的截止时刻 / 遍历两层目录收集（路径, 字节数, mtime） |

## 直接依赖

- `core::error::{AppError, AppResult}`
- `infra::storage::Storage`（`read_to_string` / `write_string` / `list` / `remove` / `base_dir`）
- `tracing`（`warn` / `debug` / `info`）

## 被依赖

- `service/submission/mod.rs` — `submit` 成功后落盘；`get_submission_detail` 在 `code` 为空时回落
- `commands::maintenance_cmd` — `local_data_usage` 调 `inspect_snapshots`、`purge_local_data` 调 `purge_stale_snapshots`
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

清理本地数据（commands::maintenance_cmd）
  inspect_snapshots(keep_days=30)  → 总条数/字节 + 过期条数/字节（只读，供预览）
  purge_stale_snapshots(keep_days=30)
       walk_snapshots 收集 → mtime < now-30d 的逐条 storage.remove
       删除后扫 submissions/ 直接子项，remove 成功的（= 已空）即为回收的空目录
```

## 设计要点

- **为什么需要**：详情页的代码来自 OJ，而 OJ 会在多种情形下不回吐代码 —— 比赛隐藏本人记录、题目 `codeShare=false`、赛后回收。更常见的是**提交本身失败**：那时根本没有 `submission_id`，服务端一行记录都没有，「我刚才交的是什么」彻底无从查起（实测 HOJ 提交失败返回 HTTP 500，服务端不留痕）。快照是**本地事实**，不依赖服务端是否愿意回吐。
- **带 OJ 维度**：`submit_id` 是各 OJ 自增的资源号，跨 OJ 必然重号（与 `SubmissionService::cache_key` 同款约定）。有测试锁定两个 OJ 的同号提交互不覆盖。
- **扩展名推导与前端同源**：`source_extension` 的后缀与前端 `utils/language.sourceFileNameOf` 一致（`main.cpp` → `cpp`、`Main.java` → `java`…），未知语言回退 `txt` —— **绝不猜 `.cpp`**：判题端按后缀判语言，猜错等于让人误以为交的是另一种语言。语言前缀归一覆盖部署变体（`"C++17 (GCC 13.2)"` / `"Python 3.10"` / `"PyPy3"` / `"Golang"`），C 用词边界判定（与前端 `/^c\b/` 对齐）。
- **读取时支持扫描回退**：提交时与查询时的语言写法可能不同（服务端归一、选手改语言），精确路径会落空而快照明明就在那儿，故按 `{submit_id}.*` 扫一遍该 OJ 的快照目录。
- **路径安全**：`oj_id` / `submit_id` 来自会话与服务端响应，必须拒绝路径分隔符与 `..`（与 `ContestService::read_state_path` 同款防线）。
- **best-effort**：落盘失败只记 `warn`。快照是便利特性，丢一份快照远比丢一次提交轻。
- **「过期」按 mtime 判定，不比对服务端列表**：后者要网络、要分页、还可能因赛制隐藏记录而误判；而留档价值本就随时间衰减。窗口 30 天（`SNAPSHOT_KEEP_DAYS`）是「基本不会误删还想看的东西」与「不让目录无限累积」之间的折中。
- **清理绝不越界**：`purge_stale_snapshots` 只删 `submissions/` 下、mtime 早于窗口的文件，**保留窗口内的一律不动**（这是本功能唯一会丢数据的地方，有专门用例锁定）；删空的 OJ 子目录一并回收（`Storage::remove` 对目录是非递归删除，非空必然失败，正好用来「只回收空目录」）。
- **统计只读**：`inspect_snapshots` 不改动任何磁盘状态 —— 界面必须能安全地反复预览。
- **删除失败逐条忽略并告警**：清理是尽力而为的维护动作，不该因为一个文件被占用而整体失败。

## 测试

`src-tauri/src/service/submission/tests/snapshot_tests.rs` 锁定：扩展名与前端 `sourceFileNameOf` 逐一对齐、部署变体归一、`C#`/`JavaScript` 不被前缀吞并、未知语言回退 `txt`、路径越权拒绝、落盘读取往返、语言变更后扫描回退、跨 OJ 不撞号、非法 `submit_id` 不 panic 且不写出任何文件。

**清理语义**（用 `File::set_times` 伪造 mtime，不引入 `filetime` 依赖）：`inspect_snapshots` 统计跨 OJ 的总量与过期条数、窗口放宽后同一批留档不再过期、统计不改磁盘状态；`purge_stale_snapshots` **保留窗口内留档、只删过期的**、释放字节数与预览一致、清空的 OJ 子目录被回收而仍有留档的保留；从未提交过（目录不存在）时统计全零、清理是 no-op 而非报错。
