// 数据目录解析、位置指针与一次性迁移。
//
// **为什么需要这个模块**：数据根目录（base_dir）此前硬编码为 `%TEMP%/hinina`，
// 而临时目录会被 Windows 磁盘清理、第三方清理工具或系统策略**随时清空** ——
// 那意味着选手的工作区代码与提交留档无声消失且不可恢复。现在改为
// `app_local_data_dir()`（`%LOCALAPPDATA%/{identifier}`，**不随域漫游**），
// 并允许用户在设置页指定别处。
//
// **位置指针**（[`POINTER_FILE`]）**固定放在默认目录**下，绝不在自定义目录里找指针
// （否则「自定义目录在哪」本身就需要指针，形成递归）。指针缺省即「用默认目录」。
//
// **迁移只在启动时执行**（见 [`prepare_startup`]）：设置页改目录时只写指针 +
// 「待迁移来源」，由下次启动在 `Logger::init` **之前**完成搬运。若在运行中迁移，
// 已迁移的旧目录与仍在写入的旧目录会产生分叉 —— 重启后这段写入就丢了。
//
// **迁移清单不含 `logs/`**（[`MIGRATED_ENTRIES`]）：日志只服务近期排障，
// 旧日志留在原地无损失，而它恰恰是唯一可能被进程占用的目录。
//
// **搬迁触发用一次性标记，不用「目标目录为空」**：默认目录里几乎总是有 WebView2 的
// `EBWebView/` profile，用空目录当门槛等于对每个老用户都永不迁移（实测踩到）。
//
// **跨卷复制先落暂存名再改名**：直接写目标时中途失败会留下半拷贝，而重试逻辑用
// 「目标是否存在」判完成 —— 半拷贝会被误判成「已跳过」，失败计数为零，迁移标记被
// 清除，数据目录永久残缺。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::core::error::{AppError, AppResult};

/// 位置指针文件名（固定放在**默认数据目录**下）。
const POINTER_FILE: &str = "data_dir.json";

/// 临时目录下的旧数据目录名（历史遗留位置）。
const LEGACY_DIR_NAME: &str = "hinina";

/// 迁移时搬运的条目。
///
/// **刻意不含 `logs/`**：日志只服务近期排障，旧日志留在原地没有损失；而它是唯一
/// 可能被当前进程占用的目录，搬它容易失败并让整个迁移看起来"没成功"。
const MIGRATED_ENTRIES: &[&str] = &[
    "config.json",
    "sessions",
    "workspaces",
    "submissions",
    "announcements_read",
    "cache",
];

/// 缓存目录名（[`MIGRATED_ENTRIES`] 的一员，但**不算「冲突条目」**，见下）。
const CACHE_ENTRY: &str = "cache";

/// 写盘可用性探针的文件名（写成功即立刻删除）。
const PROBE_FILE: &str = ".hinina-write-probe";

/// 数据目录的来源（供界面如实展示「当前到底在用哪个目录、为什么」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DataDirSource {
    /// 用户未指定，使用默认目录（正常情况）
    Default,
    /// 用户指定的目录
    Custom,
    /// 默认与指定目录都不可用，回退到临时目录（**强告警**：数据随时可能被系统清理）
    FallbackTemp,
}

impl DataDirSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Custom => "custom",
            Self::FallbackTemp => "fallbackTemp",
        }
    }
}

/// 启动时解析出的数据目录方案。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDirPlan {
    /// 实际使用的数据根目录
    pub base_dir: PathBuf,
    /// 默认数据目录（设置页「恢复默认」的目标，也是指针文件所在处）
    pub default_dir: PathBuf,
    pub source: DataDirSource,
}

/// 位置指针文件结构。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DataDirPointer {
    /// 用户指定的数据目录（`None` = 用默认目录）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_dir: Option<String>,
    /// **待迁移来源**：设置页改目录时勾了「迁移现有数据」时写入，
    /// 由下次启动完成搬运后清除（见模块头注释）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migrate_from: Option<String>,
    /// **一次性标记**：临时目录的旧数据搬家结果。
    ///
    /// 为什么需要它而不是「目标目录为空」：默认目录里几乎**总是**有 WebView2 的
    /// `EBWebView/` profile（任何一次启动都会创建），用「空目录」当门槛等于
    /// **对每个老用户都永不迁移** —— 实测踩到，迁移静默不执行。
    ///
    /// 也不能「每次启动都尝试」：用户在新目录里删掉的旧工作区会被反复搬回来。
    ///
    /// 三态语义（见 `should_attempt_legacy`）：
    /// - `None`：从未尝试（全新升级场景 → 默认目录下搬一次）
    /// - `Some(true)`：已成功搬完 → **永不重试**
    /// - `Some(false)`：**试过但失败** → 下次启动重试，且**不因用户改目录而放弃**
    ///   （否则那份数据会永远留在会被系统清理的临时目录里）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_migrated: Option<bool>,
}

/// 迁移结果（供日志与界面如实汇报）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrateOutcome {
    /// 成功搬运的条目
    pub moved: Vec<String>,
    /// 因目标已存在而跳过的条目（不覆盖既有数据）
    pub skipped: Vec<String>,
    /// 搬运失败的条目（rename 与 copy 都不成）
    pub failed: Vec<String>,
    /// 旧目录是否已被删除（**只有旧目录确实空了才可能为真**）。
    ///
    /// `logs/` 按约定不迁移，故通常为 `false`（旧目录留下一个 logs 子目录）。
    /// 也正因如此，这个字段不能用来判断「迁移是否成功」—— 那要看 [`MigrateOutcome::failed`]。
    pub legacy_removed: bool,
}

impl MigrateOutcome {
    /// 是否确实做了搬运（用于决定要不要打日志/提示）。
    pub fn is_noop(&self) -> bool {
        self.moved.is_empty() && self.skipped.is_empty() && self.failed.is_empty()
    }

    /// 是否全部成功（无失败项）。
    pub fn is_ok(&self) -> bool {
        self.failed.is_empty()
    }

    /// 并入另一次迁移的结果（同一启动内可能先搬 `migrate_from` 再搬临时目录）。
    pub fn merge(&mut self, other: MigrateOutcome) {
        self.moved.extend(other.moved);
        self.skipped.extend(other.skipped);
        self.failed.extend(other.failed);
        self.legacy_removed |= other.legacy_removed;
    }
}

/// 临时目录下的旧数据目录（`%TEMP%/hinina`）。
pub fn legacy_dir() -> PathBuf {
    std::env::temp_dir().join(LEGACY_DIR_NAME)
}

/// 位置指针文件的路径（**始终在默认目录下**）。
pub fn pointer_path(default_dir: &Path) -> PathBuf {
    default_dir.join(POINTER_FILE)
}

/// 读取位置指针；文件不存在或损坏一律按「未指定」处理（保守回退默认目录）。
///
/// 指针损坏**绝不阻断启动**：它只是「用户想去哪个目录」的提示，回退默认目录仍能
/// 正常工作；报错反而会让客户端打不开。
pub fn read_pointer(default_dir: &Path) -> DataDirPointer {
    let path = pointer_path(default_dir);
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return DataDirPointer::default();
    };
    match serde_json::from_str::<DataDirPointer>(&raw) {
        Ok(pointer) => pointer,
        Err(e) => {
            warn!(path = %path.display(), error = %e, "数据目录指针解析失败，按默认目录处理");
            DataDirPointer::default()
        }
    }
}

/// 写入位置指针（自动创建默认目录）。
///
/// **原子写**（temp + rename）：直接 `fs::write` 是 truncate-in-place，写到一半掉电/
/// 崩溃会留下空文件或半截 JSON。虽然 [`read_pointer`] 对损坏降级为「用默认目录」
/// 不会崩，但那等于**静默丢掉用户自定义的目录设置**（他下次启动会发现数据「换位置了」）。
/// 同目录改名是原子的，故读者要么看到旧内容、要么看到新内容。
pub fn write_pointer(default_dir: &Path, pointer: &DataDirPointer) -> AppResult<()> {
    std::fs::create_dir_all(default_dir)
        .map_err(|e| AppError::Io(format!("创建默认数据目录失败 {}: {}", default_dir.display(), e)))?;
    let json = serde_json::to_string_pretty(pointer)
        .map_err(|e| AppError::Serialization(format!("序列化数据目录指针失败: {}", e)))?;

    let path = pointer_path(default_dir);
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, json)
        .map_err(|e| AppError::Io(format!("写入数据目录指针失败 {}: {}", temp.display(), e)))?;
    std::fs::rename(&temp, &path).map_err(|e| {
        // 改名失败时清掉临时文件，避免下次启动看到 `.json.tmp` 残片
        let _ = std::fs::remove_file(&temp);
        AppError::Io(format!("替换数据目录指针失败 {}: {}", path.display(), e))
    })
}

/// 目录是否可直接使用（能创建、能写入）。探测文件写成功后立刻删除。
///
/// 只做**最小可用性验证**（创建 + 写一个探针文件），不尝试判断磁盘配额：
/// 目标是「启动时就能发现目录不可用并回退」，而不是精确预测未来是否写得下。
pub fn dir_is_usable(dir: &Path) -> bool {
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(PROBE_FILE);
    if std::fs::write(&probe, b"probe").is_err() {
        return false;
    }
    let _ = std::fs::remove_file(&probe);
    true
}

/// 目录是否不存在或为空（迁移的前置条件）。
pub fn dir_is_empty(dir: &Path) -> bool {
    match std::fs::read_dir(dir) {
        Ok(mut entries) => entries.next().is_none(),
        // 不存在 = 空；读不了时按「非空」处理（保守：宁可不迁移，也不覆盖）
        Err(e) => e.kind() == std::io::ErrorKind::NotFound,
    }
}

/// 迁移会**跳过**、且跳过会导致「静默使用陈旧数据」的条目。
///
/// = [`MIGRATED_ENTRIES`] 去掉 `cache`：缓存可重建，被跳过只是让新目录从空缓存开始
/// （TTL 自然填充），不构成陈旧数据问题。**派生而非另列一份**，避免两处漂移。
pub fn conflicting_entries() -> impl Iterator<Item = &'static str> {
    MIGRATED_ENTRIES.iter().copied().filter(|e| *e != CACHE_ENTRY)
}

/// `dir` 中已存在的**冲突条目**（迁移会跳过它们 → 陈旧数据被静默采用）。
///
/// 这是「恢复默认 + 迁移」的校验判据。**绝不能用 [`dir_is_empty`]**：默认目录按本模块
/// 的设计**必然非空** ——
/// - `data_dir.json`（位置指针**固定**存于默认目录，见 [`pointer_path`]）；
/// - `EBWebView/`（WebView2 的 profile，每次启动都会重建，见
///   `prepare_startup_migrates_even_when_default_dir_has_webview_profile` 的回归测试）；
/// - 可能还有 `logs/`。
///
/// 三者都不在迁移清单里、都不冲突，却会让 `dir_is_empty` 恒为 false → 校验永远失败，
/// 且「清空该目录」的指引是**死循环**：`EBWebView` 运行中被 WebView2 锁住删不掉、
/// 下次启动又先于设置页重建；删 `data_dir.json` 则自定义目录指针丢失 →
/// `source` 变回 `default` → 「恢复默认」按钮（仅自定义目录时渲染）直接消失。
pub fn conflicting_entries_in(dir: &Path) -> Vec<&'static str> {
    conflicting_entries()
        .filter(|entry| dir.join(entry).exists())
        .collect()
}

/// 路径是否等于 `base` 或位于其内部（Windows 下按大小写不敏感比较）。
fn is_same_or_inside(path: &Path, base: &Path) -> bool {
    let normalize = |p: &Path| {
        let s = p.to_string_lossy().replace('\\', "/");
        let s = s.trim_end_matches('/').to_string();
        if cfg!(windows) {
            s.to_lowercase()
        } else {
            s
        }
    };
    let (p, b) = (normalize(path), normalize(base));
    p == b || p.starts_with(&format!("{}/", b))
}

/// 校验用户指定的数据目录（设置页「更改目录」）。
///
/// 拒绝的每一种情形都有具体后果，不是形式主义：
/// - 非绝对路径 → 相对路径会随进程工作目录漂移，等于数据位置不确定；
/// - 指向旧临时目录 → 又回到会被系统清理的位置，正是本次要修的问题；
/// - 指向已存在的文件 → 无法作为目录使用；
/// - **含冲突条目** → 迁移会跳过它们，界面将**静默采用那里的陈旧数据**（见
///   [`conflicting_entries_in`]）；
/// - 不可创建 / 不可写 → 现在就要报错，而不是等到写工作区时才失败。
///
/// **判据是「冲突条目」而不是「目录非空」**（曾经用 [`dir_is_empty`]，那是错的）：
/// 我们自己写进去的数据本身就是"非空" —— 于是**离开过的自定义目录再也选不回来**
/// （它必然含 `workspaces/`、`config.json` 等），而错误指引「请选择一个空目录」实际是在
/// 要求用户**删掉自己的数据**。目录里的**无关**文件则不影响：迁移对已存在条目是**跳过**
/// 而非覆盖，我们只创建自己的条目，不会动用户的东西。
pub fn validate_target(path: &Path, legacy: &Path) -> AppResult<PathBuf> {
    if path.as_os_str().is_empty() {
        return Err(AppError::Config("数据目录不能为空".into()));
    }
    if !is_absolute_path(path) {
        return Err(AppError::Config(format!(
            "数据目录必须是绝对路径: {}",
            path.display()
        )));
    }
    if is_same_or_inside(path, legacy) {
        return Err(AppError::Config(format!(
            "不能把数据目录设到临时目录（{}）—— 那里随时可能被系统清理",
            legacy.display()
        )));
    }
    if path.is_file() {
        return Err(AppError::Config(format!(
            "目标是一个文件，不是目录: {}",
            path.display()
        )));
    }
    let conflicts = conflicting_entries_in(path);
    if !conflicts.is_empty() {
        return Err(AppError::Config(format!(
            "目标目录已有 {}（{}）—— 迁移会跳过它们，界面将采用那里的陈旧数据。\
             请换一个目录，或先备份并删除这些条目",
            conflicts.join(" / "),
            path.display()
        )));
    }
    if !dir_is_usable(path) {
        return Err(AppError::Config(format!(
            "目标目录不可创建或不可写入: {}",
            path.display()
        )));
    }
    Ok(path.to_path_buf())
}

/// 是否为绝对路径。
///
/// Windows 上额外要求**盘符前缀**：`Path::is_absolute()` 对 `/foo` 也返回 true，
/// 但那是「当前盘根目录」，语义随进程当前盘漂移，不作为数据目录接受。
fn is_absolute_path(path: &Path) -> bool {
    if !path.is_absolute() {
        return false;
    }
    if cfg!(windows) {
        use std::path::Component;
        return matches!(path.components().next(), Some(Component::Prefix(_)));
    }
    true
}

/// 解析启动时的数据目录（不含迁移，供测试与「只想看方案」的场景使用）。
///
/// 优先级：指针指定的目录（且可用）→ 默认目录（且可用）→ 临时目录（回退 + 告警）。
pub fn resolve(default_dir: &Path, legacy: &Path) -> DataDirPlan {
    let pointer = read_pointer(default_dir);

    if let Some(custom) = pointer.data_dir.as_deref().filter(|s| !s.trim().is_empty()) {
        let path = PathBuf::from(custom);
        if is_absolute_path(&path) && dir_is_usable(&path) {
            return DataDirPlan {
                base_dir: path,
                default_dir: default_dir.to_path_buf(),
                source: DataDirSource::Custom,
            };
        }
        warn!(
            dir = %path.display(),
            "指定的数据目录不可用，回退默认目录"
        );
    }

    if dir_is_usable(default_dir) {
        return DataDirPlan {
            base_dir: default_dir.to_path_buf(),
            default_dir: default_dir.to_path_buf(),
            source: DataDirSource::Default,
        };
    }

    // 默认目录都不可用（权限策略锁定 AppData / 磁盘满）：回退临时目录。
    // **能打完比赛**优先于「数据位置绝对干净」—— 调用方必须据此强告警，
    // 并在界面上让选手看到真实路径（见 `get_data_dir` 命令）。
    warn!(
        default_dir = %default_dir.display(),
        fallback = %legacy.display(),
        "默认数据目录不可用，回退临时目录（数据随时可能被系统清理）"
    );
    let _ = std::fs::create_dir_all(legacy);
    DataDirPlan {
        base_dir: legacy.to_path_buf(),
        default_dir: default_dir.to_path_buf(),
        source: DataDirSource::FallbackTemp,
    }
}

/// 启动时准备数据目录：解析方案 + 执行一次性迁移。
///
/// 两个迁移来源**在同一次启动内都可能执行**，故这里**不提前返回**：
/// ① 指针带 `migrate_from`（设置页改目录时勾了「迁移现有数据」）；
/// ② 临时目录的旧数据尚未成功搬家（见 [`should_attempt_legacy`]）。
///
/// 曾经 ① 成功后直接 `return`，导致 ② 被跳过 —— 若旧临时目录的搬家先前失败过，
/// 用户中途改用自定义目录就会让那份数据**永远滞留**在会被系统清理的位置。
///
/// ② 刻意**不用「目标目录为空」**当门槛：默认目录里几乎总是有 WebView2 的
/// `EBWebView/` profile，用空目录当条件等于永不迁移（实测踩到）。
///
/// 迁移失败**不阻断启动**：调用方继续用解析出的 `base_dir`（数据仍在原处，
/// 客户端能正常工作），但必须强告警 —— 这正是「回退仍可用」优于「启动失败」的场景。
pub fn prepare_startup(default_dir: &Path, legacy: &Path) -> (DataDirPlan, MigrateOutcome) {
    let pointer = read_pointer(default_dir);
    let plan = resolve(default_dir, legacy);
    let mut outcome = MigrateOutcome::default();

    // ① 显式要求的迁移（设置页改目录）
    if let Some(from) = pointer
        .migrate_from
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
    {
        if from != plan.base_dir && from.exists() {
            let step = migrate(&from, &plan.base_dir);
            // 无失败项才清除标记：有失败项时保留，下次启动重试（已搬过去的条目
            // 会因「目标已存在」被跳过，故重试是幂等的，不会重复搬运）
            if should_clear_pending(&step) {
                clear_migrate_from(default_dir);
            }
            outcome.merge(step);
        } else {
            // 来源不存在或与目标相同：标记已无意义，清掉避免每次启动都尝试
            debug!(from = %from.display(), "待迁移来源不存在或与目标相同，清除标记");
            clear_migrate_from(default_dir);
        }
    }

    // ② 临时目录搬家（不提前返回，理由见函数注释）
    if should_attempt_legacy(&pointer, plan.source, legacy) {
        let step = migrate(legacy, &plan.base_dir);
        // 成功 → Some(true)（永不重试）；失败 → Some(false)（**换个目录也继续重试**）
        mark_legacy_migrated(default_dir, step.is_ok());
        outcome.merge(step);
    }

    (plan, outcome)
}

/// 是否该尝试把临时目录的旧数据搬到当前数据目录。
///
/// | `legacy_migrated` | 当前目录来源 | 行为 |
/// |---|---|---|
/// | `Some(true)` | 任意 | **永不**（否则用户在新目录里删掉的旧工作区会被搬回来） |
/// | `Some(false)` | 任意 | **重试** —— 上次试过但失败，欠着这份数据 |
/// | `None` | 默认 | 搬（全新升级场景） |
/// | `None` | 自定义 / 回退临时 | 不搬（用户主动选了别处且没要求迁移，尊重其选择） |
///
/// `Some(false)` 之所以**不看来源**：它意味着「我们确实尝试过、且失败」，那份数据是
/// 用户的真实数据；若因为用户中途改了目录就放弃，它会一直留在临时目录里等被系统清理。
fn should_attempt_legacy(pointer: &DataDirPointer, source: DataDirSource, legacy: &Path) -> bool {
    if pointer.legacy_migrated == Some(true) || !legacy.exists() {
        return false;
    }
    pointer.legacy_migrated == Some(false) || source == DataDirSource::Default
}

/// 读-改-写位置指针（写失败只告警：下次启动会重试，不影响正确性）。
///
/// **必须重新读盘**，不能用启动时读到的内存副本 `..pointer.clone()` 去写：同一次启动里
/// 可能连续改指针（先清 `migrate_from`、再置 `legacy_migrated`），拿陈旧副本写会把前一步
/// 刚清掉的字段**复活** —— 测试抓到过：清掉的 `migrate_from` 被后一次写入带回来，
/// 于是「迁移已完成」的标记永远清不掉。
fn update_pointer(default_dir: &Path, mutate: impl FnOnce(&mut DataDirPointer)) {
    let mut pointer = read_pointer(default_dir);
    mutate(&mut pointer);
    if let Err(e) = write_pointer(default_dir, &pointer) {
        warn!(error = %e, "更新数据目录指针失败（下次启动会重试，不影响正确性）");
    }
}

/// 记录临时目录搬家的结果。
///
/// 成功写 `Some(true)`（一次性标记，永不重试）；失败写 `Some(false)`（下次启动重试，
/// 且**不因用户改目录而放弃** —— 见 [`should_attempt_legacy`]）。
fn mark_legacy_migrated(default_dir: &Path, succeeded: bool) {
    update_pointer(default_dir, |pointer| {
        pointer.legacy_migrated = Some(succeeded);
    });
}

/// 迁移后是否应清除「待迁移来源」标记。
///
/// **失败时保留**：下次启动重试，而不是让用户的数据永远留在旧目录里（他以为搬完了）。
/// 抽成纯函数是因为「迁移失败」在单测里无法确定性构造（需要真实的文件占用），
/// 而这条决策本身必须被锁定。
fn should_clear_pending(outcome: &MigrateOutcome) -> bool {
    outcome.is_ok()
}

/// 清除指针里的「待迁移来源」字段。
fn clear_migrate_from(default_dir: &Path) {
    update_pointer(default_dir, |pointer| {
        pointer.migrate_from = None;
    });
}

/// 把 `from` 下 [`MIGRATED_ENTRIES`] 搬到 `to`，返回逐项结果。
///
/// **逐项容错**：单项失败不影响其余项（一个被占用的文件不该让整场数据搬不过去）。
/// 目标已存在的条目**跳过而不覆盖** —— 覆盖等于用旧数据盖掉新数据。
pub fn migrate(from: &Path, to: &Path) -> MigrateOutcome {
    let mut outcome = MigrateOutcome::default();

    if std::fs::create_dir_all(to).is_err() {
        warn!(to = %to.display(), "创建目标数据目录失败，迁移放弃");
        outcome.failed.push("(目标目录)".to_string());
        return outcome;
    }

    for entry in MIGRATED_ENTRIES {
        let src = from.join(entry);
        if !src.exists() {
            continue;
        }
        let dst = to.join(entry);
        if dst.exists() {
            // 不覆盖既有数据（重试场景下这是常态，不是错误）
            outcome.skipped.push((*entry).to_string());
            continue;
        }
        match move_entry(&src, &dst) {
            Ok(()) => outcome.moved.push((*entry).to_string()),
            Err(e) => {
                warn!(entry, from = %src.display(), to = %dst.display(), error = %e, "迁移条目失败");
                outcome.failed.push((*entry).to_string());
            }
        }
    }

    // 旧目录整体删除：**只删空目录（非递归）**。
    //
    // 刻意不用 `remove_dir_all`：它是递归的，会把「迁移失败的条目」连同 `logs/`
    // 一起删掉 —— 前者是**数据丢失**（用户以为数据搬过去了，实际被删了）。
    // 非递归删除只可能在确实什么都不剩时成功，天然安全。
    // `logs/` 按约定不搬，故通常删不掉、旧目录留在原地 —— 那是预期，不是失败。
    if from.exists() && std::fs::remove_dir(from).is_ok() {
        outcome.legacy_removed = true;
    }

    if !outcome.is_noop() {
        info!(
            from = %from.display(),
            to = %to.display(),
            moved = ?outcome.moved,
            skipped = ?outcome.skipped,
            failed = ?outcome.failed,
            legacy_removed = outcome.legacy_removed,
            "数据目录迁移完成"
        );
    }
    outcome
}

/// 搬运单个条目：优先 `rename`（同卷零拷贝），失败则**先复制到暂存名再改名**后删除源。
///
/// 跨卷时 `rename` 在 Windows 上返回 `ERROR_NOT_SAME_DEVICE`，必须降级 —— 用户把
/// 数据目录设到另一个盘是常见需求。
///
/// **复制先落到 `{to}.hinina-partial` 再改名**，而不是直接写 `to`：中途失败
/// （磁盘满 / 文件被锁）会留下半拷贝的目标，而 [`migrate`] 用 `dst.exists()` 判
/// 「已完成」—— 半拷贝会被误判成「已存在 → 跳过」，于是重试永远不再搬它、
/// 失败计数为零 → 迁移标记被清除 → **活跃数据目录永久残缺**。
/// 落到暂存名后改名，使 `to` 的存在性等价于「搬运完整完成」。
fn move_entry(from: &Path, to: &Path) -> std::io::Result<()> {
    // 暂存与目标同目录，保证改名是同文件系统内的原子操作
    let staging = staging_path(to);
    // 先清掉上次失败可能残留的暂存。**必须在快速路径之前**：`rename` 成功时不会
    // 走到复制分支，残片就永远留着（并可能被后续启动误当成数据）。
    remove_any(&staging);

    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }

    if let Err(e) = copy_recursive(from, &staging) {
        // 失败必须清掉半拷贝 —— 留着就是下一次的「已存在」
        remove_any(&staging);
        return Err(e);
    }
    if let Err(e) = std::fs::rename(&staging, to) {
        remove_any(&staging);
        return Err(e);
    }

    // 复制成功后删除源；删不掉只告警（数据已安全落到新目录，残留旧文件不影响正确性）
    remove_source(from);
    Ok(())
}

/// 半拷贝暂存路径（与目标同目录）。
fn staging_path(to: &Path) -> PathBuf {
    let name = to
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    to.with_file_name(format!("{}.hinina-partial", name))
}

/// 删除文件或目录（不存在时 no-op；失败只告警 —— 清理失败不该中断迁移流程）。
fn remove_any(path: &Path) {
    if path.is_dir() {
        if let Err(e) = std::fs::remove_dir_all(path) {
            warn!(path = %path.display(), error = %e, "清理暂存目录失败");
        }
    } else if path.exists() {
        if let Err(e) = std::fs::remove_file(path) {
            warn!(path = %path.display(), error = %e, "清理暂存文件失败");
        }
    }
}

/// 复制完成后删除源（失败只告警：数据已在新目录，残留旧文件不影响正确性）。
fn remove_source(from: &Path) {
    if from.is_dir() {
        if let Err(e) = std::fs::remove_dir_all(from) {
            warn!(path = %from.display(), error = %e, "复制完成但旧条目删除失败（数据已在新目录）");
        }
    } else if let Err(e) = std::fs::remove_file(from) {
        warn!(path = %from.display(), error = %e, "复制完成但旧文件删除失败（数据已在新目录）");
    }
}

/// 递归复制文件或目录。
fn copy_recursive(from: &Path, to: &Path) -> std::io::Result<()> {
    if from.is_dir() {
        std::fs::create_dir_all(to)?;
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            copy_recursive(&entry.path(), &to.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(from, to).map(|_| ())
    }
}

#[cfg(test)]
#[path = "tests/data_dir_tests.rs"]
mod tests;
