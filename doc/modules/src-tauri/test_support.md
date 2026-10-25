# test_support（测试专用支撑，仅 cfg(test) 编译）

> 源文件：`src-tauri/src/test_support.rs` + `test_support/loopback.rs` + `test_support/app_ctx.rs`

## 职责

为全 crate 测试提供两 类共享设施：

- **`TempDir` / `Guarded`**（test_support.rs）：RAII 临时目录（构造清残留、Drop 回收），解决长期跑测试的目录堆积问题。
- **`loopback::LoopbackServer`**（M2）：脚本化 loopback HTTP server —— 预置响应按序弹出、每个请求被录制（方法/路径+查询/头/体）。契约测试（HOJ/Hydro 适配器、命令层 only_mine 与分页）用它断言**真实网络栈**发出的请求；响应队列耗尽时回 500，让多余请求显式失败。
- **`app_ctx::context_with_hoj(base_url)`**（M2）：预写 `config.json`（HOJ 实例指向给定地址，通常为 loopback）后走 `AppContext::init` 构造**全真上下文**（真实配置/注册表/服务/事件总线/文件系统），返回 `(ctx, TempDir 守卫)`。命令层 `*_impl` 测试的统一底座。

## 依赖关系

- 只被 `#[cfg(test)]` 目标编译；依赖 tokio / reqwest / serde_json（均为既有多带依赖）。
- 适配器契约测试（`adapter/{hoj,hydro}/tests/mod_tests.rs` 契约段）、命令测试（`commands/tests/*_cmd_tests.rs`）、http 传输测试（`infra/tests/http_tests.rs`）共用。

## 相关约束

- OJ 实例 `id` 必须与适配器工厂声明完全一致（`"HOJ"`/`"Hydro"`，大小写敏感）——`register_instance` 按字符串匹配工厂。
