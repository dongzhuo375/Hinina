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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserConfig {
    /// 登录用户名（用于自动填充）
    #[serde(default)]
    pub last_username: String,
    /// 旧版「上次登录的 OJ 类型」，已迁移至 `oj.active`（OJ 选择是应用级状态，
    /// 决定全部 Provider 行为，不属于用户偏好）。`skip_serializing` 保证
    /// 读取迁移后不再写回（见 `normalize_legacy_values`）。
    #[serde(default, skip_serializing, rename = "lastOjType")]
    pub legacy_last_oj_type: Option<String>,
}

// ── OJ 连接配置 ──

/// 单个 OJ 实例的连接配置。
///
/// 「配置了哪些 OJ」是数据：接一个新 OJ = 配置里加一条实例 + 适配器工厂清单
/// 加一行，**不需要**给 `OjConfig` 加字段（旧版 `hoj_url` 那种以具体 OJ 命名
/// 的字段，每接一个 OJ 就要加一条 + 三处校验分支）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OjInstance {
    /// OJ 身份（须与适配器工厂 `id()` 一致，如 "HOJ"）
    #[serde(default)]
    pub id: String,
    /// 服务端地址（站点根，如 `https://hoj.dongzhuo.top`，不带 `/api` 等前缀）
    #[serde(default)]
    pub base_url: String,
    /// 是否启用（false = 组合根不注册该实例的 Provider）
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// OJ 私有旋钮。刻意弱类型：强类型枚举会让「新 OJ 要改 core」原样复活。
    #[serde(default)]
    pub options: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OjConfig {
    /// 当前 OJ 实例 id（取代旧 `user.lastOjType`：OJ 选择是应用级状态）
    #[serde(default = "default_active_oj")]
    pub active: String,
    /// 已配置的 OJ 实例列表。
    ///
    /// serde 缺省为**空表**而非默认清单：旧格式文件（无 instances 键）须落到
    /// `normalize_legacy_values` 的迁移分支（legacy hojUrl → 实例），而非被
    /// 字段级默认直接填成默认地址 —— 那会静默丢掉用户自定义的旧地址。
    /// 全新配置（无文件）走 `Default`，那里才是默认 HOJ 实例。
    #[serde(default)]
    pub instances: Vec<OjInstance>,
    /// 当前比赛引用 —— **不透明字符串**：HOJ 是数字串，Hydro 是 hex ObjectId。
    /// 「当前比赛」本质上是对服务端资源的引用，不该假设它是数字（旧
    /// `contest_id: i64` 装不下非数字 ID）。空串 = 未配置，不自动加载。
    #[serde(default)]
    pub contest_ref: String,
    /// 比赛密码（私有赛需要），公开赛留空。
    #[serde(default)]
    pub contest_password: Option<String>,
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
    /// 是否启用题面缓存（内存 + 磁盘，TTL 见 `service::problem::PROBLEM_CACHE_TTL`）。
    ///
    /// 默认开启：题面在比赛期间基本不变，缓存可省掉「切题来回/重进应用」的重复请求，
    /// 并在断网时仍可打开已缓存的题面。关闭后每次打开题目都直连服务端
    /// （题面被管理员中途修正时想立刻看真值，可临时关闭）。
    #[serde(default = "default_cache_problem_statement")]
    pub cache_problem_statement: bool,

    // ── 旧版字段（仅读取用于迁移，skip_serializing 保证永不写回）──
    /// 旧版 HOJ 专属地址 → 迁移为 `instances` 中 HOJ 实例的 `base_url`
    #[serde(default, skip_serializing, rename = "hojUrl")]
    pub legacy_hoj_url: Option<String>,
    /// 旧版数字比赛 ID → 迁移为 `contest_ref` 字符串
    #[serde(default, skip_serializing, rename = "contestId")]
    pub legacy_contest_id: Option<i64>,
}

impl Default for OjConfig {
    fn default() -> Self {
        Self {
            active: default_active_oj(),
            instances: default_oj_instances(),
            contest_ref: String::new(),
            contest_password: None,
            timeout_secs: default_timeout(),
            poll_interval_secs: default_poll_interval(),
            poll_timeout_secs: default_poll_timeout(),
            cache_ttl_secs: default_cache_ttl(),
            cache_problem_statement: default_cache_problem_statement(),
            legacy_hoj_url: None,
            legacy_contest_id: None,
        }
    }
}

/// 默认当前 OJ：HOJ（与旧版 `default_oj_type` 一致，行为零变化）
fn default_active_oj() -> String {
    "HOJ".into()
}

/// 默认实例清单：单 HOJ 实例（沿用旧版默认地址）
fn default_oj_instances() -> Vec<OjInstance> {
    vec![OjInstance {
        id: default_active_oj(),
        base_url: default_hoj_url(),
        enabled: true,
        options: serde_json::Map::new(),
    }]
}

/// OJ 实例默认启用
const fn default_true() -> bool {
    true
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
/// 题面缓存默认开启（关掉只影响性能，不影响正确性）
const fn default_cache_problem_statement() -> bool {
    true
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
    /// 编辑器主题（Monaco 主题名，取值域：vs / vs-dark；由解题页编辑器设置切换，
    /// **只作用于编辑器区域** —— 客户端界面仍只有浅色）
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
/// 另外 `theme = dark`（深色 UI 尚未实现，前端只有浅色）、
/// `splitRatio = 0.45`（旧默认，现默认 0.48）仍按旧规则修正。
/// 只修正**恰好等于旧默认值**的项，用户显式设置的其他值一律不动。
pub fn normalize_legacy_values(cfg: &mut AppConfig) {
    if cfg.editor.default_language.is_empty() {
        cfg.editor.default_language = default_language();
    } else if let Some(name) = normalize_language_display_name(&cfg.editor.default_language) {
        cfg.editor.default_language = name.to_string();
    }

    // dark 主题未实现：整机主题归一到浅色，并把它一并写入的编辑器主题也归位。
    // **仅在 theme_name == "dark" 时**才动 editor_theme —— 编辑器主题已是用户可选
    // 项（解题页编辑器设置），`theme_name = light` + `editor_theme = vs-dark`
    // 是合法组合，无条件重置会把选手刚选的深色编辑器悄悄改回浅色。
    if cfg.theme.theme_name == "dark" {
        cfg.theme.theme_name = default_theme_name();
        if cfg.theme.editor_theme == "vs-dark" {
            cfg.theme.editor_theme = default_editor_theme();
        }
    }

    // ── OJ 配置 v2 迁移（hojUrl/contestId/lastOjType → instances/contestRef/active）──
    //
    // 旧字段经 serde 过渡字段读入（skip_serializing：迁移后不再写回）；
    // 磁盘文件在下一次写配置时自然收敛到新格式（与 splitRatio 归一同一模式）。
    if cfg.oj.instances.is_empty() {
        // 旧版单 OJ 配置（或缺失）：以旧 hojUrl（缺省用默认地址）建立 HOJ 实例
        let base_url = cfg
            .oj
            .legacy_hoj_url
            .clone()
            .unwrap_or_else(default_hoj_url);
        cfg.oj.instances = vec![OjInstance {
            id: default_active_oj(),
            base_url,
            enabled: true,
            options: serde_json::Map::new(),
        }];
    }
    if let Some(t) = cfg.user.legacy_last_oj_type.take() {
        let t = t.trim();
        if !t.is_empty() {
            cfg.oj.active = t.to_string();
        }
    }
    if cfg.oj.contest_ref.is_empty() {
        // 旧 contestId == 0 表示未配置，不迁移
        if let Some(id) = cfg.oj.legacy_contest_id.filter(|v| *v > 0) {
            cfg.oj.contest_ref = id.to_string();
        }
    }
    cfg.oj.contest_ref = cfg.oj.contest_ref.trim().to_string();
    // active 必须指向已配置实例，不指向时回退 HOJ（与组合根的注册防线同语义）
    if !cfg.oj.instances.iter().any(|i| i.id == cfg.oj.active) {
        cfg.oj.active = default_active_oj();
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
        // 实例 id 唯一（重复 id 会让注册表互相覆盖、会话文件名撞车）
        let mut seen = std::collections::HashSet::new();
        for instance in &self.oj.instances {
            if instance.id.trim().is_empty() {
                return Err("OJ 实例 id 不能为空".into());
            }
            if !seen.insert(instance.id.trim().to_string()) {
                return Err(format!("OJ 实例 id 重复: {}", instance.id));
            }
        }
        // 启用实例的地址必须合法（禁用实例不注册，地址可为占位）
        for instance in self.oj.instances.iter().filter(|i| i.enabled) {
            if !is_valid_http_url(&instance.base_url) {
                return Err(format!(
                    "{} 的服务器地址必须以 http:// 或 https:// 开头",
                    instance.id
                ));
            }
        }
        // active 必须指向已配置实例
        if !self.oj.instances.iter().any(|i| i.id == self.oj.active) {
            return Err(format!("当前 OJ（{}）不在已配置实例列表中", self.oj.active));
        }
        Ok(())
    }

    /// 就地钳制越界字段，取值域与前端 SettingsView 校验一致。
    ///
    /// 返回是否修改了任何字段（调用方据此记 warn 日志）。
    pub fn sanitize(&mut self) -> bool {
        let before = self.clone();

        self.oj.active = self.oj.active.trim().to_string();
        self.oj.contest_ref = self.oj.contest_ref.trim().to_string();
        for instance in &mut self.oj.instances {
            instance.id = instance.id.trim().to_string();
            instance.base_url = instance.base_url.trim().to_string();
        }
        self.oj.timeout_secs = self.oj.timeout_secs.clamp(1, 120);
        self.oj.poll_interval_secs = self.oj.poll_interval_secs.clamp(1, 30);
        self.oj.poll_timeout_secs = self.oj.poll_timeout_secs.clamp(30, 3600);
        self.oj.cache_ttl_secs = self.oj.cache_ttl_secs.clamp(0, 600);
        self.editor.font_size = self.editor.font_size.clamp(8, 32);
        self.editor.tab_size = self.editor.tab_size.clamp(1, 8);
        self.editor.auto_save_interval_secs = self.editor.auto_save_interval_secs.clamp(5, 300);
        self.editor.default_language = sanitize_language_id(&self.editor.default_language);
        self.theme.editor_theme = sanitize_editor_theme(&self.theme.editor_theme);
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

/// 编辑器主题净化：取值域 = Monaco 内置主题 `vs` / `vs-dark`（解题页编辑器设置可选），
/// 其余值（手改配置、未来自定义主题名）回退默认 `vs`。
///
/// 必须收敛：`monaco.editor.setTheme` 收到未知主题名会静默保留上一个主题，
/// 配置与界面就此不一致，且用户无从察觉。
fn sanitize_editor_theme(raw: &str) -> String {
    match raw {
        "vs" | "vs-dark" => raw.to_string(),
        _ => default_editor_theme(),
    }
}

#[cfg(test)]
#[path = "tests/config_tests.rs"]
mod tests;
