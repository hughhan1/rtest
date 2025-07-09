//! Python test discovery module.
//!
//! This module provides functionality for discovering tests in Python code
//! by parsing the AST and identifying test functions and classes based on
//! configurable naming patterns.

mod discovery;
mod pattern;
mod visitor;

// Re-export public API
pub use discovery::{discover_tests, test_info_to_function, TestDiscoveryConfig};
