//! Python test discovery module.
//!
//! This module provides functionality for discovering tests in Python code
//! by parsing the AST and identifying test functions and classes based on
//! configurable naming patterns.

mod discovery;
pub mod module_resolver;
mod pattern;
pub mod semantic_analyzer;
mod visitor;

// Re-export public API
pub use discovery::{
    discover_tests, discover_tests_with_inheritance, test_info_to_function, TestDiscoveryConfig,
    TestInfo,
};
pub use module_resolver::{ModuleResolver, ParsedModule};
pub use semantic_analyzer::{SemanticTestDiscovery, TestClassInfo};
