//! # Validation Actor
//!
//! This module implements an actor-based pattern for async XML validation.
//! The actor runs on a dedicated thread, processes validation requests via a channel,
//! and caches parsed schemas for performance.
//!
//! ## Why an Actor?
//!
//! 1. **Thread isolation**: libxml2 is not thread-safe; the actor serializes access
//! 2. **Schema caching**: Parsed schemas are expensive (`tei_all.xsd` takes seconds); we cache them
//! 3. **Coalescing**: Automatic validation runs after every edit. When several
//!    file-schema requests are queued, only the newest is validated; older ones
//!    are answered with [`SUPERSEDED`] so their results never overwrite newer ones
//! 4. **Panic safety**: `catch_unwind` prevents libxml2 panics from killing the thread
//! 5. **Async integration**: tokio oneshot channels bridge to async/await
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
use libxml::schemas::SchemaValidationContext;
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
    xsd_cache: HashMap<PathBuf, SchemaValidationContext>,
}

/// Error returned for a validation request that was skipped because a newer
/// request for a file schema was already queued behind it.
pub const SUPERSEDED: &str = "superseded by a newer validation request";

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
                    xsd_cache: HashMap::new(),
                };
                actor.run();
            })
            .expect("Failed to spawn validation actor thread");

        ValidationSender(tx)
    }

    fn run(&mut self) {
        while let Ok(first) = self.rx.recv() {
            for req in self.coalesce(first) {
                self.handle(req);
            }
        }
    }

    /// Collects everything already queued after `first` and drops all but the
    /// newest file-schema request. String-schema requests are kept in order.
    fn coalesce(&self, first: ValidationRequest) -> Vec<ValidationRequest> {
        let mut pending = vec![first];
        pending.extend(self.rx.try_iter());

        let newest_file_request = pending.iter().rposition(ValidationRequest::is_file_schema);
        let mut kept = Vec::with_capacity(pending.len());
        for (i, req) in pending.into_iter().enumerate() {
            if req.is_file_schema() && Some(i) != newest_file_request {
                req.reply_superseded();
            } else {
                kept.push(req);
            }
        }
        kept
    }

    fn handle(&mut self, req: ValidationRequest) {
        let start = std::time::Instant::now();

        // Each validation runs inside catch_unwind: the libxml crate panics on
        // internal libxml2 errors, and the actor thread must survive those.
        // AssertUnwindSafe is acceptable because a panicking validation leaves
        // no partially updated state behind; a cached schema is only read.
        match req {
            ValidationRequest::Xsd { xml, schema_path, reply } => {
                log::info!("Starting XSD validation against {:?}", schema_path);
                if !self.xsd_cache.contains_key(&schema_path) {
                    log::info!("Cache miss for XSD schema: {:?}", schema_path);
                    match XsdValidator::load_schema(&schema_path) {
                        Ok(ctx) => {
                            self.xsd_cache.insert(schema_path.clone(), ctx);
                        }
                        Err(e) => {
                            let _ = reply.send(Err(e));
                            return;
                        }
                    }
                }
                let schema_name = XsdValidator::schema_name(&schema_path);
                if let Some(ctx) = self.xsd_cache.get_mut(&schema_path) {
                    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                        XsdValidator::validate_with_context(ctx, &xml, &schema_name)
                    }));
                    if let Ok(res) = result {
                        log::info!("XSD validation took {:?}", start.elapsed());
                        let _ = reply.send(res);
                    } else {
                        log::error!("XSD validation panicked; dropping cached schema");
                        self.xsd_cache.remove(&schema_path);
                    }
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
                if !self.rng_cache.contains_key(&schema_path) {
                    log::info!("Cache miss for RelaxNG schema: {:?}", schema_path);
                    match RelaxNgSchema::parse_file(&schema_path) {
                        Ok(schema) => {
                            self.rng_cache.insert(schema_path.clone(), schema);
                        }
                        Err(e) => {
                            let _ = reply.send(Err(e));
                            return;
                        }
                    }
                }
                if let Some(schema) = self.rng_cache.get(&schema_path) {
                    let schema_name = schema_path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                        schema.validate(&xml, &schema_name)
                    }));
                    if let Ok(res) = result {
                        log::info!("RelaxNG validation took {:?}", start.elapsed());
                        let _ = reply.send(res);
                    } else {
                        log::error!("RelaxNG validation panicked; dropping cached schema");
                        self.rng_cache.remove(&schema_path);
                    }
                }
            }
            ValidationRequest::RelaxNgString { xml, schema_content, schema_name, reply } => {
                // Not cached: string schemas would need a content hash as key,
                // and they are only used for one-off validations.
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

impl ValidationRequest {
    /// Requests against schema files are the automatic per-edit validations.
    fn is_file_schema(&self) -> bool {
        matches!(self, Self::Xsd { .. } | Self::RelaxNg { .. })
    }

    fn reply_superseded(self) {
        let reply = match self {
            Self::Xsd { reply, .. }
            | Self::XsdString { reply, .. }
            | Self::RelaxNg { reply, .. }
            | Self::RelaxNgString { reply, .. } => reply,
        };
        let _ = reply.send(Err(SUPERSEDED.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rng_request(xml: &str) -> (ValidationRequest, oneshot::Receiver<Result<ValidationResult, String>>) {
        let (reply, rx) = oneshot::channel();
        let req = ValidationRequest::RelaxNg {
            xml: xml.to_string(),
            schema_path: PathBuf::from("/nonexistent.rng"),
            reply,
        };
        (req, rx)
    }

    #[test]
    fn test_coalesce_keeps_only_newest_file_request() {
        let (tx, rx) = channel();
        let actor = ValidationActor {
            rx,
            rng_cache: HashMap::new(),
            xsd_cache: HashMap::new(),
        };

        let (first, mut first_rx) = rng_request("<a/>");
        let (second, mut second_rx) = rng_request("<b/>");
        let (third, _third_rx) = rng_request("<c/>");
        tx.send(second).unwrap();
        tx.send(third).unwrap();

        let kept = actor.coalesce(first);

        assert_eq!(kept.len(), 1);
        assert!(matches!(&kept[0], ValidationRequest::RelaxNg { xml, .. } if xml == "<c/>"));
        assert_eq!(first_rx.try_recv().unwrap().unwrap_err(), SUPERSEDED);
        assert_eq!(second_rx.try_recv().unwrap().unwrap_err(), SUPERSEDED);
    }
}
