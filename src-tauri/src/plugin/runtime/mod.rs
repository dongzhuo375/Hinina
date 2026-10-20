//! @plugin-module
//! @layer plugin/runtime
//! @version 1
//! @stability experimental
//! 引擎接入契约，尚无 JS 执行器。Runtime 不要求 Send，未来可由专属线程或
//! Tokio LocalSet 持有。async Promise 不等于安全中断：引擎必须另有执行/内存限制。

use crate::plugin::api::event::PluginEventEnvelope;
use crate::plugin::api::{PluginApi, PluginResult};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::mpsc;

/// @plugin-api
/// @since 1.0
/// @stability experimental
/// Runtime Bridge 的最小装配参数；api 必须已绑定宿主授权。只能接收插件信封，
/// 无 CoreEvent、AppContext、Service 或总线 Receiver。
pub struct PluginRuntimeBridge {
    pub api: Arc<dyn PluginApi>,
    pub events: mpsc::Receiver<PluginEventEnvelope>,
}

/// @plugin-api
/// @since 1.0
/// @stability experimental
/// 引擎实现应串行等待同一插件的 handler；异常/超时只停止该插件。
/// 不默认提供 fs/process/env/fetch/socket/native module，入口为打包好的 JS。
#[async_trait(?Send)]
pub trait PluginRuntime {
    /// @plugin-api
    /// @since 1.0
    /// @permission host-only
    /// @stability experimental
    /// 加载源码并注册 handler；队列在 Host 注册时已含初始 Resync，activate 完成后
    /// worker 才消费该通知。失败由调用方注销订阅并调用 shutdown。
    async fn activate(&mut self, source: &str, api: Arc<dyn PluginApi>) -> PluginResult<()>;
    /// @plugin-api
    /// @since 1.0
    /// @stability experimental
    /// 处理一条信封，可包含多个按注册顺序执行的 handler；错误只返回 PluginError。
    async fn deliver_event(&mut self, envelope: &PluginEventEnvelope) -> PluginResult<()>;
    /// @plugin-api
    /// @since 1.0
    /// @stability experimental
    /// 幂等释放引擎/handler/在途请求；先注销 Host，丢弃缓存事件，保留私有存储。
    async fn shutdown(&mut self) -> PluginResult<()>;
}

#[cfg(test)]
#[path = "tests/runtime_tests.rs"]
mod tests;
