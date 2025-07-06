//! Safe downcasting utilities
//!
//! This module provides safe downcasting extensions for the Collector trait
//! to avoid silent failures and improve error handling.

use crate::collection::{Collector, Function};
use crate::error::{CollectionError as NewCollectionError, RtestError};
use std::any::Any;

/// Extension trait for safe downcasting
pub trait SafeDowncast: Any {
    /// Safely downcast to a concrete type with error handling
    fn safe_downcast_ref<T: Any>(&self) -> Result<&T, DowncastError>;
    
    /// Try to downcast to a concrete type
    fn try_downcast_ref<T: Any>(&self) -> Option<&T>;
}

/// Error type for failed downcasts
#[derive(Debug)]
pub struct DowncastError {
    pub expected_type: &'static str,
    pub actual_type_id: std::any::TypeId,
    pub context: String,
}

impl std::fmt::Display for DowncastError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Downcast failed: expected {}, got {:?} in context: {}",
            self.expected_type, self.actual_type_id, self.context
        )
    }
}

impl std::error::Error for DowncastError {}

impl SafeDowncast for dyn Any {
    fn safe_downcast_ref<U: Any>(&self) -> Result<&U, DowncastError> {
        self.downcast_ref::<U>().ok_or_else(|| DowncastError {
            expected_type: std::any::type_name::<U>(),
            actual_type_id: self.type_id(),
            context: String::new(),
        })
    }
    
    fn try_downcast_ref<U: Any>(&self) -> Option<&U> {
        self.downcast_ref::<U>()
    }
}

impl SafeDowncast for dyn Any + Send {
    fn safe_downcast_ref<U: Any>(&self) -> Result<&U, DowncastError> {
        self.downcast_ref::<U>().ok_or_else(|| DowncastError {
            expected_type: std::any::type_name::<U>(),
            actual_type_id: self.type_id(),
            context: String::new(),
        })
    }
    
    fn try_downcast_ref<U: Any>(&self) -> Option<&U> {
        self.downcast_ref::<U>()
    }
}

impl SafeDowncast for dyn Any + Send + Sync {
    fn safe_downcast_ref<U: Any>(&self) -> Result<&U, DowncastError> {
        self.downcast_ref::<U>().ok_or_else(|| DowncastError {
            expected_type: std::any::type_name::<U>(),
            actual_type_id: self.type_id(),
            context: String::new(),
        })
    }
    
    fn try_downcast_ref<U: Any>(&self) -> Option<&U> {
        self.downcast_ref::<U>()
    }
}

/// Extension trait for Collector-specific safe downcasting
pub trait CollectorDowncast {
    /// Safely downcast to Function with proper error context
    fn as_function(&self) -> Result<&Function, RtestError>;
    
    /// Try to get as Function
    fn try_as_function(&self) -> Option<&Function>;
}

impl<T: ?Sized + Collector> CollectorDowncast for T {
    fn as_function(&self) -> Result<&Function, RtestError> {
        self.as_any()
            .downcast_ref::<Function>()
            .ok_or_else(|| {
                RtestError::Collection(NewCollectionError::ParseError {
                    path: self.path().to_path_buf(),
                    line: None,
                    message: format!(
                        "Expected Function collector but got different type for nodeid: {}",
                        self.nodeid()
                    ),
                })
            })
    }
    
    fn try_as_function(&self) -> Option<&Function> {
        self.as_any().downcast_ref::<Function>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[derive(Debug)]
    struct TestType {
        value: i32,
    }
    
    #[test]
    fn test_safe_downcast() {
        let test_value = TestType { value: 42 };
        let any_ref: &dyn Any = &test_value;
        
        // Successful downcast
        let result = any_ref.safe_downcast_ref::<TestType>();
        assert!(result.is_ok());
        assert_eq!(result.unwrap().value, 42);
        
        // Failed downcast
        let result = any_ref.safe_downcast_ref::<String>();
        assert!(result.is_err());
    }
    
    #[test]
    fn test_try_downcast() {
        let test_value = TestType { value: 42 };
        let any_ref: &dyn Any = &test_value;
        
        // Successful try
        assert!(any_ref.try_downcast_ref::<TestType>().is_some());
        
        // Failed try
        assert!(any_ref.try_downcast_ref::<String>().is_none());
    }
}