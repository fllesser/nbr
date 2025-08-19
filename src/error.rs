//! Error handling module for nbr
//!
//! This module defines custom error types used throughout the CLI application.
#![allow(dead_code)]

use thiserror::Error;

/// Main error type for the nbr application
#[derive(Error, Debug)]
pub enum NbrError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("TOML parsing error: {0}")]
    TomlDeserialize(#[from] toml::de::Error),

    #[error("TOML serialization error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

    #[error("Template error: {message}")]
    Template { message: String },

    #[error("Project creation error: {message}")]
    ProjectCreation { message: String },

    #[error("Plugin error: {message}")]
    Plugin { message: String },

    #[error("Adapter error: {message}")]
    Adapter { message: String },

    #[error("Configuration error: {message}")]
    Config { message: String },

    #[error("Environment error: {message}")]
    Environment { message: String },

    #[error("Command execution error: {command} failed with exit code {exit_code}")]
    CommandExecution { command: String, exit_code: i32 },

    #[error("Invalid argument: {message}")]
    InvalidArgument { message: String },

    #[error("Resource not found: {resource}")]
    NotFound { resource: String },

    #[error("Permission denied: {message}")]
    PermissionDenied { message: String },

    #[error("Already exists: {resource}")]
    AlreadyExists { resource: String },

    #[error("Validation error: {message}")]
    Validation { message: String },

    #[error("Cache error: {message}")]
    Cache { message: String },

    #[error("Archive error: {message}")]
    Archive { message: String },

    #[error("Unknown error: {message}")]
    Unknown { message: String },
}

impl NbrError {
    /// Create a new template error
    pub fn template<S: Into<String>>(message: S) -> Self {
        Self::Template {
            message: message.into(),
        }
    }

    /// Create a new project creation error
    pub fn project_creation<S: Into<String>>(message: S) -> Self {
        Self::ProjectCreation {
            message: message.into(),
        }
    }

    /// Create a new plugin error
    pub fn plugin<S: Into<String>>(message: S) -> Self {
        Self::Plugin {
            message: message.into(),
        }
    }

    /// Create a new adapter error
    pub fn adapter<S: Into<String>>(message: S) -> Self {
        Self::Adapter {
            message: message.into(),
        }
    }

    /// Create a new network error from reqwest error
    pub fn network(err: reqwest::Error) -> Self {
        Self::Network(err)
    }

    /// Create a new IO error
    pub fn io<S: Into<String>>(message: S) -> Self {
        Self::Io(std::io::Error::other(message.into()))
    }

    /// Create a new git error from git2 error
    pub fn git(err: git2::Error) -> Self {
        Self::Git(err)
    }

    /// Create a new configuration error
    pub fn config<S: Into<String>>(message: S) -> Self {
        Self::Config {
            message: message.into(),
        }
    }

    /// Create a new environment error
    pub fn environment<S: Into<String>>(message: S) -> Self {
        Self::Environment {
            message: message.into(),
        }
    }

    /// Create a new command execution error
    pub fn command_execution<S: Into<String>>(command: S, exit_code: i32) -> Self {
        Self::CommandExecution {
            command: command.into(),
            exit_code,
        }
    }

    /// Create a new invalid argument error
    pub fn invalid_argument<S: Into<String>>(message: S) -> Self {
        Self::InvalidArgument {
            message: message.into(),
        }
    }

    /// Create a new not found error
    pub fn not_found<S: Into<String>>(resource: S) -> Self {
        Self::NotFound {
            resource: resource.into(),
        }
    }

    /// Create a new permission denied error
    pub fn permission_denied<S: Into<String>>(message: S) -> Self {
        Self::PermissionDenied {
            message: message.into(),
        }
    }

    /// Create a new already exists error
    pub fn already_exists<S: Into<String>>(resource: S) -> Self {
        Self::AlreadyExists {
            resource: resource.into(),
        }
    }

    /// Create a new validation error
    pub fn validation<S: Into<String>>(message: S) -> Self {
        Self::Validation {
            message: message.into(),
        }
    }

    /// Create a new cache error
    pub fn cache<S: Into<String>>(message: S) -> Self {
        Self::Cache {
            message: message.into(),
        }
    }

    /// Create a new archive error
    pub fn archive<S: Into<String>>(message: S) -> Self {
        Self::Archive {
            message: message.into(),
        }
    }

    /// Create a new unknown error
    pub fn unknown<S: Into<String>>(message: S) -> Self {
        Self::Unknown {
            message: message.into(),
        }
    }

    /// Check if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::Io(_) => false,
            Self::Network(_) => true,
            Self::Serialization(_) => false,
            // Self::Yaml(_) => false,
            Self::TomlDeserialize(_) | Self::TomlSerialize(_) => false,
            Self::Git(_) => true,
            Self::Template { .. } => true,
            Self::ProjectCreation { .. } => true,
            Self::Plugin { .. } => true,
            Self::Adapter { .. } => true,
            Self::Config { .. } => true,
            Self::Environment { .. } => true,
            Self::CommandExecution { .. } => true,
            Self::InvalidArgument { .. } => false,
            Self::NotFound { .. } => true,
            Self::PermissionDenied { .. } => false,
            Self::AlreadyExists { .. } => true,
            Self::Validation { .. } => false,
            Self::Cache { .. } => true,
            Self::Archive { .. } => true,
            Self::Unknown { .. } => false,
        }
    }

    /// Get error category for logging and metrics
    pub fn category(&self) -> &'static str {
        match self {
            Self::Io(_) => "io",
            Self::Network(_) => "network",
            Self::Serialization(_) | Self::TomlDeserialize(_) | Self::TomlSerialize(_) => {
                "serialization"
            }
            Self::Git(_) => "git",
            Self::Template { .. } => "template",
            Self::ProjectCreation { .. } => "project",
            Self::Plugin { .. } => "plugin",
            Self::Adapter { .. } => "adapter",
            Self::Config { .. } => "config",
            Self::Environment { .. } => "environment",
            Self::CommandExecution { .. } => "command",
            Self::InvalidArgument { .. } => "argument",
            Self::NotFound { .. } => "not_found",
            Self::PermissionDenied { .. } => "permission",
            Self::AlreadyExists { .. } => "exists",
            Self::Validation { .. } => "validation",
            Self::Cache { .. } => "cache",
            Self::Archive { .. } => "archive",
            Self::Unknown { .. } => "unknown",
        }
    }
}

/// Result type alias for convenience
pub type Result<T> = std::result::Result<T, NbrError>;

/// Convert anyhow::Error to NbrError
impl From<anyhow::Error> for NbrError {
    fn from(err: anyhow::Error) -> Self {
        Self::Unknown {
            message: err.to_string(),
        }
    }
}

/// Helper trait for converting Results
pub trait NbrResultExt<T> {
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String;
}

impl<T, E> NbrResultExt<T> for std::result::Result<T, E>
where
    E: Into<NbrError>,
{
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| {
            let base_err = e.into();
            let context = f();
            match base_err {
                NbrError::Unknown { .. } => NbrError::Unknown { message: context },
                _ => NbrError::Unknown {
                    message: format!("{}: {}", context, base_err),
                },
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = NbrError::template("Test template error");
        assert_eq!(err.to_string(), "Template error: Test template error");
        assert_eq!(err.category(), "template");
        assert!(err.is_recoverable());
    }

    #[test]
    fn test_error_categories() {
        assert_eq!(NbrError::plugin("test").category(), "plugin");
        assert_eq!(NbrError::adapter("test").category(), "adapter");
        assert_eq!(NbrError::config("test").category(), "config");
    }
}
