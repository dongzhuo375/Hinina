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
pub fn snapshot_path(
    oj_id: &str,
    submit_id: &str,
    language: &str,
) -> AppResult<String> {
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
        if let Ok(content) = storage.read_to_string(&path) {
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
            if let Ok(content) = storage.read_to_string(&relative) {
                return Some(content);
            }
        }
    }
    None
}

#[cfg(test)]
#[path = "tests/snapshot_tests.rs"]
mod tests;
