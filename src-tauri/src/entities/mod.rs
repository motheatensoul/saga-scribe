mod custom_entities;
mod custom_mappings;
mod menota;
mod punctuation;
mod registry;

pub use custom_entities::CustomEntitiesManager;
pub use custom_mappings::CustomMappingsManager;
pub use menota::{bundled, BundledEntity};
pub use punctuation::is_punctuation_entity;
pub use registry::{Entity, EntityRegistry};

/// The document type declaration Menota files use: the MENOTA/MUFI entities
/// (`&thorn;`, `&middot;`, …) are declared in Menota's external entity list.
/// It is never fetched (nor needed: the app resolves entities itself), but
/// declaring it makes a document using those entities well-formed XML.
pub const MENOTA_DOCTYPE: &str = r#"<!DOCTYPE TEI [
<!ENTITY % Menota_entities SYSTEM "http://www.menota.org/menota-entities.txt">
%Menota_entities;
]>"#;
