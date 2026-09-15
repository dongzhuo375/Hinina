# config

## 职责
定义应用全局配置的实体类型，包含用户偏好、OJ 连接、编辑器、主题、布局五类配置。所有 struct 均实现 `Serialize + Deserialize + Default`，以 JSON 格式通过 `ConfigService` 持久化和加载。所有可变行为参数均从 Config 读取，支持运行时热更新（`ConfigService::reload()`）。另提供旧值归一纯函数 `normalize_legacy_values`（P55），把历史配置中的旧默认值收敛到当前取值域。

## 核心类型

### AppConfig
应用全局配置根 struct，字段均以 `#[serde(default)]` 标注以确保向后兼容。
- `user: UserConfig` — 用户偏好
- `oj: OjConfig` — OJ 连接配置
- `editor: EditorConfig` — 编辑器偏好
- `theme: ThemeConfig` — 主题与外观
- `layout: LayoutConfig` — 布局偏好

### UserConfig
- `last_oj_type: String`（默认 `"HOJ"`）— 上次登录的 OJ 类型
- `last_username: String`（默认 `""`）— 用于自动填充登录用户名

### OjConfig
- `hoj_url: String`（默认 `"https://hoj.dongzhuo.top"`）— HOJ 服务端地址
- `timeout_secs: u64`（默认 `30`）— HTTP 请求超时
- `poll_interval_secs: u64`（默认 `2`）— 评测轮询间隔
- `poll_timeout_secs: u64`（默认 `300`）— 评测最大等待时间
- `cache_ttl_secs: u64`（默认 `60`）— 比赛列表缓存 TTL
- `contest_id: i64`（默认 `0`）— 默认加载的比赛 ID（阶段 7 单比赛模式），`0` 表示不自动加载
- `contest_password: Option<String>`（默认 `None`）— 比赛密码（私有赛需要），公开赛留空

### EditorConfig
- `font_size: u32`（默认 `14`）— 编辑器字体大小
- `tab_size: u32`（默认 `4`）— Tab 宽度
- `auto_save: bool`（默认 `true`）— 是否启用自动保存
- `auto_save_interval_secs: u64`（默认 `30`）— 自动保存间隔
- `default_language: String`（默认 `"cpp"`）— 默认编程语言（**Monaco language id**，取值域：cpp / java / python / c 等；旧版默认值曾是显示名 `"C++"`，Monaco 不识别，由 `normalize_legacy_values` 归一）

### ThemeConfig
- `theme_name: String`（默认 `"light"`）— 当前主题名称（取值域：light；dark 尚未实现，加载时会被归一为 light）
- `editor_theme: String`（默认 `"vs"`）— Monaco 编辑器主题名（取值域：vs；vs-dark 随 dark 主题一并实现）

### LayoutConfig
- `sidebar_width: u32`（默认 `280`）— 侧边栏宽度（像素）
- `split_ratio: f64`（默认 `0.48`）— 题面与编辑器分栏比例（0.0~1.0，0.5 表示各占一半；旧默认 0.45 由 `normalize_legacy_values` 归一）

## 默认值函数
所有默认值通过模块级私有函数提供（如 `default_hoj_url()`, `default_timeout()`, `default_font_size()` 等），`const fn` 用于编译期常量、`fn` 用于需要 `String` 的默认值。

## 旧值归一（P55）
- **`normalize_legacy_values(cfg: &mut AppConfig)`**（公开纯函数）— 一次性归一历史配置文件中的旧默认值，由 `ConfigService::new()` 与 `reload()` 两条加载路径调用。背景：旧版默认值写入了「显示名」而非取值域内的合法值 —— `editor.defaultLanguage = "C++"`（Monaco 只认 `cpp`）、`theme = dark/vs-dark`（深色主题尚未实现，前端只有浅色）、`splitRatio = 0.45`（旧默认，现默认 0.48）。**只修正恰好等于旧默认值的项，用户显式设置的其他值一律不动**：
  - 语言：经私有 `normalize_language_id` 把显示名映射为 Monaco id（`"C++"→"cpp"` / `"Java"→"java"` / `"Python"→"python"` / `"C"→"c"`），未知值不动
  - 主题：`theme_name == "dark"` → `"light"`；`editor_theme == "vs-dark"` → `"vs"`
  - 分栏比例：`split_ratio == 0.45` → `0.48`。浮点精确比较是刻意的：只有原样落盘的旧默认值才会二进制相等
- 抽成纯函数（而非埋在 ConfigService 里）是为了可被单元测试直接锁定，且幂等（每次加载都会执行）

## 直接依赖
- `serde::{Deserialize, Serialize}`

## 被依赖
- `service::config::ConfigService`（通过 `ConfigRepository` 加载/保存 `AppConfig`，并在 `new()` / `reload()` 加载路径调用 `normalize_legacy_values`）
- `service::theme::ThemeService`（读取/写入 `ThemeConfig`）
- `service::submission::SubmissionService`（通过 `OjConfig` 获取轮询参数）
- `service::contest::ContestService`（通过 `OjConfig::cache_ttl_secs` 控制缓存 TTL）
- `core::context`（`AppContext::init` 读取 `OjConfig::timeout_secs` 构造 HttpClient）

## 逻辑流程
- 所有子 struct 的 `Default::default()` 返回合理的默认值，`AppConfig::default()` 组合各子 struct 默认值
- `#[serde(default)]` 保证 JSON 缺少字段时回退到 `Default`，实现向后兼容的配置热升级
- 加载路径（`ConfigService::new` / `reload`）反序列化后调用 `normalize_legacy_values` 归一旧默认值，保证前端拿到的配置永远落在取值域内
- 源文件：`src-tauri/src/core/entity/config.rs`

## 测试
`src-tauri/src/core/entity/tests/config_tests.rs`（由 `config.rs` 底部 `#[cfg(test)] #[path = "tests/config_tests.rs"] mod tests;` 引用）锁定：默认值全部落在取值域内（`cpp` / `light` / `vs` / `0.48`）、四种旧语言显示名归一为 Monaco id、合法 id 与未知值（含空串）不动、dark/vs-dark 归一为 light/vs 而浅色不动、`split_ratio` 只替换恰好 0.45 的旧默认（0.44/0.46/0.5 等用户值不动）、归一幂等、旧版落盘 JSON 端到端反序列化 + 归一。
