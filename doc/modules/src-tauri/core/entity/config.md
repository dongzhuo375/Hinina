# config

## 职责
定义应用全局配置的实体类型，包含用户偏好、OJ 连接、编辑器、主题、布局五类配置。所有 struct 均实现 `Serialize + Deserialize + Default`，以 JSON 格式通过 `ConfigService` 持久化和加载。所有可变行为参数均从 Config 读取，支持运行时热更新（`ConfigService::reload()`）。另提供旧值归一纯函数 `normalize_legacy_values`（P55），把历史配置中的旧默认值收敛到当前取值域；以及写入路径的 `validate()` / `sanitize()`（M4），供 `update_config` Command 持久化前做后端兜底校验与钳制。

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
- `poll_interval_secs: u64`（默认 `2`）— 评测轮询间隔（由前端 submissionStore 的 createPoller 经 get_config 消费，后端单次查询不读取）
- `poll_timeout_secs: u64`（默认 `300`）— 评测最大等待时间（同上，前端 poller 的 deadline 判据）
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

## 写入路径校验与净化（M4）
`update_config` Command 持久化前调用（config.json 可被手改，前端 SettingsView 不是唯一防线）。均为纯函数/纯方法，可直接单测：
- **`AppConfig::validate(&self) -> Result<(), String>`** — 校验不可钳制项（目前仅 `oj.hoj_url`），失败返回可直接展示的错误消息。判据与前端一致：trim 后以 `http://` 或 `https://` 开头（大小写不敏感）且其余部分非空、不含空白（等价 `/^https?:\/\/\S+$/i`）。Command 层把 `Err(msg)` 映射为 `AppError::Config(msg)` 拒绝落盘
- **`AppConfig::sanitize(&mut self) -> bool`** — 就地钳制越界字段，取值域与前端 SettingsView 校验一致：`timeout_secs 1..=120`、`poll_interval_secs 1..=30`、`poll_timeout_secs 30..=3600`、`cache_ttl_secs 0..=600`、`contest_id >= 0`、`font_size 8..=32`、`tab_size 1..=8`、`auto_save_interval_secs 5..=300`、`split_ratio 0.30..=0.70`（非有限值先回退默认 0.48 再钳制）；`hoj_url` trim；`default_language` 经私有 `sanitize_language_id` 净化 —— 旧显示名先走 `normalize_language_id` 映射，合法 Monaco id（cpp/java/python/c）保留，其余未知值回退 `cpp`。返回是否修改了任何字段（调用方据此记 warn 日志），为此各配置 struct 追加了 `PartialEq` derive

## 直接依赖
- `serde::{Deserialize, Serialize}`

## 被依赖
- `service::config::ConfigService`（通过 `ConfigRepository` 加载/保存 `AppConfig`，并在 `new()` / `reload()` 加载路径调用 `normalize_legacy_values`）
- `commands::config_cmd`（`update_config` 持久化前调用 `validate()` + `sanitize()` 做后端兜底）
- `service::theme::ThemeService`（读取/写入 `ThemeConfig`）
- `service::contest::ContestService`（通过 `OjConfig::cache_ttl_secs` 控制缓存 TTL）
- `core::context`（`AppContext::init` 读取 `OjConfig::timeout_secs` 构造 HttpClient）

## 逻辑流程
- 所有子 struct 的 `Default::default()` 返回合理的默认值，`AppConfig::default()` 组合各子 struct 默认值
- `#[serde(default)]` 保证 JSON 缺少字段时回退到 `Default`，实现向后兼容的配置热升级
- 加载路径（`ConfigService::new` / `reload`）反序列化后调用 `normalize_legacy_values` 归一旧默认值，保证前端拿到的配置永远落在取值域内
- 源文件：`src-tauri/src/core/entity/config.rs`

## 测试
`src-tauri/src/core/entity/tests/config_tests.rs`（由 `config.rs` 底部 `#[cfg(test)] #[path = "tests/config_tests.rs"] mod tests;` 引用）锁定：默认值全部落在取值域内（`cpp` / `light` / `vs` / `0.48`）、四种旧语言显示名归一为 Monaco id、合法 id 与未知值（含空串）不动、dark/vs-dark 归一为 light/vs 而浅色不动、`split_ratio` 只替换恰好 0.45 的旧默认（0.44/0.46/0.5 等用户值不动）、归一幂等、旧版落盘 JSON 端到端反序列化 + 归一。M4 校验/净化用例：`validate` 拒绝空串 / `ftp://x` / 裸域名 / 只有 scheme 头 / 含空白地址并接受大小写混合的 http(s)（trim 后校验）、`sanitize` 钳制全部越界字段并返回 true、合法配置（默认值与边界值）原样不动且返回 false、未知语言（`rust` / 空串）回退 `cpp` 而合法 id 保留、URL trim 与 NaN 分栏比例回退默认、手改 JSON 端到端 validate + sanitize 收敛。
