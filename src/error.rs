//! Error types for the KeraDB Rust SDK.

// @group ErrorTypes : Error definitions and conversions for KeraDB SDK

use thiserror::Error;

/// Primary error type for all KeraDB operations.
#[derive(Debug, Error)]
pub enum KeraDbError {
    /// The native library could not be loaded.
    #[error("Failed to load KeraDB native library: {0}")]
    LibraryLoad(String),

    /// A native FFI call returned an error string.
    #[error("KeraDB error: {0}")]
    Native(String),

    /// A JSON serialization or deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// A string contained invalid UTF-8.
    #[error("UTF-8 encoding error: {0}")]
    Utf8(String),

    /// A null pointer was returned where a valid pointer was expected.
    #[error("Null pointer returned from native call: {0}")]
    NullPointer(String),

    /// An operation was attempted on a closed database.
    #[error("Database is closed")]
    Closed,

    /// A generic, context-free error string.
    #[error("{0}")]
    Other(String),
}

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, KeraDbError>;

// ---------------------------------------------------------------------------
// @group UnitTests : Error type formatting and conversions
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_load_error_message() {
        let e = KeraDbError::LibraryLoad("not found".into());
        assert!(e.to_string().contains("not found"));
    }

    #[test]
    fn native_error_message() {
        let e = KeraDbError::Native("disk full".into());
        assert_eq!(e.to_string(), "KeraDB error: disk full");
    }

    #[test]
    fn json_error_from_serde() {
        let raw = serde_json::from_str::<serde_json::Value>("{bad");
        let e: KeraDbError = raw.unwrap_err().into();
        assert!(e.to_string().contains("JSON error"));
    }

    #[test]
    fn utf8_error_message() {
        let e = KeraDbError::Utf8("invalid sequence".into());
        assert!(e.to_string().contains("UTF-8"));
    }

    #[test]
    fn null_pointer_error_message() {
        let e = KeraDbError::NullPointer("insert returned null".into());
        assert!(e.to_string().contains("Null pointer"));
    }

    #[test]
    fn closed_error_message() {
        let e = KeraDbError::Closed;
        assert_eq!(e.to_string(), "Database is closed");
    }

    #[test]
    fn other_error_message() {
        let e = KeraDbError::Other("something went wrong".into());
        assert_eq!(e.to_string(), "something went wrong");
    }

    #[test]
    fn result_ok_propagates() {
        let r: Result<i32> = Ok(42);
        assert_eq!(r.unwrap(), 42);
    }

    #[test]
    fn result_err_propagates() {
        let r: Result<i32> = Err(KeraDbError::Closed);
        assert!(r.is_err());
    }
}
