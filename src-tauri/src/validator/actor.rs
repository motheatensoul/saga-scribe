//! # Validation Actor
//!
//! This module implements an actor-based pattern for async XML validation.
//! The actor runs on a dedicated thread, processes validation requests via a channel,
//! and caches parsed schemas for performance.
//!
//! ## Why an Actor?
//!
//! 1. **Thread isolation**: libxml2 is not thread-safe; the actor serializes access
//! 2. **Schema caching**: Parsed RelaxNG schemas are expensive; we cache them
//! 3. **Panic safety**: `catch_unwind` prevents libxml2 panics from killing the thread
//! 4. **Async integration**: tokio oneshot channels bridge to async/await
//!
//! ## Request Flow
//!
//! ```text
//! async fn validate_xml() {
//!     let (tx, rx) = oneshot::channel();
//!     actor.send(ValidationRequest::RelaxNg { xml, schema_path, reply: tx });
//!     let result = rx.await?;  // Blocks async task, not OS thread
//! }
//! ```

use super::{RelaxNgValidator, XsdValidator};
use crate::validator::relaxng::RelaxNgSchema;
use crate::validator::ValidationResult;
use std::collections::HashMap;
use std::panic;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use tokio::sync::oneshot;

/// Validation actor that runs on a dedicated thread.
///
/// The actor maintains a cache of parsed RelaxNG schemas and processes
/// validation requests sequentially from a channel.
pub struct ValidationActor {
    rx: Receiver<ValidationRequest>,
    rng_cache: HashMap<PathBuf, RelaxNgSchema>,
    // TODO: Add XSD cache when we implement XSD reuse
}

/// A request to validate XML against a schema.
///
/// Each variant includes a `reply` channel for async response delivery.
pub enum ValidationRequest {
    /// Validate against XSD schema from file path
    Xsd {
        xml: String,
        schema_path: PathBuf,
        reply: oneshot::Sender<Result<ValidationResult, String>>,
    },
    /// Validate against XSD schema from string content
    XsdString {
        xml: String,
        schema_content: String,
        schema_name: String,
        reply: oneshot::Sender<Result<ValidationResult, String>>,
    },
    /// Validate against RelaxNG schema from file path (cached)
    RelaxNg {
        xml: String,
        schema_path: PathBuf,
        reply: oneshot::Sender<Result<ValidationResult, String>>,
    },
    /// Validate against RelaxNG schema from string content (not cached)
    RelaxNgString {
        xml: String,
        schema_content: String,
        schema_name: String,
        reply: oneshot::Sender<Result<ValidationResult, String>>,
    },
}

/// Handle for sending validation requests to the actor.
///
/// This can be cloned and shared across threads.
#[derive(Clone)]
pub struct ValidationSender(pub Sender<ValidationRequest>);

impl ValidationActor {
    /// Spawns the validation actor on a new thread.
    ///
    /// Returns a [`ValidationSender`] that can be used to submit validation requests.
    /// The actor thread runs until the sender is dropped and all requests are processed.
    pub fn spawn() -> ValidationSender {
        let (tx, rx) = channel();

        thread::Builder::new()
            .name("validation-actor".into())
            .spawn(move || {
                let mut actor = ValidationActor {
                    rx,
                    rng_cache: HashMap::new(),
                };
                actor.run();
            })
            .expect("Failed to spawn validation actor thread");

        ValidationSender(tx)
    }

    fn run(&mut self) {
        while let Ok(req) = self.rx.recv() {
            let start = std::time::Instant::now();
            
            //FIXME The comment below reads like Opus thought output, not actual documentation. Redo.
            
            // We use catch_unwind to ensure that if libxml panics (e.g. on internal error),
            // the actor thread doesn't die and can continue processing requests.
            // Note: We need to move the cache OUT of the catch_unwind closure 
            // if we want to modify it, but catch_unwind requires UnwindSafe.
            // HashMap and RelaxNgSchema (raw ptr) are not UnwindSafe by default.
            // So we handle the request logic first, then validate inside catch_unwind?
            // Or we just accept that if it panics, we might lose cache state if we were mutating it?
            // Actually, we can use AssertUnwindSafe on &mut self.
            
            // However, we need to split the borrowing.
            // Let's match first.
            
            match req {
                ValidationRequest::Xsd { xml, schema_path, reply } => {
                    log::info!("Starting XSD validation against {:?}", schema_path);
                    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                        XsdValidator::validate(&xml, &schema_path)
                    }));
                    if let Ok(res) = result {
                        log::info!("XSD validation took {:?}", start.elapsed());
                        let _ = reply.send(res);
                    } else {
                        log::error!("XSD validation panicked");
                    }
                }
                ValidationRequest::XsdString { xml, schema_content, schema_name, reply } => {
                    log::info!("Starting XSD string validation: {}", schema_name);
                    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                        XsdValidator::validate_with_schema_string(&xml, &schema_content, &schema_name)
                    }));
                    if let Ok(res) = result {
                        log::info!("XSD string validation took {:?}", start.elapsed());
                        let _ = reply.send(res);
                    } else {
                        log::error!("XSD string validation panicked");
                    }
                }
                ValidationRequest::RelaxNg { xml, schema_path, reply } => {
                    log::info!("Starting RelaxNG validation against {:?}", schema_path);
                    
                    // Check cache first
                    if !self.rng_cache.contains_key(&schema_path) {
                        log::info!("Cache miss for RelaxNG schema: {:?}", schema_path);
                        match RelaxNgSchema::parse_file(&schema_path) {
                            Ok(schema) => {
                                self.rng_cache.insert(schema_path.clone(), schema);
                            }
                            Err(e) => {
                                let _ = reply.send(Err(e));
                                continue;
                            }
                        }
                    } else {
                        log::info!("Cache hit for RelaxNG schema: {:?}", schema_path);
                    }

                    // Get schema from cache
                    if let Some(schema) = self.rng_cache.get(&schema_path) {
                        let schema_name = schema_path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                        
                        let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                            schema.validate(&xml, &schema_name)
                        }));
                        
                        if let Ok(res) = result {
                            log::info!("RelaxNG validation took {:?}", start.elapsed());
                            let _ = reply.send(res);
                        } else {
                            log::error!("RelaxNG validation panicked");
                            // If it panicked, maybe the schema is corrupted? 
                            // We could remove it from cache, but it's risky.
                        }
                    }
                }
                ValidationRequest::RelaxNgString { xml, schema_content, schema_name, reply } => {
                    // String schemas are harder to cache effectively without a hash of content.
                    // For now, we won't cache strings, assuming they are rare or small.
                    log::info!("Starting RelaxNG string validation: {}", schema_name);
                    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                        RelaxNgValidator::validate_with_schema_string(&xml, &schema_content, &schema_name)
                    }));
                    if let Ok(res) = result {
                        log::info!("RelaxNG string validation took {:?}", start.elapsed());
                        let _ = reply.send(res);
                    } else {
                        log::error!("RelaxNG string validation panicked");
                    }
                }
            }
        }
    }
}
