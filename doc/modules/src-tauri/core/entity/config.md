# config

## 职责
定义应用全局配置的实体类型，包含用户偏好、OJ 连接、编辑器、主题、布局五类配置。所有 struct 均实现 `Serialize + Deserialize + Default`，以 JSON 格式通过 `ConfigService` 持久化和加载。所有可变行为参数均从 Config 读取，支持运行时热更新（`ConfigService::reload()`）。

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
- `default_language: String`（默认 `"C++"`）— 默认编程语言

### ThemeConfig
- `theme_name: String`（默认 `"dark"`）— 当前主题名称
- `editor_theme: String`（默认 `"vs-dark"`）— Monaco 编辑器主题名

### LayoutConfig
- `sidebar_width: u32`（默认 `280`）— 侧边栏宽度（像素）
- `split_ratio: f64`（默认 `0.45`）— 题面与编辑器分栏比例（0.0~1.0）

## 默认值函数
所有默认值通过模块级私有函数提供（如 `default_hoj_url()`, `default_timeout()`, `default_font_size()` 等），`const fn` 用于编译期常量、`fn` 用于需要 `String` 的默认值。

## 直接依赖
- `serde::{Deserialize, Serialize}`

## 被依赖
- `service::config::ConfigService`（通过 `ConfigRepository` 加载/保存 `AppConfig`）
- `service::theme::ThemeService`（读取/写入 `ThemeConfig`）
- `service::submission::SubmissionService`（通过 `OjConfig` 获取轮询参数）
- `service::contest::ContestService`（通过 `OjConfig::cache_ttl_secs` 控制缓存 TTL）

## 逻辑流程
- 所有子 struct 的 `Default::default()` 返回合理的默认值，`AppConfig::default()` 组合各子 struct 默认值
- `#[serde(default)]` 保证 JSON 缺少字段时回退到 `Default`，实现向后兼容的配置热升级
- 源文件：`src-tauri/src/core/entity/config.rs`
