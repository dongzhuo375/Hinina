use serde::{Deserialize, Serialize};

/// 应用全局配置。
///
/// 以 JSON 格式持久化，ConfigService 负责加载/保存。
/// 所有可变行为参数均从 Config 读取，支持运行时热更新。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    /// 用户偏好
    #[serde(default)]
    pub user: UserConfig,
    /// OJ 连接配置
    #[serde(default)]
    pub oj: OjConfig,
    /// 编辑器偏好
    #[serde(default)]
    pub editor: EditorConfig,
    /// 主题与外观
    #[serde(default)]
    pub theme: ThemeConfig,
    /// 布局偏好
    #[serde(default)]
    pub layout: LayoutConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            user: UserConfig::default(),
            oj: OjConfig::default(),
            editor: EditorConfig::default(),
            theme: ThemeConfig::default(),
            layout: LayoutConfig::default(),
        }
    }
}

// ── 用户偏好 ──

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserConfig {
    /// 上次登录的 OJ 类型
    #[serde(default = "default_oj_type")]
    pub last_oj_type: String,
    /// 登录用户名（用于自动填充）
    #[serde(default)]
    pub last_username: String,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            last_oj_type: default_oj_type(),
            last_username: String::new(),
        }
    }
}

fn default_oj_type() -> String {
    "HOJ".into()
}

// ── OJ 连接配置 ──

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OjConfig {
    /// HOJ 服务端地址
    #[serde(default = "default_hoj_url")]
    pub hoj_url: String,
    /// 请求超时（秒）
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    /// 评测轮询间隔（秒）
    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: u64,
    /// 评测最大等待时间（秒）
    #[serde(default = "default_poll_timeout")]
    pub poll_timeout_secs: u64,
    /// 比赛列表缓存 TTL（秒）
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl_secs: u64,
    /// 默认加载的比赛 ID（阶段 7：单比赛模式，从配置读取）。
    /// 设为 0 表示不自动加载。
    #[serde(default)]
    pub contest_id: i64,
    /// 比赛密码（私有赛需要），公开赛留空。
    #[serde(default)]
    pub contest_password: Option<String>,
}

impl Default for OjConfig {
    fn default() -> Self {
        Self {
            hoj_url: default_hoj_url(),
            timeout_secs: default_timeout(),
            poll_interval_secs: default_poll_interval(),
            poll_timeout_secs: default_poll_timeout(),
            cache_ttl_secs: default_cache_ttl(),
            contest_id: 0,
            contest_password: None,
        }
    }
}

fn default_hoj_url() -> String {
    "https://hoj.dongzhuo.top".into()
}
const fn default_timeout() -> u64 {
    30
}
const fn default_poll_interval() -> u64 {
    2
}
const fn default_poll_timeout() -> u64 {
    300
}
const fn default_cache_ttl() -> u64 {
    60
}

// ── 编辑器配置 ──

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorConfig {
    /// 字体大小
    #[serde(default = "default_font_size")]
    pub font_size: u32,
    /// Tab 宽度
    #[serde(default = "default_tab_size")]
    pub tab_size: u32,
    /// 是否自动保存（不启用则退化为手动保存）
    #[serde(default = "default_auto_save")]
    pub auto_save: bool,
    /// 自动保存间隔（秒），仅 auto_save 为 true 时生效
    #[serde(default = "default_auto_save_interval")]
    pub auto_save_interval_secs: u64,
    /// 默认编程语言（值域 = HOJ 语言显示名，与提交契约一致，如 "C++"；
    /// Monaco 高亮 id 由前端派生）
    #[serde(default = "default_language")]
    pub default_language: String,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            font_size: default_font_size(),
            tab_size: default_tab_size(),
            auto_save: default_auto_save(),
            auto_save_interval_secs: default_auto_save_interval(),
            default_language: default_language(),
        }
    }
}

const fn default_font_size() -> u32 {
    14
}
const fn default_tab_size() -> u32 {
    4
}
const fn default_auto_save() -> bool {
    true
}
const fn default_auto_save_interval() -> u64 {
    30
}
fn default_language() -> String {
    "C++".into()
}

// ── 主题配置 ──

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeConfig {
    /// 当前主题名称（取值域：light；dark 尚未实现，加载时会被归一为 light）
    #[serde(default = "default_theme_name")]
    pub theme_name: String,
    /// 编辑器主题（Monaco 主题名，取值域：vs；vs-dark 随 dark 主题一并实现）
    #[serde(default = "default_editor_theme")]
    pub editor_theme: String,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            theme_name: default_theme_name(),
            editor_theme: default_editor_theme(),
        }
    }
}

fn default_theme_name() -> String {
    "light".into()
}
fn default_editor_theme() -> String {
    "vs".into()
}

// ── 布局配置 ──

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutConfig {
    /// 侧边栏宽度（像素）
    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: u32,
    /// 题面与编辑器分栏比例（0.0 ~ 1.0，0.5 表示各占一半；默认 0.48）
    #[serde(default = "default_split_ratio")]
    pub split_ratio: f64,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            sidebar_width: default_sidebar_width(),
            split_ratio: default_split_ratio(),
        }
    }
}

const fn default_sidebar_width() -> u32 {
    280
}
const fn default_split_ratio() -> f64 {
    0.48
}

// ── 旧值归一（P55）──

/// 旧版 Monaco language id → HOJ 语言显示名（大小写不敏感）。
fn normalize_language_display_name(raw: &str) -> Option<&'static str> {
    match raw.to_ascii_lowercase().as_str() {
        "cpp" => Some("C++"),
        "c" => Some("C"),
        "java" => Some("Java"),
        "python" => Some("Python"),
        _ => None,
    }
}

/// 一次性归一历史配置文件中的旧默认值（加载路径调用，纯函数便于测试）。
///
/// 背景：`editor.defaultLanguage` 的值域已翻转为 HOJ 语言显示名（与提交契约一致）——
/// 被上一版归一成 Monaco id（'cpp' 等）的配置在此映射回显示名；
/// 已是显示名或其他非空值（OJ 可能提供 Go/Rust 等）原样保留，仅空串回退默认 "C++"。
/// 另外 `theme = dark/vs-dark`（深色主题尚未实现，前端只有浅色）、
/// `splitRatio = 0.45`（旧默认，现默认 0.48）仍按旧规则修正。
/// 只修正**恰好等于旧默认值**的项，用户显式设置的其他值一律不动。
pub fn normalize_legacy_values(cfg: &mut AppConfig) {
    if cfg.editor.default_language.is_empty() {
        cfg.editor.default_language = default_language();
    } else if let Some(name) = normalize_language_display_name(&cfg.editor.default_language) {
        cfg.editor.default_language = name.to_string();
    }

    // dark 主题未实现：归一到浅色，避免前端拿到不存在的主题名
    if cfg.theme.theme_name == "dark" {
        cfg.theme.theme_name = "light".into();
    }
    if cfg.theme.editor_theme == "vs-dark" {
        cfg.theme.editor_theme = "vs".into();
    }

    // 0.45 是旧版默认值；用户手动调出的其他比例（含恰好 0.45 之外的任意值）不受影响。
    // 浮点精确比较是刻意的：只有原样落盘的旧默认值才会二进制相等
    if cfg.layout.split_ratio == 0.45 {
        cfg.layout.split_ratio = default_split_ratio();
    }
}

// ── 写入路径校验与净化（M4）──
//
// config.json 可被手改，前端 SettingsView 不是唯一防线：
// `update_config` 持久化前必须先 `validate()`（不可钳制项拒绝）再
// `sanitize()`（可钳制项收敛到与前端一致的取值域）。

impl AppConfig {
    /// 校验不可钳制的字段（目前仅服务器地址），失败返回可直接展示的错误消息。
    ///
    /// 判据与前端 SettingsView 一致：trim 后以 http:// 或 https:// 开头
    /// （大小写不敏感），且其余部分非空、不含空白（等价 `/^https?:\/\/\S+$/i`）。
    pub fn validate(&self) -> Result<(), String> {
        if !is_valid_http_url(&self.oj.hoj_url) {
            return Err("服务器地址必须以 http:// 或 https:// 开头".into());
        }
        Ok(())
    }

    /// 就地钳制越界字段，取值域与前端 SettingsView 校验一致。
    ///
    /// 返回是否修改了任何字段（调用方据此记 warn 日志）。
    pub fn sanitize(&mut self) -> bool {
        let before = self.clone();

        self.oj.hoj_url = self.oj.hoj_url.trim().to_string();
        self.oj.timeout_secs = self.oj.timeout_secs.clamp(1, 120);
        self.oj.poll_interval_secs = self.oj.poll_interval_secs.clamp(1, 30);
        self.oj.poll_timeout_secs = self.oj.poll_timeout_secs.clamp(30, 3600);
        self.oj.cache_ttl_secs = self.oj.cache_ttl_secs.clamp(0, 600);
        self.oj.contest_id = self.oj.contest_id.max(0);
        self.editor.font_size = self.editor.font_size.clamp(8, 32);
        self.editor.tab_size = self.editor.tab_size.clamp(1, 8);
        self.editor.auto_save_interval_secs = self.editor.auto_save_interval_secs.clamp(5, 300);
        self.editor.default_language = sanitize_language_id(&self.editor.default_language);
        // NaN.clamp 返回 NaN：非有限值先回退默认，再钳制到滑杆值域 [0.30, 0.70]
        if !self.layout.split_ratio.is_finite() {
            self.layout.split_ratio = default_split_ratio();
        }
        self.layout.split_ratio = self.layout.split_ratio.clamp(0.30, 0.70);

        *self != before
    }
}

/// 是否为合法 http(s) 地址（判据见 `AppConfig::validate` 文档）。
fn is_valid_http_url(raw: &str) -> bool {
    let url = raw.trim();
    let lower = url.to_ascii_lowercase();
    let rest = if let Some(r) = lower.strip_prefix("https://") {
        r
    } else if let Some(r) = lower.strip_prefix("http://") {
        r
    } else {
        return false;
    };
    !rest.is_empty() && !rest.chars().any(|c| c.is_whitespace())
}

/// 语言净化：旧 Monaco id 先经 `normalize_language_display_name` 映射为显示名，
/// 其余非空值（含 OJ 可能提供的 Go/Rust 等）原样保留，仅空串回退默认 "C++"。
fn sanitize_language_id(raw: &str) -> String {
    if let Some(name) = normalize_language_display_name(raw) {
        return name.to_string();
    }
    if raw.is_empty() {
        default_language()
    } else {
        raw.to_string()
    }
}

#[cfg(test)]
#[path = "tests/config_tests.rs"]
mod tests;
