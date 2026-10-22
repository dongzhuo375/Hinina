//! @plugin-module
//! @layer plugin/runtime
//! @version 2
//! @stability stable
//! 引擎接入契约，QuickJsRuntime 由专属线程持有。Runtime 不要求 Send，可由专属线程或
//! Tokio LocalSet 持有。async Promise 不等于安全中断：引擎必须另有执行/内存限制。
//! Manager 以 `Box<dyn PluginRuntime>` 在生产路径上实际使用本端口。

use crate::plugin::api::event::PluginEventEnvelope;
use crate::plugin::api::{PluginApi, PluginResult};
use async_trait::async_trait;
use std::sync::Arc;
pub mod quickjs;

/// @plugin-api
/// @since 2.0
/// @stability stable
/// What happened when one envelope was delivered to a plugin's handlers. A non-zero
/// `failed_handlers` means the plugin's view may have diverged, so the host should prompt
/// a reconciliation; the engine keeps running either way (one rejection never kills it).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DeliveryOutcome {
    /// Handlers that rejected during this delivery.
    pub failed_handlers: u32,
}

/// @plugin-api
/// @since 1.0
/// @stability stable
/// 引擎实现应串行等待同一插件的 handler。单个 handler 的拒绝只计入失败数、不终止引擎；
/// 引擎级失败（超时、内存、撤销、基础设施）才使该插件失败，且只影响该插件。
/// 不默认提供 fs/process/env/fetch/socket/native module，入口为打包好的 JS。
#[async_trait(?Send)]
pub trait PluginRuntime {
    /// @plugin-api
    /// @since 1.0
    /// @permission host-only
    /// @stability stable
    /// 加载源码并注册 handler；队列在 Host 注册时已含初始 Resync，activate 完成后
    /// worker 才消费该通知。失败由调用方注销订阅并调用 shutdown。
    async fn activate(
        &mut self,
        source: &str,
        api: Arc<dyn PluginApi>,
        config: &serde_json::Value,
        allowed_events: Vec<String>,
    ) -> PluginResult<()>;
    /// @plugin-api
    /// @since 1.0
    /// @stability stable
    /// 处理一条信封，可包含多个按注册顺序执行的 handler；错误只返回 PluginError。
    /// 单个 handler 拒绝不终止引擎：失败计数经 `DeliveryOutcome` 回传，供宿主决定是否
    /// 主动触发一次 Resync 重新对账。
    async fn deliver_event(
        &mut self,
        envelope: &PluginEventEnvelope,
    ) -> PluginResult<DeliveryOutcome>;
    /// @plugin-api
    /// @since 1.0
    /// @stability stable
    /// 幂等释放引擎/handler/在途请求；先注销 Host，丢弃缓存事件，保留私有存储。
    async fn shutdown(&mut self) -> PluginResult<()>;
}

#[cfg(test)]
#[path = "tests/runtime_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/engine_tests.rs"]
mod engine_tests;
