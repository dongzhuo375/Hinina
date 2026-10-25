// Hinina 库根，公开模块树

pub mod adapter;
pub mod commands;
pub mod core;
pub mod infra;
pub mod plugin;
pub mod service;

/// 测试专用支撑（临时目录 RAII 守卫等），仅 `cfg(test)` 编译。
#[cfg(test)]
pub mod test_support;
