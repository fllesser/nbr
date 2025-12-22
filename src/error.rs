//! Error handling module for nbr
//!
//! This module defines custom error types used throughout the CLI application.
use thiserror::Error;

/// Main error type for the nbr application
#[derive(Error, Debug)]
pub enum Error {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Command execution error: {command} failed with exit code {exit_code}")]
    CommandExecution { command: String, exit_code: i32 },

    #[error("Operation cancelled")]
    Cancelled,
}

impl Error {
    /// Check if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::Network(_) => true,
            Self::CommandExecution { .. } => true,
            Self::Cancelled => false,
        }
    }

    /// Get error category for logging and metrics
    pub fn category(&self) -> &'static str {
        match self {
            Self::Network(_) => "network",
            Self::CommandExecution { .. } => "command",
            Self::Cancelled => "cancelled",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = Error::Cancelled;
        assert_eq!(err.to_string(), "Operation cancelled");
        assert_eq!(err.category(), "cancelled");
        assert!(!err.is_recoverable());
    }

    #[test]
    fn test_error_categories() {
        assert_eq!(Error::Cancelled.category(), "cancelled");
    }
}
