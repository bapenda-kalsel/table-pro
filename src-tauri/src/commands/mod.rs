//! Tauri command handlers, split by domain.
//! Each module is a thin layer: resolve the driver from AppState, delegate to
//! the `DatabaseDriver` trait, and record the call in the SQL audit log.

pub mod ai;
pub mod audit;
pub mod connection;
pub mod data;
pub mod ops;
pub mod schema;
pub mod snippets;
