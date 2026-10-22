//! @plugin-module
//! @layer plugin
//! @version 2
//! @stability stable
//!
pub mod api;
pub(crate) mod bounded_json;
pub mod execution_api;
pub mod host;
pub(crate) mod limits;
pub mod manager;
pub mod permission;
pub mod query;
pub mod registry;
pub mod runtime;
#[cfg(test)]
#[path = "tests/support.rs"]
pub(crate) mod test_support;
