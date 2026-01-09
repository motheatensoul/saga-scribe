use crate::entities::EntityRegistry;
use crate::normalizer::LevelDictionary;
use crate::parser::{Compiler, CompilerConfig, LemmaMapping};
use std::collections::HashMap;

// Too many arguments for clippy, consolidate into a template-struct if compatible with tauri.
#[tauri::command]
pub fn compile_dsl(
    input: String,
    template_header: String,
    template_footer: String,
    word_wrap: Option<bool>,
    auto_line_numbers: Option<bool>,
    multi_level: Option<bool>,
    entities_json: Option<String>,
    normalizer_json: Option<String>,
    entity_mappings_json: Option<String>,
    custom_mappings: Option<HashMap<String, String>>,
    lemma_mappings_json: Option<String>,
) -> Result<String, String> {
    // Load entities if provided
    let mut registry = EntityRegistry::new();
    if let Some(json) = entities_json {
        registry.load_from_str(&json)?;
    }

    // Load level dictionary if provided
    let mut dictionary = match normalizer_json {
        Some(json) => Some(LevelDictionary::load(&json)?),
        None => None,
    };

    // Load entity base letter mappings if provided
    if let Some(ref mut dict) = dictionary {
        if let Some(ref json) = entity_mappings_json {
            dict.load_entity_mappings(json)?;
        }
        // Apply custom mappings (overrides base mappings)
        if let Some(custom) = custom_mappings {
            dict.add_entity_mappings(custom);
        }
    }

    // Parse lemma mappings if provided (keyed by word INDEX)
    let lemma_mappings: HashMap<u32, LemmaMapping> = match lemma_mappings_json {
        Some(json) => serde_json::from_str(&json).map_err(|e| format!("Failed to parse lemma mappings: {}", e))?,
        None => HashMap::new(),
    };

    // Configure compiler
    let config = CompilerConfig {
        word_wrap: word_wrap.unwrap_or(false),
        auto_line_numbers: auto_line_numbers.unwrap_or(false),
        multi_level: multi_level.unwrap_or(false),
    };

    let mut compiler = Compiler::new()
        .with_entities(&registry)
        .with_config(config)
        .with_lemma_mappings(lemma_mappings);

    // Add dictionary if available
    if let Some(ref dict) = dictionary {
        compiler = compiler.with_dictionary(dict);
    }

    let body = compiler.compile(&input)?;
    Ok(format!("{}\n{}\n{}", template_header, body, template_footer))
}
