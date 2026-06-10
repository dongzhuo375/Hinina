use serde::{de::DeserializeOwned, Serialize};

use crate::core::error::AppResult;

/// 配置持久化仓库。
///
/// 抽象配置文件读写，解耦 ConfigService 与底层存储实现。
pub trait ConfigRepository: Send + Sync {
    /// 加载配置
    fn load_config<T: DeserializeOwned>(&self) -> AppResult<T>;

    /// 保存配置
    fn save_config<T: Serialize>(&self, config: &T) -> AppResult<()>;

    /// 检查配置文件是否存在
    fn config_exists(&self) -> bool;
}
