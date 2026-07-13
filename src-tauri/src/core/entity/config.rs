use serde::{Deserialize, Serialize};

/// 应用全局配置。
///
/// 以 JSON 格式持久化，ConfigService 负责加载/保存。
/// 所有可变行为参数均从 Config 读取，支持运行时热更新。
#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// 默认编程语言
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    /// 当前主题名称（light / dark）
    #[serde(default = "default_theme_name")]
    pub theme_name: String,
    /// 编辑器主题（Monaco 主题名）
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
    "dark".into()
}
fn default_editor_theme() -> String {
    "vs-dark".into()
}

// ── 布局配置 ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutConfig {
    /// 侧边栏宽度（像素）
    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: u32,
    /// 题面与编辑器分栏比例（0.0 ~ 1.0，0.5 表示各占一半）
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
    0.45
}
