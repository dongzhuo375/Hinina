// OJ 适配器层：协议差异在此消化，向 Service 层只暴露 Provider trait。
//
// 「有哪些 OJ」是数据：接入新 OJ = 新建一个子目录 + 在 factories() 加一行，
// 不需要修改 core / infra / commands 的任何文件。

pub mod hoj;
pub mod hydro;
pub mod qduoj;
pub mod hustoj;

use std::sync::Arc;

use crate::core::event::event_bus::EventBus;
use crate::core::provider::registry::ProviderSet;
use crate::infra::http::HttpClient;
use crate::infra::storage::Storage;

/// 适配器构造依赖 —— **只准 infra 依赖**（http / 事件总线 / 存储）。
///
/// 硬约束：禁止把 Service 塞进 `AdapterDeps` —— 与「插件只能访问
/// `plugin/api`，禁止直接调用内部 Service」同理：适配器一旦反向依赖
/// 应用层，依赖边界就彻底糊掉了。
pub struct AdapterDeps {
    pub http_client: Arc<HttpClient>,
    pub event_bus: Arc<EventBus>,
    pub storage: Arc<Storage>,
}

/// OJ 适配器工厂：身份自声明 + 统一构造签名。
///
/// 这个形状（id + 构造 + 配置）即 v1.0 插件 manifest 的雏形 —— 将来把
/// 编译期工厂清单换成运行时扫描插件目录，上层（registry / context /
/// Service）不用再改。现在只做编译期清单，不做动态加载。
pub trait AdapterFactory: Send + Sync {
    /// OJ 身份（稳定契约：决定会话文件名 `sessions/{id}.json`）
    fn id(&self) -> &'static str;

    /// 按服务端地址构造该 OJ 的 Provider 能力集合。
    fn build(&self, deps: &AdapterDeps, base_url: &str) -> ProviderSet;
}

/// 内建 OJ 工厂清单。接入新 OJ：加 `pub mod xxx;` + 此处加一行 `&xxx::FACTORY`。
pub fn factories() -> Vec<&'static dyn AdapterFactory> {
    vec![&hoj::FACTORY, &hydro::FACTORY]
}

#[cfg(test)]
#[path = "tests/adapter_tests.rs"]
mod tests;
