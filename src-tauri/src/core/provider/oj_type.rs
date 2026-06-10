use serde::{Deserialize, Serialize};

/// OJ 类型标识
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OJType {
    HOJ,
    QDUOJ,
    HUSTOJ,
}
