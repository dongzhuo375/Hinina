# CI/CD 基线清理记录（PR2：fmt/clippy 清零与 rust-lint 升必需）

> 记录日期：2026-10-25  
> 版本锚定：`main` @ 116b700（PR #52 合并后），分支 `chore/ci-baseline-cleanup`  
> 文档性质：CI/CD 施工过程记录（入库，见 [doc/cicd/](README.md)）；对应施工方案 §十 的 PR2「存量 lint/test 基线修复」  
> 状态：E2/E3 全部清零，rust-lint 已升为必需检查

## 一、处置内容（4 个提交）

| 提交 | 内容 |
|---|---|
| `7ab2ded` | **E2 清零**：`cargo fmt --all`（48 文件，M0 实测 220 块、清零前 190 块）；含工作区既有 4 个 fmt 修复文件（hydro error/types、logger_tests）。无语义变化 |
| `ade9900` | **E3 清零**：clippy 7 处——AppConfig 手写 Default 改 `derive(Default)`（逐字段 `Xxx::default()` 与 derive 完全等价）；hoj 会话三态测试两处 `assert_eq!` 字面布尔改 `assert!`；`is_terminal() == false` 改 `!is_terminal()`；cache_tests 冗余 `i as i32`；commands mod_tests 一处同类；oj 切换测试处理 `remove_all` 的 must_use Result |
| 本提交 | **rust-lint 升必需**：`ci-required` 的 `needs` 加入 `rust-lint`（contexts 不变，仍为单一 `ci-required`）；m0 记录 E2/E3/计数差异三项闭环标注 |

## 二、749/748 计数差异结案（M0 遗留待查项）

静态盘点（源码 `#[test]`/`#[tokio::test]` 属性计数）比 Windows 运行数多 1，差异来自 `infra/tests/fs_plugin_repo_tests.rs` 的 `a_symlink_escaping_the_code_root_is_skipped_not_fatal`——标注 **`#[cfg(unix)]`**，Windows 上不编译故不运行。**非缺测、非漏计**：Linux runner 上运行数将包含它。方法论备注：静态盘点须把 `#[cfg(...)]` 条件编译的测试单独归类，否则与运行数必然有平台性偏差。

## 三、rust-lint 升必需后的门禁构成

`ci-required`（分支保护唯一必需检查名）= `frontend` + `rust-lint`（fmt --check + clippy -D warnings，**首次全绿**）+ `rust-test`（Windows/Ubuntu 矩阵）。job 稳定名与 m0-baseline §六 一致，未触发改名同步。

## 四、验证

- `cargo fmt --all -- --check`：0 差异
- `cargo clippy --locked --all-targets -- -D warnings`：RC=0（M0 以来首次）
- `cargo test --lib`：780 通过；logger 集成测试 2 项通过
- 前端 561 / build / TSC 不受影响（无前端改动）

## 五、施工中发现并修复的测试 flaky（评审第三轮顺带）

全量套件在宿主**开着代理**的环境下每轮偶发 1-3 个网络测试失败（M2 评审时全绿的原因：那时代理未开）。hyper trace 实证根因：**reqwest 默认读取 `HTTP_PROXY` 等环境变量，loopback 的 127.0.0.1 请求被宿主代理（127.0.0.1:10809）劫持**——约 45% 的连续 POST 拿到死连接（"error sending request"）。修复：

- `HttpClient::with_timeout_no_proxy`（cfg(test) 专用构造器，`.no_proxy()`）；
- 全部测试构造点换用（test_adapter_deps、hoj/hydro fixture、infra http_tests）；
- `AppContext::init` 在测试分支同样使用 no_proxy 构造（生产分支保持读环境代理不变——生产代理语义是否需要显式化列为遗留观察）。

修复后 `submit_server_error` 12/12、全量三轮全绿（原 45% 失败率）。另两处顺带修：

- oj 切换测试的磁盘缓存断言改为**轮询等 on_oj_switched 的异步清理落地**（problem 服务的磁盘题面缓存清理是 spawn 异步，原 `remove_all` expect 会与写盘/清理赛跑）；
- loopback 底座改 **HTTP/1.1 keep-alive 循环**（原 `Connection: close` + 立即关闭 socket 与 reqwest 连接池复用存在竞态；录制与弹出同临界区的原子化也在本 PR 落地）。
