# data_dir_cmd（数据目录命令）

> 源文件：`src-tauri/src/commands/data_dir_cmd.rs`

## 职责

设置页「数据目录」的三件事：看当前用哪个目录、改到别处、恢复默认，外加一个原生目录选择器。

**改动一律「下次启动生效」**：各 Service 都持有以 base_dir 为根的 `Storage`，日志还握着文件句柄 —— 运行中热切等于重建整个 `AppContext`，风险极高且收益为零（选手改数据目录是低频动作）。故命令只做「校验 + 写位置指针」，真正的搬运由下次启动在 `Logger::init` 之前完成（见 `infra/data_dir.md`）。

**也不在运行中迁移**：那样会让「已迁移的旧目录」与「仍在写入的旧目录」产生分叉，重启后这段写入就丢了。改目录时只把「待迁移来源」写进指针。

## 核心类型/函数

| 名称 | 签名 | 说明 |
|------|------|------|
| `DataDirInfo` | VO（`Serialize`，camelCase） | `currentDir` / `defaultDir` / `source` / `restartRequired` |
| `DataDirChange` | VO（`Serialize`，camelCase） | `targetDir` / `migrateFrom`（可为 null）/ `restartRequired`（恒 true） |
| `get_data_dir` | `async fn(State<'_, AppContext>) -> AppResult<DataDirInfo>` | 读当前目录 + 指针；`restartRequired` = 指针指向别处或有待迁移来源 |
| `set_data_dir` | `async fn(State, path: String, migrate: bool) -> AppResult<DataDirChange>` | 校验（`validate_target`）→ 写指针；拒绝「与当前目录相同」。**不勾迁移时可改回任何用过的目录** || `reset_data_dir` | `async fn(State, migrate: bool) -> AppResult<DataDirChange>` | 回到默认目录；校验走 `validate_reset`（见下） |
| `validate_reset` | `pub(crate) fn(&Path, &Path, bool) -> AppResult<()>` | **抽出的校验决策**（命令本体依赖 `State` 无法单测，而判据正是 HIGH 缺陷漏网处） |
| `pick_data_dir` | `async fn(tauri::AppHandle) -> AppResult<Option<String>>` | 原生目录选择器；取消返回 `None` |
| `write_target` | 私有 | 组装并写入指针（`data_dir=None` 表示用默认目录） |

## 直接依赖

- `tauri`（`State` / `AppHandle`）、`tauri-plugin-dialog`（`DialogExt`，`pick_folder`）、`tokio::sync::oneshot`、`serde`、`tracing`
- `crate::core::context::AppContext`（`storage` / `default_data_dir`）
- `crate::core::error::{AppError, AppResult}`
- `crate::infra::data_dir`（`validate_target` / `dir_is_usable` / `read_pointer` / `write_pointer` / `legacy_dir` / `DataDirPointer`）

## 被依赖

- `main.rs` — `generate_handler!` 注册 4 条命令
- `src/bridge/system.bridge.ts` → `system.service.ts` → `views/SettingsView.vue`「数据目录」区块

## 逻辑流程

```
get_data_dir
  → currentDir = storage.base_dir()
  → pointer = read_pointer(default_data_dir)
  → restartRequired = pointer.migrate_from.is_some()
                      || pointer.data_dir != currentDir

set_data_dir(path, migrate)
  → validate_target(path, legacy)      // 六种拒绝情形见 infra/data_dir.md
  → 与当前目录相同？→ Config 错误
  → write_pointer { data_dir: path, migrate_from: migrate ? current : null }
  → DataDirChange { targetDir, migrateFrom, restartRequired: true }

reset_data_dir(migrate)
  → dir_is_usable(default_data_dir) 不通过？→ Config 错误
  → 已在默认目录？→ Config 错误
  → write_pointer { data_dir: None, migrate_from: migrate ? current : null }

pick_data_dir
  → app.dialog().file().pick_folder(回调)
  → oneshot 收 FilePath → into_path() → String
```

## 设计要点

- **两处校验判据一致，且都按 `migrate` 分档**：`set_data_dir` 走 `validate_target`、`reset_data_dir` 走 `validate_reset`。分档是「**改了能不能改回去**」的关键 —— 不迁移时没有跳过，目标目录里有什么都不影响（用户是在明确选择「用那个目录里原来的数据」）；只有**迁移**时才需要拒绝冲突条目（否则会静默用目标的旧数据）。详见 `infra/data_dir.md`。
- **不勾迁移时无需校验**：用户只是想切回默认目录，里面有什么就是什么（那是他自己的选择）。
- **校验决策抽成 `validate_reset` 纯函数**：命令本体依赖 `tauri::State` 无法单测，而上面那条判据正是 HIGH 缺陷漏网的原因 —— 抽出来才有人能锁住它（有 5 条用例，含「真机默认目录不得被拒」的回归测试，且经变异测试确认非空转）。
- **`pick_data_dir` 用「回调 + oneshot」而不是 `blocking_pick_folder`**：后者会阻塞当前线程，而命令跑在异步运行时上（阻塞工作线程是浪费，某些平台还要求弹窗在主线程）。
- **`write_target` 读改写而不是整体覆盖指针**：`legacy_migrated`（临时目录一次性搬家标记）由启动流程管理，设置页改目录不该把它抹掉。
- **改动恒为 `restartRequired: true`**：不是保守，是事实 —— 本命令不搬运、不切换，界面必须说清「当前仍在使用旧目录」。
- **校验放在命令层而不是界面层**：前端不是唯一防线（与 `update_config` 同款约定）。

## 测试

命令本体依赖 `tauri::State`（无公开构造器）无法直接单测（见 `commands/mod.md`），故覆盖其可测部分：

- `commands/tests/mod_tests.rs`：`DataDirInfo` / `DataDirChange` 的 camelCase 线上形状锁定、`migrateFrom` 为 null 的表示、`DataDirSource` 三个变体的序列化字符串（`fallbackTemp` 是界面必须显眼告警的那一种）。
- `infra/tests/data_dir_tests.rs`：全部校验分支与迁移语义（见 `infra/data_dir.md`）。
