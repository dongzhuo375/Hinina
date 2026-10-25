# CI/CD M0 基线冻结记录

> 记录日期：2026-10-25  
> 版本锚定：`main` @ 135039e，分支 `chore/ci-m0-baseline` @ 265e918（基线在两个工具链提交之后执行）  
> 文档性质：CI/CD 施工过程记录（入库，见 [doc/cicd/](README.md) 目录说明）；前置规划文档《CI/CD 现状评估与施工方案》《CI/CD 分阶段施工方案》为本地调研文档，位于 `doc/local/`（不入库）  
> 执行边界：全部 10 条命令在本机（Windows）实际执行并计时，原始日志在本地 `doc/local/m0-logs/`（不入库），本文数字均摘自日志；**失败只记录未修复**（修复属 PR1/PR2 范围）。cargo 侧为暖缓存计时，CI 冷启动会更慢。

## 一、工具链基线

| 项 | 值 | 说明 |
|---|---|---|
| Node.js | 22.14.0 | 与 `.nvmrc` 一致（265e918 引入） |
| npm | 11.18.0（**未固定**） | 本机手动升级所得；Node 22.14.0 官方自带 npm 为 10.9.2，CI 经 setup-node + `.nvmrc` 得到的是自带版本，与本机存在静默不一致。M1 ci.yml 施工时需决策：`packageManager` 字段固定，或接受 runner 自带 npm 并更新本记录（见 §十） |
| Rust | 1.96.0 (ac68faa20 2026-05-25) | 与 `rust-toolchain.toml` 一致（265e918 引入；rustup 已为固定 channel 下载组件） |
| rustfmt | 1.9.0-stable (ac68faa20) | 已安装 |
| clippy | 0.1.96 (ac68faa20) | 已安装 |
| 默认 target | x86_64-pc-windows-msvc | 唯一已安装 target |
| 操作系统 | Windows 10.0.26200 x64 | |
| npm 缓存 | `D:\nodejs\node_cache` | 在 C 盘外，符合仓库约定；执行当日曾现 ACL 故障（见 E4，当日已修复） |

## 二、命令执行结果（施工方案 §3.2.2 全量，逐条实测）

| # | 命令 | 结果 | 耗时 | 备注 |
|---|---|---|---|---|
| 1 | `npm ci` | ✅ 通过 | 23s | 166 包装齐；因 E4 使用工作区回退缓存（冷缓存口径） |
| 2 | `npx tsc -p tsconfig.node.json --noEmit` | ❌ 失败 | 2s | `vite.config.ts` 的 `node:fs`/`node:url`/`process` 无类型；根因 E1 |
| 3 | `npm run plugin:sdk:check` | ✅ 通过 | 2s | SDK typings + scaffold + balloon 示例类型检查 |
| 4 | `npm test` | ✅ 通过 | 6s | **39 个测试文件、538 个测试全部通过**（38 个 src + 1 个 balloon SDK 示例） |
| 5 | `npm run build` | ✅ 通过 | 27s | vue-tsc --noEmit + vite build（app tsconfig 不含 vite.config.ts，故 E1 不影响本命令） |
| 6 | `cargo fmt --all -- --check` | ❌ 失败 | 1s | 220 个格式差异块，六模块全覆盖（infra 96 / adapter 66 / plugin 16 / commands 16 / core 13 / service 12 / lib 1）；见 E2 |
| 7 | `cargo clippy --locked --all-targets -- -D warnings` | ❌ 失败 | 17s | 7 个错误、4 类机械 lint（i32 自身转换、`== false` 比较、可 derive 的 impl、`assert_eq!` 字面布尔）；见 E3 |
| 8 | `cargo test --locked --all-targets` | ✅ 通过 | 34s | **748 通过 / 0 失败 / 0 ignored**（lib 目标 748 + main 目标 0） |
| 9 | `cargo test --locked --doc` | ✅ 通过 | 3s | 本 crate 无 doctest（0 个），命令为无害空转 |
| 10 | `npm run tauri build` | ✅ 通过 | 139s | 产出 `src-tauri/target/release/bundle/nsis/Hinina_0.1.0_x64-setup.exe`（20,287,482 字节 ≈ 19.3 MiB）；release 编译 1m31s |

计时口径：npm 侧命令 1 为全新缓存冷安装；cargo 侧命令 7/8/9 在既有暖缓存上执行（target/ 约 20GB）。CI 无缓存首跑耗时将显著高于本表。

静态 vs 运行差异：Rust 测试函数静态盘点 749 个（源码 `#[test]`/`#[tokio::test]` 属性计数），实际运行 748 个，差 1 待查（无 ignored；PR2 基线清理时核实，不阻塞 M1）。

## 三、问题分类清单（施工方案 §3.2.3 六类逐项）

| 编号 | 现象 | 分类 | 证据（日志名，存本地 `doc/local/m0-logs/`） | 处置 |
|---|---|---|---|---|
| E1 | `tsc -p tsconfig.node.json --noEmit` 失败：`@types/node` 不在 devDependencies，`tsconfig.node.json` 亦无 `types` 配置 | 存量依赖缺口（非本机问题、非产品缺陷） | `02-tsc-node.log`；package.json devDependencies 无 `@types/node` | **阻塞 M1 frontend job 的 tsc-node 步骤**。修复：新增 `@types/node` devDependency（按仓库约定先与用户确认安装位置）→ 归 PR1/PR2 |
| E2 | rustfmt 220 个差异块，全模块分布 | 存量格式欠账 | `06-cargo-fmt.log`（2730 行 diff） | 修复 = 一次性 `cargo fmt` 全量格式化，独立提交 → PR2 |
| E3 | clippy `-D warnings` 下 7 个错误 | 存量 lint 欠账 | `07-cargo-clippy.log` | 4 类均为机械改写；PR2 清零后将 rust-lint 设为必需检查（施工方案 §4.2 原设计） |
| E4 | `D:\nodejs\node_cache` 全树对普通用户不可写（ACL 仅 Administrators/SYSTEM 有写权，Users/Authenticated Users 只读），npm 写缓存 EPERM；非提升会话连 `icacls /grant` 亦被拒 | 本机环境故障（仅影响本机，不影响 CI；**已闭环**） | `01-npm-ci.log` 首次失败记录 + icacls 探测输出 | **已修复（2026-10-25）**：本机管理员终端执行 `icacls "D:\nodejs\node_cache" /grant "hdz:(OI)(CI)M" /T`，探测确认可写；回滚命令：`icacls "D:\nodejs\node_cache" /remove:d hdz /T`。基线执行时以工作区回退缓存绕过（该缓存已删除） |
| — | 产品缺陷 | 无 | 测试 0 失败、构建全通过 | — |
| — | 测试错误或抖动 | 无（本次无 flaky；749/748 差异非抖动） | `04`/`08` 日志 | — |
| — | CI 配置尚未具备 | 无 workflows（by design，M1 施工） | 无 `.github/workflows/` 目录（`.github/` 下仅 copilot-instructions.md 与 pr-assets/，均非 CI） | — |

## 四、Ubuntu runner 准备步骤（M0 任务 4，可直接复制）

来源：Tauri 2 官方 prerequisites（v2.tauri.app/start/prerequisites，2026-10-25 抓取）+ 本仓 README「环境」节（QuickJS 需 C 编译器）+ `src-tauri/Cargo.toml` rquickjs 依赖注释（构建期 C 编译）。

```bash
sudo apt update
sudo apt install -y \
  libwebkit2gtk-4.1-dev \
  build-essential \
  curl \
  wget \
  file \
  libxdo-dev \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev
```

说明：

- `build-essential` 提供 gcc/make，同时满足 rquickjs 编译 QuickJS C 源码的前提（README 明示 Linux 需 gcc/clang）。
- `pkg-config`、`git`：GitHub-hosted ubuntu runner 预装；自备环境需补 `pkg-config`。
- `patchelf` **不在官方 prerequisites 清单**——Linux AppImage 打包阶段（M5）才需要，届时另加。
- Windows runner（M1 rust-test 矩阵）：GitHub windows-latest 预装 MSVC 与 WebView2，无需额外步骤（本机 NSIS 产物已实证可产出）。
- 所有执行 Cargo/Tauri 编译的 Ubuntu job（rust-lint、rust-test、后续 Tauri build）必须先跑上述 apt 步骤；纯前端命令的 frontend job 不需要（施工方案 §4.2）。

## 五、首版发布边界（M0 任务 5）

- **Windows x64 NSIS**：唯一发布目标，本次基线已实证可产出（命令 10，19.3 MiB）。
- macOS/Linux：仅做构建观测（M3 integration），不公开承诺支持；macOS/Linux 的插件运行时构建验证此前从未做过（README 明示），首验放在 M3。

## 六、workflow 与 job 稳定名称（M0 任务 6，M1 起沿用，改名须同步分支保护）

| Workflow | 触发 | Jobs（稳定名） |
|---|---|---|
| `ci.yml` | pull_request、push main、workflow_dispatch | `frontend`、`rust-lint`、`rust-test`（windows + ubuntu 矩阵）、`ci-required`（汇总） |
| `integration.yml` | push main、每日定时、workflow_dispatch | M3 定义 |
| `release.yml` | `v*.*.*` 标签、人工 | M4 定义（validate / test / build-windows / verify-artifact / draft-release） |

## 七、M1 阻塞性判定（施工方案 §3.4 第 2 条）

首个 CI **不能**全量直接设为阻塞，具体缺口与对应修复：

1. `frontend` job 的 tsc-node 步骤必红（E1）→ 补 `@types/node` 后才能阻塞；其余 frontend 步骤（npm ci / sdk-check / test / build）本次全绿，可直接阻塞。
2. `rust-lint` 必红（E2+E3）→ 按施工方案 §4.2 首版以非阻塞上线，PR2 清零 fmt/clippy 存量后升为必需。
3. `rust-test`（Windows/Ubuntu）本机 Windows 侧全绿可直接阻塞；Ubuntu 侧首次运行前需完成 §四 依赖步骤，且存在跨平台路径/文件语义风险（M1 首跑观测）。

## 八、验收对照（施工方案 §3.4 逐条）

1. **同一开发机从干净依赖安装开始可重复执行** ✅：npm 侧以全新回退缓存完整重装后逐条执行；cargo 侧暖缓存连续执行（fmt→clippy→test→doc 串行无冲突）。冷缓存差异已在 §二 注明。
2. **首个 CI 能否全量阻塞已明确** ✅：不能；阻塞项有具体修复任务（E1/E2/E3 → PR1/PR2），非长期忽略（见 §七）。
3. **未把"尚未执行"写成"已经通过"** ✅：10 条命令全部实际执行，失败 3 条（#2/#6/#7）如实记录为失败。

## 九、分支与 PR 顺序（照录施工方案 §十，M0 分支提交在前）

分支 `chore/ci-m0-baseline` 共四个提交：`3bc2398` chore(repo): ignore local planning notes under doc/local；`265e918` chore(ci): pin node and rust toolchain baselines（.nvmrc / rust-toolchain.toml / package.json engines / Architecture.md 根目录树同步）；`1b619a1` docs(cicd): add M0 baseline record under doc/cicd（本记录入库 + Architecture.md doc 树同步）；`350e735` docs(cicd): mark E4 npm cache ACL fixed and fallback cache removed（E4 闭环补充）。

后续 PR 顺序：① 工具链固定、engines.node、.gitattributes 行尾治理与最小 ci.yml → ② 存量 lint/test 基线修复（E1/E2/E3）→ ③ 真实 Command 测试 → ④ 本地 HTTP 与 Adapter 契约测试 → ⑤ 关键前端测试 → ⑥ 覆盖率与夜间构建 → ⑦ Playwright 与桌面 smoke → ⑧ 版本门禁与草稿 Release（含 CHANGELOG/RELEASE/SECURITY）→ ⑨ OS 签名与平台扩展 → ⑩ Updater 与供应链。

## 十、遗留事项

- ~~E4 ACL 修复~~ 已完成（2026-10-25，见 §三）；工作区回退缓存 `doc/local/m0-npm-cache/`（88MB，不入库）已删除。
- npm 未随工具链固定（§一）：本机 11.18.0 为手动升级，Node 22.14.0 自带 10.9.2——M1 ci.yml 施工时决策 `packageManager` 或接受 runner 默认，决策后更新 §一。
- 749/748 测试计数差异待 PR2 核实（§二）。
- 开发手册 §5.3「CI 自动发布」等描述仍超前于现状，按施工方案 §11.2 在 M1 落地后统一纠正。
