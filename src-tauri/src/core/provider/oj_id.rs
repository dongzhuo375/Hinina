use serde::{Deserialize, Serialize};
use std::fmt;

/// OJ 身份标识 —— 数据，而非编译期枚举。
///
/// 接入一个新 OJ 不应要求修改 Domain（Clean Architecture：领域类型不随外部
/// 集成点的数量变化）。适配器经 `AdapterFactory::id()` 自声明身份
/// （如 `adapter::hoj::ID`），本类型只做非空规整（trim）。
///
/// **持久化契约**：会话文件名 = `{id}.json`（如 `sessions/HOJ.json`）。
/// 内建 OJ 的 id 与历史上枚举变体的 Debug 输出完全一致，旧会话文件无需迁移。
///
/// 取代闭集枚举后的真实代价是失去编译期穷尽检查，防线移至：
/// 启动时校验当前 OJ 已注册并告警、查询未命中返回 `ProviderNotFound`、
/// `factories()` 的 id 唯一性测试（见 `adapter/mod.rs`）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OjId(String);

impl OjId {
    pub fn new(raw: &str) -> Self {
        Self(raw.trim().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 会话文件名（不含目录）：`{id}.json`。
    pub fn session_file(&self) -> String {
        format!("{}.json", self.0)
    }
}

impl fmt::Display for OjId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
