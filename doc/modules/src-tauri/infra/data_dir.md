# data_dir（数据目录解析、位置指针与一次性迁移）

> 源文件：`src-tauri/src/infra/data_dir.rs`

## 职责

决定客户端数据根目录（base_dir）落在哪里，并负责**一次性搬家**。

**为什么需要它**：base_dir 此前硬编码为 `%TEMP%/hinina`，而临时目录会被 Windows 磁盘清理、第三方清理工具或系统策略**随时清空** —— 那意味着选手的工作区代码与提交留档无声消失且不可恢复。现在默认落在 `app_local_data_dir()`（`%LOCALAPPDATA%/{identifier}`，**不随域漫游**），并允许用户在设置页指定别处。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `POINTER_FILE` | `"data_dir.json"` | 位置指针文件名，**固定放在默认目录下** |
| `LEGACY_DIR_NAME` | `"hinina"` | 临时目录下的旧数据目录名 |
| `MIGRATED_ENTRIES` | `&[&str]` | 迁移清单：`config.json` / `sessions` / `workspaces` / `submissions` / `announcements_read` / `cache`（**不含 `logs`**） |
| `PROBE_FILE` | `".hinina-write-probe"` | 写盘可用性探针（写完即删） |
| `DataDirSource` | enum：`Default` / `Custom` / `FallbackTemp` | 数据目录来源，`Serialize` 为 `default` / `custom` / `fallbackTemp` |
| `DataDirPlan` | `{ base_dir, default_dir, source }` | 启动时解析出的方案 |
| `DataDirPointer` | `{ data_dir, migrate_from, legacy_migrated }` | 指针文件结构（字段全部 `skip_serializing_if`，未设置时不落盘） |
| `MigrateOutcome` | `{ moved, skipped, failed, legacy_removed }` | 迁移结果；`is_noop()` / `is_ok()` |
| `legacy_dir()` | `() -> PathBuf` | `%TEMP%/hinina` |
| `pointer_path(default_dir)` | `(&Path) -> PathBuf` | 指针文件路径 |
| `read_pointer` / `write_pointer` | — | 读写指针；**损坏或缺失一律按「未指定」处理**（绝不阻断启动） |
| `dir_is_usable` | `(&Path) -> bool` | 能创建 + 能写入（探针文件写后即删） |
| `dir_is_empty` | `(&Path) -> bool` | 不存在或为空（读不了时按「非空」保守处理）。**注意：不可用于默认目录的校验**，见 `conflicting_entries_in` |
| `conflicting_entries` | `() -> impl Iterator<Item = &'static str>` | [`MIGRATED_ENTRIES`] 去掉 `cache`（派生，避免两处漂移） |
| `conflicting_entries_in` | `(&Path) -> Vec<&'static str>` | `dir` 中已存在的**冲突条目**（迁移会跳过 → 陈旧数据被静默采用）。「恢复默认 + 迁移」的校验判据 |
| `validate_target` | `(&Path, legacy) -> AppResult<PathBuf>` | 设置页「更改目录」的校验（见下） |
| `resolve` | `(default_dir, legacy) -> DataDirPlan` | 解析优先级：指针指定（可用）→ 默认（可用）→ 临时目录（回退 + 告警） |
| `prepare_startup` | `(default_dir, legacy) -> (DataDirPlan, MigrateOutcome)` | 启动用：解析 + 一次性迁移 |
| `migrate` | `(from, to) -> MigrateOutcome` | 逐项容错搬运；目标已存在则**跳过不覆盖**；单项走 `move_entry` |
| `move_entry` / `copy_recursive` | 私有 | `rename` 优先；失败则**复制到 `{to}.hinina-partial` 暂存名再改名**（见下） |
| `staging_path` / `remove_any` / `remove_source` | 私有 | 暂存路径 / 安全删除 / 复制后删源（均只告警） |
| `should_clear_pending` | `(&MigrateOutcome) -> bool` | 迁移后是否清除「待迁移来源」标记（= 无失败项） |
| `should_attempt_legacy` | `(&DataDirPointer, DataDirSource, legacy) -> bool` | 是否该重试临时目录搬家（三态语义，见下） |
| `update_pointer` | `(&Path, impl FnOnce(&mut DataDirPointer))` | **读-改-写**指针（见下） |
| `is_absolute_path` / `is_same_or_inside` | 私有 | Windows 下要求盘符前缀；路径包含判定按大小写不敏感 |

### `validate_target` 拒绝的情形

| 情形 | 后果 |
|---|---|
| 空路径 / 非绝对路径 | 相对路径会随进程工作目录漂移，数据位置不确定 |
| **旧临时目录本身或其子目录** | 又回到会被系统清理的位置，正是本次要修的问题 |
| 已存在的**文件** | 无法作为目录使用 |
| **`migrate` 且含冲突条目** | 迁移会跳过它们 → 陈旧数据被静默采用 |
| 不可创建 / 不可写 | 现在报错，而不是等到写工作区时才失败 |
| （Windows）无盘符前缀 | `/foo` 是「当前盘根目录」，语义随进程当前盘漂移 |

### 冲突检查**只在 `migrate` 时**生效 —— 「改了能不能改回去」的关键

| 意图 | `migrate` | 行为 |
|---|---|---|
| 「我要用那个目录里原来的数据」 | `false` | **放行** —— 没有迁移就没有跳过，不存在陈旧数据问题 |
| 「把我现在的数据搬过去」 | `true` | 目标有我们的条目则**拒绝**（否则静默用目标的旧数据） |

判据演进（两次都错在「把正常状态当异常」）：

1. 最初用 `dir_is_empty` → **真正用过的目录永远改不回去**（它必然含 `config.json`、`workspaces/`），而指引「请选择一个空目录」是在要求用户删掉自己的数据。
2. 改为无条件检查「冲突条目」→ 仍然**改不回去**（用过的目录必然有冲突条目）。真正的判据是**「用户想做什么」**：不迁移时目标里有什么都不影响。

同一判断在 `commands::data_dir_cmd::validate_reset` 里也是按 `migrate` 分档的，**两处必须一致**。

## 直接依赖

- `serde`（指针序列化）、`tracing`
- `core::error::{AppError, AppResult}`

## 被依赖

- `main.rs` — `.setup()` 里调 `prepare_startup`（**在 `AppContext::init` 之前**）
- `core::context::AppContext` — 持有 `default_data_dir` 与 `data_dir_source`
- `commands::data_dir_cmd` — `read_pointer` / `write_pointer` / `validate_target` / `dir_is_usable` / `legacy_dir`

## 逻辑流程

```
启动（main.rs 的 .setup()）:
  default_dir = app.path().app_local_data_dir()   // %LOCALAPPDATA%/{identifier}
  legacy      = %TEMP%/hinina
  prepare_startup(default_dir, legacy)
    ├ resolve: 指针指定(可用) → 默认(可用) → 临时目录(回退 + error 告警)
    ├ ① 指针带 migrate_from（设置页改目录）→ migrate(from → base_dir)；无失败项才清标记
    └ ② 首次搬家：source==Default && legacy_migrated != Some(true) && legacy 存在
         → migrate(legacy → base_dir)；无失败项才置位 legacy_migrated
  → 之后才 AppContext::init（会打开日志文件）

migrate(from, to):
  逐项：源不存在 → 跳过；目标已存在 → **skipped（不覆盖）**；rename 失败 → 复制 + 删源
  末尾：**只删空目录**（非递归）—— 见下

设置页改目录（commands::data_dir_cmd）:
  validate_target → write_pointer（data_dir / migrate_from）→ 返回 restartRequired
  **不在运行中搬运**，由下次启动完成
```

## 设计要点

- **指针固定放在默认目录**：绝不在自定义目录里找指针，否则「自定义目录在哪」本身就需要指针，形成递归。指针缺省即「用默认目录」。
- **指针写入是原子的（temp + rename）**：直接 `fs::write` 是 truncate-in-place，写到一半掉电/崩溃会留下空文件或半截 JSON —— `read_pointer` 虽能降级不崩，但那等于**静默丢掉用户自定义的目录设置**。
- **指针更新一律「读-改-写」（`update_pointer`）**：同一次启动里可能连续改指针（先清 `migrate_from`、再置 `legacy_migrated`）。曾用启动时读到的内存副本 `..pointer.clone()` 写入，把前一步刚清掉的字段**复活** —— 测试抓到过（清掉的 `migrate_from` 被后一次写入带回来）。
- **迁移只在启动时执行**：设置页改目录时只写指针 + 「待迁移来源」，由下次启动在 `Logger::init` **之前**完成搬运。若在运行中迁移，已迁移的旧目录与仍在写入的旧目录会产生分叉 —— 重启后这段写入就丢了。
- **两个迁移来源在同一次启动内都可能执行，故不提前返回**：曾 ①（`migrate_from`）成功后直接 `return`，导致 ②（临时目录搬家）被跳过；若 ② 先前失败过，用户中途改用自定义目录就会让那份数据**永远滞留**在会被系统清理的位置。
- **跨卷复制走暂存名再改名**（`move_entry`）：直接写目标时中途失败（磁盘满 / 文件被锁）会留下半拷贝，而重试逻辑用「目标是否存在」判完成 —— 半拷贝会被误判成「已跳过」，失败计数为零 → 迁移标记被清除 → **重试自解除，活跃数据目录永久残缺**。落到 `{to}.hinina-partial` 后改名，使 `to` 的存在性等价于「搬运完整完成」。暂存清理必须在 `rename` **快速路径之前**（否则快速路径成功时残片永远留着）。
- **临时目录搬家的三态标记**（`should_attempt_legacy`）：`None` = 从未尝试（默认目录下搬一次）；`Some(true)` = 已成功（**永不重试**，否则用户在新目录里删掉的旧工作区会被搬回来）；`Some(false)` = **试过但失败**（下次启动重试，且**不因用户改目录而放弃** —— 那份数据是真实数据，放弃了就会一直等被系统清理）。
- **搬迁触发用一次性标记，不用「目标目录为空」**：默认目录里几乎总是有 WebView2 的 `EBWebView/` profile（任何一次启动都会创建），用空目录当门槛等于**对每个老用户都永不迁移** —— 实测踩到，迁移静默不执行。
- **旧目录只删空目录（非递归）**：`remove_dir_all` 会把「迁移失败的条目」连同 `logs/` 一起删掉 —— 前者是**数据丢失**（用户以为数据搬过去了，实际被删了）。非递归删除只可能在确实什么都不剩时成功，天然安全。
- **`dir_is_empty` 不能用于数据目录校验**：我们**自己写进去的数据本身就是"非空"** —— 无论是默认目录（`data_dir.json` + `EBWebView` + `logs`）还是**用过的自定义目录**（`config.json`、`workspaces/`…）。用它当判据会让「恢复默认 + 迁移」在真机上永远失败（且指引是死循环），并让**用过的目录永远改不回去**。
- **两处校验判据一致（都是「冲突条目」）且都按 `migrate` 分档**：`set_data_dir` 走 `validate_target`、`reset_data_dir` 走 `validate_reset`。分档是「改不改得回去」的关键 —— 不迁移时没有跳过，目标里有什么都不影响；迁移时才需要拒绝冲突（否则静默用目标的旧数据）。详见上面的拒绝情形表。
- **`conflicting_entries` 派生自 `MIGRATED_ENTRIES` 而非另列一份**：只有 `cache` 被排除 —— 缓存可重建，被跳过只是让新目录从空缓存开始（TTL 自然填充），不构成「静默使用陈旧数据」。派生可避免将来往清单加条目时两处漂移。
- **迁移清单不含 `logs/`**：日志只服务近期排障，旧日志留在原地无损失；而它是唯一可能被进程占用的目录，搬它容易失败。
- **目标已存在则跳过而不是覆盖**：重试场景下这是常态（幂等），且绝不会用旧数据盖掉新数据。
- **逐项容错**：单项失败不影响其余项；失败时**不置位标记**，下次启动重试。
- **回退临时目录仍可用**：默认与指定目录都不可用时退到 `%TEMP%/hinina`，调用方（`main.rs`）必须 `error` 级告警，且界面要显眼提示 —— 「能打完比赛」优先于「数据位置绝对干净」。
- **迁移失败不阻断启动**：数据仍在原处，客户端能正常工作；`main.rs` 记 `error` 并列出失败条目。

## 测试

`src-tauri/src/infra/tests/data_dir_tests.rs`（全部基于独立临时目录，不触碰真实 `%LOCALAPPDATA%` / `%TEMP%/hinina`）：

- **指针**：往返读写、缺失/损坏降级为默认、未设置字段不落盘（`{}`）、**原子写不留 `.json.tmp` 残片**、**读-改-写不复活已清字段**。
- **冲突条目**：**真机默认目录（`data_dir.json` + `EBWebView` + `logs`）不得判为冲突**（HIGH 缺陷回归）、陈旧 `config.json`/`workspaces`/… 逐个报出且不误报必然存在的条目、**`cache` 刻意不算冲突**（同时断言它确实在迁移清单里）、目录不存在时无冲突。
- **可用性**：创建缺失目录且不留探针、拒绝文件路径、`dir_is_empty` 三态。
- **校验**：接受全新绝对路径、**接受「用过但不迁移」的目录**（回归：否则用户改不回去）；拒绝相对路径、旧临时目录及其子目录、文件、**迁移到含冲突条目的目录**（错误点名条目、给出可行出路、不诱导清空）、空串。
- **迁移**：按清单顺序搬运且**不含 logs**、目标已存在则跳过不覆盖、只有 logs 时 no-op、搬空后删除旧目录、**绝不删除未迁移的条目**、`move_entry` 对缺失源报错、**失败后不留目标与暂存（使「存在」成为完成的可信信号）**、**上次失败的条目会重试而不是被跳过**、重试前清理陈旧暂存。
- **启动准备**：无指针用默认目录、空目录下迁移 legacy、**默认目录含 WebView profile 时仍必须迁移**（实测缺陷的回归测试）、**不重复迁移**（删掉的工作区不复活）、目标已有条目时跳过但其余照搬、自定义目录时不硬塞 legacy、待迁移闭环（搬运 → 清标记 → 二次启动 no-op）、指定目录不可用时的回退与迁移、**两个迁移来源在同一次启动内都执行**、**失败过的 legacy 在改用自定义目录后仍被搬过去**、`should_attempt_legacy` 三态矩阵、`should_clear_pending` 决策锁定。
