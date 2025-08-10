//! CLI command handlers module
//!
//! This module contains all the command handlers for the nb-cli tool.

pub mod adapter;
pub mod cache;
pub mod create;
pub mod env;
pub mod generate;
pub mod init;
pub mod plugin;
pub mod run;
mod pyproject;

// Re-export handler functions for convenience
