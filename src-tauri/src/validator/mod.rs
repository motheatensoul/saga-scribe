//! # XML Validation Module
//!
//! This module provides TEI-XML validation against RelaxNG and XSD schemas
//! using libxml2 FFI bindings.
//!
//! ## Architecture
//!
//! ```text
//! Frontend ─▶ Tauri Command ─▶ ValidationActor ─▶ libxml2 (via FFI)
//!                                     │
//!                                     └─▶ Schema Cache
//! ```
//!
//! ## Components
//!
//! - **[`actor`]**: Actor-based async validation with schema caching
//! - **[`relaxng`]**: RelaxNG validation via FFI to libxml2
//! - **[`xsd`]**: XSD validation via FFI to libxml2
//! - **[`types`]**: Shared types ([`ValidationResult`], [`SchemaInfo`])
//!
//! ## Actor Pattern
//!
//! Validation runs on a dedicated thread via [`ValidationActor`](actor::ValidationActor):
//! - Caches parsed RelaxNG schemas for reuse
//! - Catches panics from libxml2 to prevent thread death
//! - Uses tokio oneshot channels for async response delivery
//!
//! ## FFI Safety
//!
//! The FFI wrappers handle:
//! - Structured error collection from libxml2
//! - Memory cleanup via Drop implementations
//! - Send/Sync traits for thread-safe schema sharing

pub mod actor;
pub mod relaxng;
pub mod types;
pub mod xsd;

pub use relaxng::RelaxNgValidator;
pub use types::{SchemaInfo, ValidationResult};
pub use xsd::XsdValidator;
