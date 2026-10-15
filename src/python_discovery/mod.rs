//! Python test discovery module.
//!
//! This module provides functionality for discovering tests in Python code
//! by parsing the AST and identifying test functions and classes based on
//! configurable naming patterns.

mod cases;
mod constant_resolver;
mod discovery;
mod module_resolver;
mod pattern;
mod semantic_analyzer;
mod visitor;

// Re-export public API
pub use cases::{format_cannot_expand_warning, CannotExpandReason, CasesExpansion};
pub(crate) use discovery::DiscoveryCache;
pub use discovery::{
    discover_tests, discover_tests_with_inheritance, TestDiscoveryConfig, TestInfo,
};
