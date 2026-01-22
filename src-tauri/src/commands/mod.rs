//! # Tauri Commands Module
//!
//! This module contains all Tauri commands that bridge the frontend UI with Rust backend
//! functionality. Commands are invoked from the frontend via Tauri's IPC system.
//!
//! ## Command Categories
//!
//! - **[`file`]**: Project file operations (open, save, export)
//! - **[`parse`]**: DSL compilation (`compile_dsl`, `compile_imported`)
//! - **[`import`]**: TEI-XML import (`import_file`)
//! - **[`validate`]**: XML validation against RelaxNG/XSD schemas
//! - **[`template`]**: Template management for TEI headers
//! - **[`entities`]**: Entity registry loading
//! - **[`dictionary`]**: ONP dictionary lookup
//! - **[`settings`]**: User settings persistence
//! - **[`stylesheet`]**: XSLT stylesheet management
//! - **[`metadata`]**: TEI header metadata generation
//!
//! ## Async Patterns
//!
//! Commands that perform I/O or CPU-intensive work use async patterns:
//!
//! ```rust,ignore
//! #[tauri::command(async)]
//! pub async fn example() -> Result<String, String> {
//!     tauri::async_runtime::spawn_blocking(move || {
//!         // CPU-bound or blocking I/O work here
//!         Ok("result".to_string())
//!     })
//!     .await
//!     .map_err(|e| e.to_string())?
//! }
//! ```
//!
//! This pattern prevents UI freezing by:
//! 1. `#[tauri::command(async)]` - Command runs off main thread
//! 2. `spawn_blocking` - CPU-bound work runs on a thread pool

pub mod dictionary;
pub mod entities;
pub mod file;
pub mod import;
pub mod metadata;
pub mod parse;
pub mod settings;
pub mod stylesheet;
pub mod template;
pub mod validate;
