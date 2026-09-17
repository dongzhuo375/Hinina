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
- `cache_problem_statement: bool`（默认 `true`）— 题面缓存开关（内存 + 磁盘，TTL 见 `service::problem::PROBLEM_CACHE_TTL`）。默认开启：题面在比赛期间基本不变，缓存可省掉「切题来回/重进应用」的重复请求并在断网时仍可打开；关闭后每次打开题目都直连服务端（题面被管理员中途修正时可临时关闭）。bool 无需 `sanitize` 钳制
- `contest_id: i64`（默认 `0`）— 默认加载的比赛 ID（阶段 7 单比赛模式），`0` 表示不自动加载
- `contest_password: Option<String>`（默认 `None`）— 比赛密码（私有赛需要），公开赛留空

### EditorConfig
- `font_size: u32`（默认 `14`）— 编辑器字体大小
- `tab_size: u32`（默认 `4`）— Tab 宽度
- `auto_save: bool`（默认 `true`）— 是否启用自动保存
- `auto_save_interval_secs: u64`（默认 `30`）— 自动保存间隔
- `default_language: String`（默认 `"C++"`）— 默认编程语言（**值域 = HOJ 语言显示名**，与提交契约一致；Monaco 高亮 id 由前端派生。上一版曾把值域归一为 Monaco id `'cpp'`，现由 `normalize_legacy_values` 重新映射回显示名）

### ThemeConfig
- `theme_name: String`（默认 `"light"`）— 当前主题名称（取值域：light；dark 尚未实现，加载时会被归一为 light）
- `editor_theme: String`（默认 `"vs"`）— Monaco 编辑器主题名（取值域：`vs` / `vs-dark`；由解题页编辑器设置切换，**只作用于编辑器区域**，客户端界面仍只有浅色。未知值由 `sanitize` 收敛回默认 —— `monaco.editor.setTheme` 收到未知主题名会静默保留上一主题）

### LayoutConfig
- `sidebar_width: u32`（默认 `280`）— 侧边栏宽度（像素）
- `split_ratio: f64`（默认 `0.48`）— 题面与编辑器分栏比例（0.0~1.0，0.5 表示各占一半；旧默认 0.45 由 `normalize_legacy_values` 归一）

## 默认值函数
所有默认值通过模块级私有函数提供（如 `default_hoj_url()`, `default_timeout()`, `default_font_size()` 等），`const fn` 用于编译期常量、`fn` 用于需要 `String` 的默认值。

## 旧值归一（P55）
- **`normalize_legacy_values(cfg: &mut AppConfig)`**（公开纯函数）— 一次性归一历史配置文件中的旧默认值，由 `ConfigService::new()` 与 `reload()` 两条加载路径调用。背景：`editor.defaultLanguage` 的值域已翻转为 HOJ 语言显示名（与提交契约一致）—— 被上一版归一成 Monaco id（`'cpp'` 等）的配置在此重新映射回显示名；另有 `theme = dark`（整机深色 UI 尚未实现，前端只有浅色）、`splitRatio = 0.45`（旧默认，现默认 0.48）仍按旧规则修正。**只修正恰好等于旧默认值的项，用户显式设置的其他值一律不动**：
  - 语言：经私有 `normalize_language_display_name` 把旧 Monaco id 映射为显示名（大小写不敏感：`cpp→"C++"` / `c→"C"` / `java→"Java"` / `python→"Python"`）；已是显示名或其他非空值（OJ 可能提供 Go/Rust 等）原样保留，仅空串回退默认 `"C++"`
  - 主题：`theme_name == "dark"` → `"light"`，并把它一并写入的 `editor_theme == "vs-dark"` 归位为 `"vs"`（整机深色配置 → 全浅色）。**仅在 `theme_name == "dark"` 时才动 `editor_theme`**：编辑器主题已是解题页可选项，`theme_name = light` + `editor_theme = vs-dark` 是合法组合，无条件重置会把选手刚选的深色编辑器悄悄改回浅色
  - 分栏比例：`split_ratio == 0.45` → `0.48`。浮点精确比较是刻意的：只有原样落盘的旧默认值才会二进制相等
- 抽成纯函数（而非埋在 ConfigService 里）是为了可被单元测试直接锁定，且幂等（每次加载都会执行）

## 写入路径校验与净化（M4）
`update_config` Command 持久化前调用（config.json 可被手改，前端 SettingsView 不是唯一防线）。均为纯函数/纯方法，可直接单测：
- **`AppConfig::validate(&self) -> Result<(), String>`** — 校验不可钳制项（目前仅 `oj.hoj_url`），失败返回可直接展示的错误消息。判据与前端一致：trim 后以 `http://` 或 `https://` 开头（大小写不敏感）且其余部分非空、不含空白（等价 `/^https?:\/\/\S+$/i`）。Command 层把 `Err(msg)` 映射为 `AppError::Config(msg)` 拒绝落盘
- **`AppConfig::sanitize(&mut self) -> bool`** — 就地钳制越界字段，取值域与前端 SettingsView 校验一致：`timeout_secs 1..=120`、`poll_interval_secs 1..=30`、`poll_timeout_secs 30..=3600`、`cache_ttl_secs 0..=600`、`contest_id >= 0`、`font_size 8..=32`、`tab_size 1..=8`、`auto_save_interval_secs 5..=300`、`split_ratio 0.30..=0.70`（非有限值先回退默认 0.48 再钳制）；`hoj_url` trim；`default_language` 经私有 `sanitize_language_id` 净化 —— 旧 Monaco id 先走 `normalize_language_display_name` 映射为显示名，其余非空值（含 OJ 可能提供的 Go/Rust 等）原样保留，仅空串回退默认 `"C++"`；`editor_theme` 经私有 `sanitize_editor_theme` 收敛到 Monaco 内置 `vs` / `vs-dark`（其余值回退默认 `vs` —— 未知主题名会让 `setTheme` 静默无效）。返回是否修改了任何字段（调用方据此记 warn 日志），为此各配置 struct 追加了 `PartialEq` derive

## 直接依赖
- `serde::{Deserialize, Serialize}`

## 被依赖
- `service::config::ConfigService`（通过 `ConfigRepository` 加载/保存 `AppConfig`，并在 `new()` / `reload()` 加载路径调用 `normalize_legacy_values`）
- `commands::config_cmd`（`update_config` 持久化前调用 `validate()` + `sanitize()` 做后端兜底）
- `service::theme::ThemeService`（读取/写入 `ThemeConfig`）
- `service::contest::ContestService`（通过 `OjConfig::cache_ttl_secs` 控制比赛列表缓存 TTL）
- `commands::problem_cmd`（`get_problem` 读取 `OjConfig::cache_problem_statement` 后传给 `ProblemService::open_problem` —— 配置读取归命令层，Service 只接参数，与 `contest_cmd` 传 `cache_ttl_secs` 同款约定）
- `core::context`（`AppContext::init` 读取 `OjConfig::timeout_secs` 构造 HttpClient）

## 逻辑流程
- 所有子 struct 的 `Default::default()` 返回合理的默认值，`AppConfig::default()` 组合各子 struct 默认值
- `#[serde(default)]` 保证 JSON 缺少字段时回退到 `Default`，实现向后兼容的配置热升级
- 加载路径（`ConfigService::new` / `reload`）反序列化后调用 `normalize_legacy_values` 归一旧默认值，保证前端拿到的配置永远落在取值域内
- 源文件：`src-tauri/src/core/entity/config.rs`

## 测试
`src-tauri/src/core/entity/tests/config_tests.rs`（由 `config.rs` 底部 `#[cfg(test)] #[path = "tests/config_tests.rs"] mod tests;` 引用）锁定：默认值全部落在取值域内（`C++` / `light` / `vs` / `0.48`）、旧 Monaco id（大小写不敏感）归一为显示名、已是显示名或未知非空值（Go/Rust 等）不动、仅空串回退 `C++`、整机 dark/vs-dark 归一为 light/vs 而浅色不动、**浅色界面 + `vs-dark` 编辑器（用户可选组合）归一后保持不动**、`split_ratio` 只替换恰好 0.45 的旧默认（0.44/0.46/0.5 等用户值不动）、归一幂等、上一版落盘 JSON（`defaultLanguage: "cpp"`）端到端反序列化 + 重新归一。M4 校验/净化用例：`validate` 拒绝空串 / `ftp://x` / 裸域名 / 只有 scheme 头 / 含空白地址并接受大小写混合的 http(s)（trim 后校验）、`sanitize` 钳制全部越界字段并返回 true、合法配置（默认值与边界值，含 `editor_theme = vs-dark`）原样不动且返回 false、旧 Monaco id 净化为显示名而非空未知值保留（仅空串回退 `C++`）、编辑器主题取值域收敛（`vs`/`vs-dark` 保留，空串/`dracula`/大小写变体回退 `vs`）、URL trim 与 NaN 分栏比例回退默认、手改 JSON 端到端 validate + sanitize 收敛。
