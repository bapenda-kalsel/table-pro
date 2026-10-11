//! Application services — the layer between Tauri commands (transport)
//! and drivers/vault (capabilities). Policy lives here, not in commands
//! or drivers.

pub mod ai;
pub mod audit;
pub mod connections;
pub mod security;
