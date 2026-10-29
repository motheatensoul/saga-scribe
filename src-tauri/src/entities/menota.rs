//! The bundled MENOTA/MUFI entity list (`static/entities/menota.json`),
//! compiled into the binary for the backend's own use: which entities are
//! punctuation, and what character each one stands for.

use std::collections::HashMap;
use std::sync::OnceLock;

const MENOTA_ENTITIES: &str = include_str!("../../../static/entities/menota.json");

/// One entry of the bundled entity list.
pub struct BundledEntity {
    /// The character(s) the entity stands for
    pub text: String,
    /// MENOTA's category, e.g. "punctuation"
    pub category: String,
}

/// The bundled entities, by name.
pub fn bundled() -> &'static HashMap<String, BundledEntity> {
    static ENTITIES: OnceLock<HashMap<String, BundledEntity>> = OnceLock::new();
    ENTITIES.get_or_init(|| {
        let parsed: Option<serde_json::Value> = serde_json::from_str(MENOTA_ENTITIES).ok();
        let Some(entities) = parsed.as_ref().and_then(|v| v.get("entities")).and_then(|e| e.as_object()) else {
            return HashMap::new();
        };
        entities
            .iter()
            .map(|(name, entity)| {
                let field = |key: &str| entity.get(key).and_then(|v| v.as_str()).unwrap_or_default().to_string();
                (name.clone(), BundledEntity { text: field("char"), category: field("category") })
            })
            .collect()
    })
}
