//! @plugin-module
//! @layer plugin
//! @version 2
//! @stability stable
//!
pub mod api;
pub mod execution_api;
pub mod host;
pub mod manager;
pub mod permission;
pub mod query;
pub mod registry;
pub mod runtime;
#[cfg(test)]
pub(crate) mod tests;
