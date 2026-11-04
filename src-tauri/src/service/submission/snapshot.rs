// 提交源码快照：把「我当时提交的那份代码」留在本地。
//
// **动机**：详情页的代码来自 OJ，而 OJ 在很多情形下不回吐代码 —— 比赛隐藏本人
// 记录、题目 `codeShare=false`、比赛结束后回收。更常见的是**提交本身失败**：
// 那时根本没有 submission_id，服务端一行记录都没有，选手回头想看「我刚才交的是
// 什么」就彻底无从查起（实测 HOJ 提交失败时返回 HTTP 500，服务端不留痕）。
//
// 故提交成功后把源码落盘一份：这是**本地事实**，不依赖服务端是否愿意回吐。
// 详情页在 OJ 未返回代码时回落到它（见 `SubmissionService::get_submission_detail`）。
//
// **落盘失败绝不阻断提交**：快照是便利特性，丢一份快照远比丢一次提交轻。
//
// 路径：`submissions/{oj_id}/{submit_id}.{ext}`。带 OJ 维度是因为 submit_id 是
// 各 OJ 自增的资源号，跨 OJ 必然重号（与 `SubmissionService::cache_key` 同款约定）。

use crate::core::error::{AppError, AppResult};
use crate::infra::storage::Storage;

/// 快照目录（相对存储根）。
const SNAPSHOT_DIR: &str = "submissions";

/// 未知语言的扩展名回退值。
///
/// 刻意不猜 `.cpp`：判题端按后缀判语言，猜错等于让人误以为交的是另一种语言。
/// 与前端 `utils/language.sourceFileNameOf` 的 `main.txt` 回退同源。
const FALLBACK_EXT: &str = "txt";

/// `s` 是否以**独立词** `word` 开头（`word` 后不能再跟字母/数字/下划线）。
///
/// 用于规避前缀吞并：`"c#"` 不能被 `"c"` 命中、`"javascript"` 不能被 `"java"` 命中。
fn starts_with_word(s: &str, word: &str) -> bool {
    match s.strip_prefix(word) {
        Some(rest) => rest
            .chars()
            .next()
            .map(|c| !c.is_alphanumeric() && c != '_')
            .unwrap_or(true),
        None => false,
    }
}

/// HOJ 语言显示名 → 语言族 id（无法识别返回 `None`）。
///
/// 与前端 `utils/language.ts` 的 `monacoIdStrict` 保持同一识别面：服务端/部署
/// 差异会带版本后缀（`"C++17 (GCC 13.2)"`、`"Python 3.10"`、`"PyPy3"`、`"Golang"`），
/// 故绝大多数判据是**前缀**匹配。判定顺序不可随意调换：
/// C++/C# 必须先于 C，JavaScript 必须先于 Java。
fn language_family(language: &str) -> Option<&'static str> {
    let s = language.trim().to_ascii_lowercase();
    if s.is_empty() {
        return None;
    }
    // C++ 族必须先于 C 判定（"c++" 的 'c' 之后是 '+'）
    if s.starts_with("c++") || s.starts_with("cpp") || s.starts_with("cxx") || s.starts_with("g++")
    {
        return Some("cpp");
    }
    // C# 族必须先于 C 判定（同上，"c#" 的 'c' 之后是 '#'）
    if s.starts_with("c#") || s.starts_with("csharp") {
        return Some("csharp");
    }
    // C 用**词边界**判定（与前端 `/^c\b/` 对齐）：C 的部署变体写作 "C" / "C With O2"，
    // 而 "cpp"/"csharp" 已在上面被截走
    if starts_with_word(&s, "c") {
        return Some("c");
    }
    // JavaScript 先于 Java（"javascript" 同样以 "java" 开头）
    if s.starts_with("javascript") || s.starts_with("js") {
        return Some("javascript");
    }
    if s.starts_with("typescript") || s.starts_with("ts") {
        return Some("typescript");
    }
    if s.starts_with("java") {
        return Some("java");
    }
    if s.starts_with("kotlin") {
        return Some("kotlin");
    }
    // "py" 前缀已覆盖 "python" / "pypy"
    if s.starts_with("python") || s.starts_with("py") {
        return Some("python");
    }
    // "go" 前缀覆盖 "golang"（部署变体的常见写法）
    if s.starts_with("go") {
        return Some("go");
    }
    if s.starts_with("rust") {
        return Some("rust");
    }
    if s.starts_with("php") {
        return Some("php");
    }
    if s.starts_with("ruby") {
        return Some("ruby");
    }
    if s.starts_with("sql") {
        return Some("sql");
    }
    None
}

/// HOJ 语言显示名 → 源文件扩展名（不含点号）。
///
/// 与前端 `sourceFileNameOf` 的后缀一致（`main.cpp` → `cpp`、`Main.java` → `java`…），
/// 未知语言回退 `txt`。
pub fn source_extension(language: &str) -> &'static str {
    match language_family(language) {
        Some("c") => "c",
        Some("cpp") => "cpp",
        Some("java") => "java",
        Some("kotlin") => "kt",
        Some("python") => "py",
        Some("javascript") => "js",
        Some("typescript") => "ts",
        Some("go") => "go",
        Some("rust") => "rs",
        Some("csharp") => "cs",
        Some("php") => "php",
        Some("ruby") => "rb",
        Some("sql") => "sql",
        _ => FALLBACK_EXT,
    }
}

/// 快照目录（`submissions/{oj_id}`），已做路径字符校验。
fn snapshot_dir(oj_id: &str) -> AppResult<String> {
    let oj = sanitize_path_part(oj_id, "oj_id")?;
    Ok(format!("{}/{}", SNAPSHOT_DIR, oj))
}

/// 拒绝路径分隔符与 `..`，防止写出存储根之外（与 `ContestService::read_state_path` 同款）。
fn sanitize_path_part(part: &str, name: &str) -> AppResult<String> {
    if part.is_empty() || part.contains(['/', '\\', ':']) || part.contains("..") {
        return Err(AppError::Io(format!("{} 含非法路径字符: {}", name, part)));
    }
    Ok(part.to_string())
}

/// 精确路径：`submissions/{oj_id}/{submit_id}.{ext}`。
pub fn snapshot_path(oj_id: &str, submit_id: &str, language: &str) -> AppResult<String> {
    let dir = snapshot_dir(oj_id)?;
    let id = sanitize_path_part(submit_id, "submit_id")?;
    Ok(format!("{}/{}.{}", dir, id, source_extension(language)))
}

/// 落盘一份源码快照（best-effort：失败只记录，不向上传播）。
///
/// `Storage::write_string` 自带父目录创建，故无需先建目录。
pub fn write_snapshot(
    storage: &Storage,
    oj_id: &str,
    submit_id: &str,
    language: &str,
    source_code: &str,
) {
    let result = snapshot_path(oj_id, submit_id, language)
        .and_then(|path| storage.write_string(&path, source_code));

    match result {
        Ok(()) => tracing::debug!(submit_id, oj_id, "提交源码快照已保存"),
        Err(e) => tracing::warn!(
            submit_id,
            oj_id,
            error = %e,
            "提交源码快照保存失败（不影响提交本身）"
        ),
    }
}

/// 读取源码快照。
///
/// 先按 `language` 推出的扩展名精确取；未命中时**回退扫描**该 OJ 的快照目录，
/// 找 `{submit_id}.*` —— 提交时与查询时的语言写法可能不同（服务端归一、
/// 选手改语言），精确路径会因此落空，而快照明明就在那儿。
pub fn read_snapshot(
    storage: &Storage,
    oj_id: &str,
    submit_id: &str,
    language: &str,
) -> Option<String> {
    if let Ok(path) = snapshot_path(oj_id, submit_id, language) {
        if let Some(content) = read_real_file(storage, &path) {
            return Some(content);
        }
    }

    let dir = snapshot_dir(oj_id).ok()?;
    let prefix = format!("{}.", submit_id);
    let entries = storage.list(&dir).ok()?;
    for entry in entries {
        let name = entry.file_name()?.to_string_lossy().to_string();
        if name.starts_with(&prefix) {
            let relative = entry.to_string_lossy().replace('\\', "/");
            if let Some(content) = read_real_file(storage, &relative) {
                return Some(content);
            }
        }
    }
    None
}

// ── 留档占用统计与过期清理（设置页「清理本地数据」） ──
//
// 「过期」的判据是**文件修改时间**早于保留窗口，而不是「服务端已查不到该提交」：
// 后者要拉服务端提交列表逐一比对（要网络、要分页、还可能因赛制隐藏记录而误判），
// 而留档的价值本来就随时间衰减，按时间保留既简单又不会误删近期记录。

/// 快照留档的保留窗口（天）：更早的留档视为「过期」，可由设置页清理。
///
/// 取值理由：赛后复盘通常在几天内完成，而单条留档只有百字节量级 —— 30 天是
/// 「基本不会误删还想看的东西」与「不让目录无限累积」之间的折中。
pub const SNAPSHOT_KEEP_DAYS: u64 = 30;

/// 快照留档占用统计。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SnapshotUsage {
    /// 留档总条数
    pub total_count: usize,
    /// 留档总字节数
    pub total_bytes: u64,
    /// 其中「过期」（早于保留窗口）的条数
    pub stale_count: usize,
    /// 其中「过期」的字节数
    pub stale_bytes: u64,
}

/// 遍历全部留档，返回 `(相对路径, 字节数, 修改时间)`。
///
/// 目录结构固定两层（`submissions/{oj_id}/{submit_id}.{ext}`），故不引入递归。
/// 读不到元数据的条目直接跳过：留档是便利特性，统计失败不该让清理流程报错。
///
/// **不跟随符号链接**：`Storage::resolve` 只做词法校验；若 `submissions/`、
/// `{oj_id}/` 或留档文件本身是指向外部的链接，跟随会把外部文件计入统计，
/// 清理阶段还会穿过目录链接删到存储根之外。三个层级一律用 `symlink_metadata`
/// 并要求真实目录/文件，链接条目直接跳过。
fn walk_snapshots(storage: &Storage) -> WalkResult {
    let mut result = WalkResult::default();
    if !is_real_dir(storage, SNAPSHOT_DIR) {
        return result;
    }
    let Ok(entries) = storage.list(SNAPSHOT_DIR) else {
        return result;
    };
    for entry in entries {
        let relative_dir = entry.to_string_lossy().replace('\\', "/");
        if !is_real_dir(storage, &relative_dir) {
            continue;
        }
        result.oj_dirs.push(relative_dir.clone());
        let Ok(files) = storage.list(&relative_dir) else {
            continue;
        };
        for file in files {
            let relative = file.to_string_lossy().replace('\\', "/");
            let Ok(meta) = storage.symlink_metadata(&relative) else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
            let Ok(modified) = meta.modified() else {
                continue;
            };
            result.files.push((relative, meta.len(), modified));
        }
    }
    result
}

/// 遍历结果：留档文件 + 已确认的**真实** OJ 子目录（回收空目录时复用，
/// 免去第二次 `list(submissions/)`；删除前仍逐条 lstat 作纵深防御）。
#[derive(Debug, Default)]
struct WalkResult {
    files: Vec<(String, u64, std::time::SystemTime)>,
    oj_dirs: Vec<String>,
}

/// 条目是否为**真实目录**（lstat，不跟随链接）。
///
/// `symlink_metadata` 不跟随链接：符号链接与 Windows junction 的 `is_dir()`
/// 均为 false（junction 实测 `is_symlink() == true`），因此本函数天然把链接
/// 挡在统计与清理之外。`submissions/` 根、OJ 子目录、回收空目录三处共用。
fn is_real_dir(storage: &Storage, relative: &str) -> bool {
    storage
        .symlink_metadata(relative)
        .is_ok_and(|meta| meta.is_dir())
}

/// 读取**真实文件**的内容（lstat 不跟随链接）。
///
/// 与遍历/清理同一判据：`submissions/{oj}/{id}.cpp` 若是指向数据目录外的
/// 链接，直接 `read_to_string` 会把外部文件内容当提交代码回给详情页。读取前
/// 先 lstat 并要求 `is_file()`（链接的 `is_file()` 为 false）。残余窗口同
/// 遍历/清理：lstat 与读取之间的本地并发替换，见 `snapshot.md` 设计要点。
fn read_real_file(storage: &Storage, relative: &str) -> Option<String> {
    let meta = storage.symlink_metadata(relative).ok()?;
    if !meta.is_file() {
        return None;
    }
    storage.read_to_string(relative).ok()
}

/// 统计留档占用（含过期部分），供清理前的预览展示。
pub fn inspect_snapshots(storage: &Storage, keep_days: u64) -> SnapshotUsage {
    let cutoff = stale_cutoff(keep_days);
    let mut usage = SnapshotUsage::default();
    for (_, bytes, modified) in walk_snapshots(storage).files {
        usage.total_count += 1;
        usage.total_bytes += bytes;
        if modified < cutoff {
            usage.stale_count += 1;
            usage.stale_bytes += bytes;
        }
    }
    usage
}

/// 删除过期留档，返回 `(删除条数, 释放字节数)`。
///
/// 删空的 OJ 子目录一并回收（否则清理后只剩一堆空目录）。删除失败逐条忽略并告警：
/// 清理是尽力而为的维护动作，不该因为一个文件被占用而整体失败。
pub fn purge_stale_snapshots(storage: &Storage, keep_days: u64) -> (usize, u64) {
    let cutoff = stale_cutoff(keep_days);
    let mut removed = 0usize;
    let mut freed = 0u64;

    let walk = walk_snapshots(storage);
    for (relative, bytes, modified) in walk.files {
        if modified >= cutoff {
            continue;
        }
        // 文件专用删除（不跟随链接）：即便条目在 lstat 与删除之间被换成目录
        // 或链接，也不会删目录、不会穿过链接——Unix 上最多删掉链接本身，
        // Windows 上目录链接直接失败，外部目标始终不受影响。
        match storage.remove_file(&relative) {
            Ok(()) => {
                removed += 1;
                freed += bytes;
            }
            Err(e) => tracing::warn!(path = %relative, error = %e, "删除过期留档失败"),
        }
    }

    if removed > 0 {
        // 复用 walk 已枚举并确认过「真实目录」的 OJ 子目录（不再重复
        // `list(submissions/)`）；删除前仍逐条 lstat 作纵深防御。
        for relative in walk.oj_dirs {
            if !is_real_dir(storage, &relative) {
                continue;
            }
            // `remove_empty_dir` 只做 `fs::remove_dir`：目录专用、不跟随链接
            //（Unix 对链接返回 ENOTDIR；Windows 只删链接条目本身、不碰目标），
            // 且非空目录必然失败 —— 正好用来「只回收空目录」。不用 `remove`：
            // 它会用 `path.is_dir()`（跟随链接）分流，可能删文件/删链接。
            if storage.remove_empty_dir(&relative).is_ok() {
                tracing::debug!(dir = %relative, "留档子目录已空，已回收");
            }
        }
        tracing::info!(
            removed,
            freed_bytes = freed,
            keep_days,
            "已清理过期提交留档"
        );
    }
    (removed, freed)
}

/// 过期判据的截止时刻：`now - keep_days`。
fn stale_cutoff(keep_days: u64) -> std::time::SystemTime {
    std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(keep_days * 24 * 60 * 60))
        .unwrap_or(std::time::UNIX_EPOCH)
}

#[cfg(test)]
#[path = "tests/snapshot_tests.rs"]
mod tests;
