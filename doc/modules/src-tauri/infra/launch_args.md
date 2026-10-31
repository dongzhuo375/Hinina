# launch_args（`--settings` 管理方钥匙）

> 源文件：`src-tauri/src/infra/launch_args.rs`

## 职责

解析 `--settings` 启动参数并固化为进程级启动标志，供 `launch_flags` 命令交给前端：带参启动时前端初始导航直落设置页（跳过登录门禁），且设置页的维护者层字段解锁。解决「配置指向连不上的服务器 → 登录失败 → 进不了设置 → 改不了地址」的管理方死锁；正常路径（含连点版本号的隐藏入口）只开选手层。命名取「打开设置页」——它不设置任何值，区别于 `--config` 那个会让人以为要传配置的误称。

定位是**防误触护栏而非安全边界**：config.json 明文可手改，前端锁不设后端强制。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `LaunchFlags` | `#[derive(Serialize)] struct { settings: bool }` | 线上形状：`{ "settings": bool }`；按需扩展字段，未出现的语义一律 false |
| `requested` | `fn requested(args: &[OsString]) -> bool`（私有） | 精确匹配 `--settings`：`--settings=x`、`-settings` 等一律不算；其余参数静默忽略（未知参数不炸启动）；`OsString` 入参与非 Unicode 参数共存 |
| `init` | `pub fn init(args: &[OsString])` | `main()` 收集 argv 后调用一次，把解析结果固化进 `OnceLock<LaunchFlags>`；重复调用不生效 |
| `get` | `pub fn get() -> LaunchFlags` | 读取标志；未 `init`（测试进程不经过 main）返回全 false 默认值。**非提交式读取**——不能在这里 `get_or_init` 写默认值，否则会把 OnceLock 提前定型、让随后真正的 `init` 静默失效（新写的往返测试抓住过） |

## 直接依赖

- `std::sync::OnceLock`（进程级标志，随进程消亡）
- `serde::Serialize`（线上形状）

## 被依赖

- `main.rs`（argv 收集后 `launch_args::init(&argv)`，紧随 `--console` 处理）
- `commands/app_cmd.rs`（`launch_flags` 命令的只读来源）

## 逻辑流程

1. `main()` 装 panic 钩子 → `arm()` → argv 收集（`args_os().skip(1)`）→ `--console` 处理 → `launch_args::init(&argv)` 固化标志 → Tokio runtime → Builder。
2. 前端守卫首次导航时经 `launch_flags` 命令读取标志：`settings=true` → 初始导航直落设置页（守卫逻辑见 `doc/modules/src/router/index.md`），设置页据此解锁维护者层。
3. 标志在一次进程生命周期内不可变；无 UI 会在运行中改它（重开带参进程才会变）。

## 测试

`src/infra/tests/launch_args_tests.rs`：精确匹配矩阵（全等 / `--settingsx` / `-settings` / `--settings=1` 拒绝 / 混入其它参数仍命中 / 非 Unicode 参数共存不 panic）；`init→get` 往返与未 init 默认值**收进单个 `#[test]`**——FLAGS 是进程级 OnceLock，测试 harness 并行执行，多用例各自 init 会互相踩状态（`frontend_log` 预算测试同款教训）。该用例首轮就抓住了 `get_or_init` 提前定型的真缺陷。

## 设计假设

- **解析语义与 `console::requested` 完全同构**：精确匹配、未知参数静默忽略、OsString 入参。三个启动参数（`--console` / `--settings`）未来若继续增多，可收敛成统一的 argv 解析模块；当前两处各自内联保持零抽象成本。
- **无跨进程转发**：`--settings` 只影响本进程的前端初始导航与页内锁；第二实例启动会被单实例插件拦截（见 `infra/single_instance.md`），其参数被丢弃——管理方在已运行的实例上无法「补带」参数，须退出后带参重启。
- **标志不可变**：不做运行中动态提权/降权；管理方会话以进程为单位。
