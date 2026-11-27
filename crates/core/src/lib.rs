//! Ultron's core: everything that is not a window. Ported unit by unit from the
//! Python original (../ultron), each checked against its recordings (rust/MAP.md).
pub mod commands;
pub mod config;
pub mod ipc;
pub mod memory;
pub mod paths;
pub mod reports;
pub mod router;
pub mod sessions;
pub mod tasks;
