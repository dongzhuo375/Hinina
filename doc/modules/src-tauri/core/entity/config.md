# config

## 职责
定义应用全局配置的实体类型，包含用户偏好、OJ 连接（实例清单 + 当前 OJ + 比赛引用）、编辑器、主题、布局五类配置。所有 struct 均实现 `Serialize + Deserialize + Default`，以 JSON 格式通过 `ConfigService` 持久化和加载。所有可变行为参数均从 Config 读取，支持运行时热更新（`ConfigService::reload()`）。另提供旧值归一纯函数 `normalize_legacy_values`（P55），把历史配置中的旧默认值收敛到当前取值域，并完成 OJ 配置 v2 迁移（`hojUrl` / `contestId` / `lastOjType` → `instances` / `contestRef` / `active`）；以及写入路径的 `validate()` / `sanitize()`（M4），供 `update_config` Command 持久化前做后端兜底校验与钳制。

## 核心类型

### AppConfig
应用全局配置根 struct，字段均以 `#[serde(default)]` 标注以确保向后兼容。
- `user: UserConfig` — 用户偏好
- `oj: OjConfig` — OJ 连接配置
- `editor: EditorConfig` — 编辑器偏好
- `theme: ThemeConfig` — 主题与外观
- `layout: LayoutConfig` — 布局偏好

### UserConfig
- `last_username: String`（默认 `""`）— 用于自动填充登录用户名
- `legacy_last_oj_type: Option<String>` — 旧版「上次登录的 OJ 类型」，已迁移至 `oj.active`（OJ 选择是应用级状态，决定全部 Provider 行为，不属于用户偏好）。`#[serde(default, skip_serializing, rename = "lastOjType")]`：仅读取用于 `normalize_legacy_values` 迁移，迁移后不再写回

### OjInstance（OJ 配置 v2 新增）
单个 OJ 实例的连接配置。「配置了哪些 OJ」是数据：接一个新 OJ = 配置里加一条实例 + 适配器工厂清单加一行，**不需要**给 `OjConfig` 加字段（旧版 `hoj_url` 那种以具体 OJ 命名的字段，每接一个 OJ 就要加一条 + 三处校验分支）：
- `id: String` — OJ 身份（须与适配器工厂 `id()` 一致，如 "HOJ"）
- `base_url: String` — 服务端地址（站点根，如 `https://hoj.dongzhuo.top`，不带 `/api` 等前缀）
- `enabled: bool`（默认 `true`）— 是否启用（false = 组合根不注册该实例的 Provider）
- `options: serde_json::Map<String, Value>` — OJ 私有旋钮。刻意弱类型：强类型枚举会让「新 OJ 要改 core」原样复活

### OjConfig
- `active: String`（默认 `"HOJ"`）— 当前 OJ 实例 id（取代旧 `user.lastOjType`：OJ 选择是应用级状态）
- `instances: Vec<OjInstance>` — 已配置的 OJ 实例列表。**serde 缺省为空表而非默认清单**（与 `Default` 给出的单 HOJ 默认实例刻意不同）：旧格式文件（无 instances 键）须落到 `normalize_legacy_values` 的迁移分支（legacy hojUrl → 实例），而非被字段级默认直接填成默认地址 —— 那会静默丢掉用户自定义的旧地址；全新配置（无文件）走 `Default`，那里才是默认 HOJ 实例
- `contest_ref: String`（默认 `""`）— 当前比赛引用，**不透明字符串**：HOJ 是数字串，其它 OJ 可能是任意资源 ID（如 Hydro 的 hex ObjectId；旧 `contest_id: i64` 装不下非数字 ID）。「当前比赛」本质上是对服务端资源的引用，不该假设它是数字。空串 = 未配置，不自动加载
- `timeout_secs: u64`（默认 `30`）— HTTP 请求超时
- `poll_interval_secs: u64`（默认 `2`）— 评测轮询间隔（由前端 submissionStore 的 createPoller 经 get_config 消费，后端单次查询不读取）
- `poll_timeout_secs: u64`（默认 `300`）— 评测最大等待时间（同上，前端 poller 的 deadline 判据）
- `cache_ttl_secs: u64`（默认 `60`）— 比赛列表缓存 TTL
- `cache_problem_statement: bool`（默认 `true`）— 题面缓存开关（内存 + 磁盘，TTL 见 `service::problem::PROBLEM_CACHE_TTL`）。默认开启：题面在比赛期间基本不变，缓存可省掉「切题来回/重进应用」的重复请求并在断网时仍可打开；关闭后每次打开题目都直连服务端（题面被管理员中途修正时可临时关闭）。bool 无需 `sanitize` 钳制
- `contest_password: Option<String>`（默认 `None`）— 比赛密码（私有赛需要），公开赛留空
- 过渡字段（旧版字段，仅读取用于迁移，`skip_serializing` 保证永不写回）：`legacy_hoj_url: Option<String>`（`rename = "hojUrl"`，旧版 HOJ 专属地址 → 迁移为 instances 中 HOJ 实例的 `base_url`）、`legacy_contest_id: Option<i64>`（`rename = "contestId"`，旧版数字比赛 ID → 迁移为 `contest_ref` 字符串）

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
所有默认值通过模块级私有函数提供（如 `default_hoj_url()`, `default_timeout()`, `default_font_size()` 等），`const fn` 用于编译期常量、`fn` 用于需要 `String` 的默认值。OJ 侧另有 `default_active_oj()`（`"HOJ"`，与旧版 `default_oj_type` 一致，行为零变化）、`default_oj_instances()`（默认单 HOJ 实例，沿用旧版默认地址）与 `default_true()`（实例默认启用）。

## 旧值归一（P55）
- **`normalize_legacy_values(cfg: &mut AppConfig)`**（公开纯函数）— 一次性归一历史配置文件中的旧默认值，由 `ConfigService::new()` 与 `reload()` 两条加载路径调用。背景：`editor.defaultLanguage` 的值域已翻转为 HOJ 语言显示名（与提交契约一致）—— 被上一版归一成 Monaco id（`'cpp'` 等）的配置在此重新映射回显示名；另有 `theme = dark`（整机深色 UI 尚未实现，前端只有浅色）、`splitRatio = 0.45`（旧默认，现默认 0.48）仍按旧规则修正。**只修正恰好等于旧默认值的项，用户显式设置的其他值一律不动**：
  - 语言：经私有 `normalize_language_display_name` 把旧 Monaco id 映射为显示名（大小写不敏感：`cpp→"C++"` / `c→"C"` / `java→"Java"` / `python→"Python"`）；已是显示名或其他非空值（OJ 可能提供 Go/Rust 等）原样保留，仅空串回退默认 `"C++"`
  - 主题：`theme_name == "dark"` → `"light"`，并把它一并写入的 `editor_theme == "vs-dark"` 归位为 `"vs"`（整机深色配置 → 全浅色）。**仅在 `theme_name == "dark"` 时才动 `editor_theme`**：编辑器主题已是解题页可选项，`theme_name = light` + `editor_theme = vs-dark` 是合法组合，无条件重置会把选手刚选的深色编辑器悄悄改回浅色
  - 分栏比例：`split_ratio == 0.45` → `0.48`。浮点精确比较是刻意的：只有原样落盘的旧默认值才会二进制相等
  - **OJ 配置 v2 迁移**（`hojUrl` / `contestId` / `lastOjType` → `instances` / `contestRef` / `active`，旧字段经 serde 过渡字段读入）：
    - `instances` 为空 → 以旧 `legacy_hoj_url`（缺省用默认地址）建立单 HOJ 实例（id `"HOJ"`、enabled、空 options）
    - `user.legacy_last_oj_type` 非空白 → 写入 `oj.active`（`take()` 消费，配合 `skip_serializing` 不再写回）
    - `contest_ref` 为空且 `legacy_contest_id > 0` → 迁移为数字字符串；旧 `contestId == 0` 表示未配置、负数是脏值，均不迁移
    - `contest_ref` trim；实例 `id` 与 `active` 统一 trim（id 带空白会让工厂匹配静默失败）；`active` 不指向**已启用**实例时回退首个启用实例（而非硬编码 HOJ —— HOJ 实例可能被禁用/移除，指向不存在或不可用的 id 都是死路；迁移分支保证 instances 至少一条，回退目标恒存在）
- 抽成纯函数（而非埋在 ConfigService 里）是为了可被单元测试直接锁定，且幂等（每次加载都会执行）

## 写入路径校验与净化（M4）
`update_config` Command 持久化前调用（config.json 可被手改，前端 SettingsView 不是唯一防线）。均为纯函数/纯方法，可直接单测：
- **`AppConfig::validate(&self) -> Result<(), String>`** — 校验不可钳制项，失败返回可直接展示的错误消息（Command 层把 `Err(msg)` 映射为 `AppError::Config(msg)` 拒绝落盘）。四条判据：
  1. 实例 id 非空、**不含路径分隔符与 `..`**（id 会拼进会话文件名 `sessions/{id}.json` 与注册表键，路径穿越必须拒绝；`Storage::resolve` 是第二道防线）且唯一（重复 id 会让注册表互相覆盖、会话文件名撞车）
  2. enabled 实例的 `base_url` 合法 —— 判据与前端 SettingsView 一致：trim 后以 `http://` 或 `https://` 开头（大小写不敏感）且其余部分非空、不含空白（等价 `/^https?:\/\/\S+$/i`）；禁用实例不注册，地址可为占位
  3. `active` 必须指向**已启用**的实例（禁用实例不会被注册，指向它等于死路）
- **`AppConfig::sanitize(&mut self) -> bool`** — 就地钳制越界字段，取值域与前端 SettingsView 校验一致：`timeout_secs 1..=120`、`poll_interval_secs 1..=30`、`poll_timeout_secs 30..=3600`、`cache_ttl_secs 0..=600`、`font_size 8..=32`、`tab_size 1..=8`、`auto_save_interval_secs 5..=300`、`split_ratio 0.30..=0.70`（非有限值先回退默认 0.48 再钳制）；`oj.active`、`oj.contest_ref` 与各实例的 `id` / `base_url` trim；`default_language` 经私有 `sanitize_language_id` 净化 —— 旧 Monaco id 先走 `normalize_language_display_name` 映射为显示名，其余非空值（含 OJ 可能提供的 Go/Rust 等）原样保留，仅空串回退默认 `"C++"`；`editor_theme` 经私有 `sanitize_editor_theme` 收敛到 Monaco 内置 `vs` / `vs-dark`（其余值回退默认 `vs` —— 未知主题名会让 `setTheme` 静默无效）。返回是否修改了任何字段（调用方据此记 warn 日志），为此各配置 struct 追加了 `PartialEq` derive

## 直接依赖
- `serde::{Deserialize, Serialize}`

## 被依赖
- `service::config::ConfigService`（通过 `ConfigRepository` 加载/保存 `AppConfig`，并在 `new()` / `reload()` 加载路径调用 `normalize_legacy_values`）
- `commands::config_cmd`（`update_config` 持久化前调用 `validate()` + `sanitize()` 做后端兜底）
- `service::theme::ThemeService`（读取/写入 `ThemeConfig`）
- `service::contest::ContestService`（通过 `OjConfig::cache_ttl_secs` 控制比赛列表缓存 TTL）
- `commands::problem_cmd`（`get_problem` 读取 `OjConfig::cache_problem_statement` 后传给 `ProblemService::open_problem` —— 配置读取归命令层，Service 只接参数，与 `contest_cmd` 传 `cache_ttl_secs` 同款约定）
- `commands::contest_cmd`（`load_configured_contest` 读取 `OjConfig::contest_ref`，空串报错提示配置）
- `commands::oj_cmd`（`switch_oj` 经 `ConfigService::update` 持久化 `OjConfig::active`）
- `core::context`（`AppContext::init` 读取 `OjConfig::timeout_secs` 构造 HttpClient，并按 `oj.active` / `oj.instances` 注册各 OJ 的 Provider）

## 逻辑流程
- 所有子 struct 的 `Default::default()` 返回合理的默认值，`AppConfig::default()` 组合各子 struct 默认值
- `#[serde(default)]` 保证 JSON 缺少字段时回退到 `Default`，实现向后兼容的配置热升级
- 加载路径（`ConfigService::new` / `reload`）反序列化后调用 `normalize_legacy_values` 归一旧默认值，保证前端拿到的配置永远落在取值域内
- 源文件：`src-tauri/src/core/entity/config.rs`

## 测试
`src-tauri/src/core/entity/tests/config_tests.rs`（由 `config.rs` 底部 `#[cfg(test)] #[path = "tests/config_tests.rs"] mod tests;` 引用）锁定：默认值全部落在取值域内（`C++` / `light` / `vs` / `0.48`）、旧 Monaco id（大小写不敏感）归一为显示名、已是显示名或未知非空值（Go/Rust 等）不动、仅空串回退 `C++`、整机 dark/vs-dark 归一为 light/vs 而浅色不动、**浅色界面 + `vs-dark` 编辑器（用户可选组合）归一后保持不动**、`split_ratio` 只替换恰好 0.45 的旧默认（0.44/0.46/0.5 等用户值不动）、归一幂等、上一版落盘 JSON（`defaultLanguage: "cpp"`）端到端反序列化 + 重新归一。M4 校验/净化用例：`validate` 拒绝空串 / `ftp://x` / 裸域名 / 只有 scheme 头 / 含空白地址并接受大小写混合的 http(s)（trim 后校验）、`sanitize` 钳制全部越界字段并返回 true、合法配置（默认值与边界值，含 `editor_theme = vs-dark`）原样不动且返回 false、旧 Monaco id 净化为显示名而非空未知值保留（仅空串回退 `C++`）、编辑器主题取值域收敛（`vs`/`vs-dark` 保留，空串/`dracula`/大小写变体回退 `vs`）、URL trim 与 NaN 分栏比例回退默认、手改 JSON 端到端 validate + sanitize 收敛。OJ 配置 v2 用例：旧配置（`hojUrl` + `lastOjType` + `contestId`）迁移为 instances / active / contestRef 且序列化产物只含新格式键（`lastOjType` / `hojUrl` / `contestId` 不再写回）、`contestId == 0` 与负数不迁移且空白 `lastOjType` 回退 HOJ、新格式原样通过（自定义实例清单不被覆盖、非数字 `contestRef` 合法）、active 指向未配置实例时归一回退首个启用实例、active 指向**禁用**实例时回退首个启用实例（不停留在死路 id）、实例 id 与 active 的 trim 归一、validate 拒绝重复实例 id / 未知 active / 含路径字符的实例 id（`/`、`\`、`..`）；手改 JSON 端到端用例改为与真实管线一致（加载归一 `normalize` + 写入兜底 `sanitize` 双路径收敛：旧 hojUrl 迁移进 HOJ 实例地址、脏值 `contestId = -1` 不迁移）。
