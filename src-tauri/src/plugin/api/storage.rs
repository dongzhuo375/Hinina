//! @plugin-module
//! @layer plugin/api
//! @version 1
//! @stability stable
//! 绑定插件 ID 的私有 JSON 存储契约；键是不透明字符串，不是文件路径。
use super::{PluginError, PluginResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// @plugin-api
/// @since 1.0
/// @stability stable
/// UTF-8 字节上限。
pub const MAX_STORAGE_KEY_BYTES: usize = 256;
/// @plugin-api
/// @since 1.0
/// @stability stable
/// JSON 序列化字节上限。
pub const MAX_STORAGE_VALUE_BYTES: usize = 65_536;

/// @plugin-api
/// @since 1.0
/// @permission storage.read
/// @stability stable
/// 区分不存在与 JSON null，不能以 null 同时表示两种状态。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum PluginStorageValue {
    Missing,
    Value { value: Value },
}

/// @plugin-api
/// @since 1.0
/// @stability stable
/// 验证键长度和控制字符；失败 InvalidInput 或 LimitExceeded。
pub fn validate_key(key: &str) -> PluginResult<()> {
    if key.is_empty() || key.chars().any(char::is_control) {
        return Err(PluginError::InvalidInput);
    }
    if key.len() > MAX_STORAGE_KEY_BYTES {
        return Err(PluginError::LimitExceeded);
    }
    Ok(())
}

/// @plugin-api
/// @since 1.0
/// @stability stable
/// 检查 JSON 序列化体积；不落盘，不提供原子性保证。
pub fn validate_value(value: &Value) -> PluginResult<()> {
    let bytes = serde_json::to_vec(value).map_err(|_| PluginError::InvalidInput)?;
    if bytes.len() > MAX_STORAGE_VALUE_BYTES {
        return Err(PluginError::LimitExceeded);
    }
    Ok(())
}

/// @plugin-api
/// @since 1.0
/// @permission storage.read / storage.write
/// @stability stable
/// 实现应以绑定授权中的插件 ID 隔离命名空间；停止/卸载保留数据。
#[async_trait]
pub trait PluginStorageApi: Send + Sync {
    /// @plugin-api
    /// @since 1.0
    /// @permission storage.read
    /// @stability stable
    async fn get(&self, key: &str) -> PluginResult<PluginStorageValue>;
    /// @plugin-api
    /// @since 1.0
    /// @permission storage.write
    /// @stability stable
    async fn set(&self, key: &str, value: Value) -> PluginResult<()>;
    /// @plugin-api
    /// @since 1.0
    /// @permission storage.write
    /// @stability stable
    /// 删除不存在键成功，I/O 失败返回 Unavailable。
    async fn delete(&self, key: &str) -> PluginResult<()>;
    /// @plugin-api
    /// @since 1.0
    /// @permission storage.read / storage.write
    /// @stability stable
    /// 单命名空间内原子比较 JSON 值再持久化；成功响应应发生在原子写完成后。
    /// 仅单 Host 进程保证，不代替外部服务幂等。
    async fn compare_and_set(
        &self,
        key: &str,
        expected: PluginStorageValue,
        value: Value,
    ) -> PluginResult<bool>;
}
