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
| `dir_is_empty` | `(&Path) -> bool` | 不存在或为空（读不了时按「非空」保守处理） |
| `validate_target` | `(&Path, legacy) -> AppResult<PathBuf>` | 设置页「更改目录」的校验（见下） |
| `resolve` | `(default_dir, legacy) -> DataDirPlan` | 解析优先级：指针指定（可用）→ 默认（可用）→ 临时目录（回退 + 告警） |
| `prepare_startup` | `(default_dir, legacy) -> (DataDirPlan, MigrateOutcome)` | 启动用：解析 + 一次性迁移 |
| `migrate` | `(from, to) -> MigrateOutcome` | 逐项容错搬运；目标已存在则**跳过不覆盖** |
| `move_entry` / `copy_recursive` | 私有 | `rename` 优先，失败降级递归复制 + 删源（跨卷必需） |
| `should_clear_pending` | `(&MigrateOutcome) -> bool` | 迁移后是否清除「待迁移来源」标记（= 无失败项）。抽成纯函数以便锁定决策 |
| `is_absolute_path` / `is_same_or_inside` | 私有 | Windows 下要求盘符前缀；路径包含判定按大小写不敏感 |

### `validate_target` 拒绝的六种情形

| 情形 | 后果 |
|---|---|
| 空路径 / 非绝对路径 | 相对路径会随进程工作目录漂移，数据位置不确定 |
| **旧临时目录本身或其子目录** | 又回到会被系统清理的位置，正是本次要修的问题 |
| 已存在的**文件** | 无法作为目录使用 |
| **非空目录** | 覆盖会丢数据、合并会混入别人的配置 → 要求空目录 |
| 不可创建 / 不可写 | 现在报错，而不是等到写工作区时才失败 |
| （Windows）无盘符前缀 | `/foo` 是「当前盘根目录」，语义随进程当前盘漂移 |

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
- **迁移只在启动时执行**：设置页改目录时只写指针 + 「待迁移来源」，由下次启动在 `Logger::init` **之前**完成搬运。若在运行中迁移，已迁移的旧目录与仍在写入的旧目录会产生分叉 —— 重启后这段写入就丢了。
- **搬迁触发用一次性标记，不用「目标目录为空」**：默认目录里几乎总是有 WebView2 的 `EBWebView/` profile（任何一次启动都会创建），用空目录当门槛等于**对每个老用户都永不迁移** —— 实测踩到，迁移静默不执行。也不能「每次启动都尝试」：用户在新目录里删掉的旧工作区会被反复搬回来。
- **旧目录只删空目录（非递归）**：`remove_dir_all` 会把「迁移失败的条目」连同 `logs/` 一起删掉 —— 前者是**数据丢失**（用户以为数据搬过去了，实际被删了）。非递归删除只可能在确实什么都不剩时成功，天然安全。
- **迁移清单不含 `logs/`**：日志只服务近期排障，旧日志留在原地无损失；而它是唯一可能被进程占用的目录，搬它容易失败。
- **目标已存在则跳过而不是覆盖**：重试场景下这是常态（幂等），且绝不会用旧数据盖掉新数据。
- **逐项容错**：单项失败不影响其余项；失败时**不置位标记**，下次启动重试。
- **回退临时目录仍可用**：默认与指定目录都不可用时退到 `%TEMP%/hinina`，调用方（`main.rs`）必须 `error` 级告警，且界面要显眼提示 —— 「能打完比赛」优先于「数据位置绝对干净」。
- **迁移失败不阻断启动**：数据仍在原处，客户端能正常工作；`main.rs` 记 `error` 并列出失败条目。

## 测试

`src-tauri/src/infra/tests/data_dir_tests.rs`（全部基于独立临时目录，不触碰真实 `%LOCALAPPDATA%` / `%TEMP%/hinina`）：

- **指针**：往返读写、缺失/损坏降级为默认、未设置字段不落盘（`{}`）。
- **可用性**：创建缺失目录且不留探针、拒绝文件路径、`dir_is_empty` 三态。
- **校验**：接受全新绝对路径；拒绝相对路径、旧临时目录及其子目录、文件、非空目录、空串。
- **迁移**：按清单顺序搬运且**不含 logs**、目标已存在则跳过不覆盖、只有 logs 时 no-op、搬空后删除旧目录、**绝不删除未迁移的条目**（本模块最危险的路径）、`move_entry` 对缺失源报错。
- **启动准备**：无指针用默认目录、空目录下迁移 legacy、**默认目录含 WebView profile 时仍必须迁移**（实测缺陷的回归测试）、**不重复迁移**（删掉的工作区不复活）、目标已有条目时跳过但其余照搬、自定义目录时不硬塞 legacy、待迁移闭环（搬运 → 清标记 → 二次启动 no-op）、指定目录不可用时的回退与迁移、`should_clear_pending` 决策锁定。
