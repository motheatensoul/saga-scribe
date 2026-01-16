use crate::annotations::AnnotationSet;
use crate::entities::EntityRegistry;
use crate::importer::tei::patching::{apply_patches_and_reconstruct, compute_patches};
use crate::importer::tei::segments::{ImportedDocument, Segment};
use crate::normalizer::LevelDictionary;
use crate::parser::{Compiler, CompilerConfig, LemmaMapping};
use std::collections::HashMap;

/// Compile DSL input to TEI-XML.
///
/// This command is async to avoid blocking the UI during compilation.
/// The actual compilation runs on a blocking thread pool.
#[allow(clippy::too_many_arguments)]
#[tauri::command(async, rename_all = "camelCase")]
pub async fn compile_dsl(
    input: String,
    template_header: String,
    template_footer: String,
    word_wrap: Option<bool>,
    auto_line_numbers: Option<bool>,
    multi_level: Option<bool>,
    wrap_pages: Option<bool>,
    entities_json: Option<String>,
    normalizer_json: Option<String>,
    entity_mappings_json: Option<String>,
    custom_mappings: Option<HashMap<String, String>>,
    lemma_mappings_json: Option<String>,
    annotations_json: Option<String>,
) -> Result<String, String> {
    // Move all the work to a blocking thread pool to avoid blocking the UI
    tauri::async_runtime::spawn_blocking(move || {
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
            Some(json) => serde_json::from_str(&json)
                .map_err(|e| format!("Failed to parse lemma mappings: {}", e))?,
            None => HashMap::new(),
        };

        // Parse annotations if provided
        let annotations: Option<AnnotationSet> = match annotations_json {
            Some(json) => Some(
                serde_json::from_str(&json)
                    .map_err(|e| format!("Failed to parse annotations: {}", e))?,
            ),
            None => None,
        };

        // Configure compiler
        let config = CompilerConfig {
            word_wrap: word_wrap.unwrap_or(false),
            auto_line_numbers: auto_line_numbers.unwrap_or(false),
            multi_level: multi_level.unwrap_or(false),
            wrap_pages: wrap_pages.unwrap_or(false),
        };

        let mut compiler = Compiler::new()
            .with_entities(&registry)
            .with_config(config)
            .with_lemma_mappings(lemma_mappings);

        // Add annotations if available
        if let Some(ref ann) = annotations {
            compiler = compiler.with_annotations(ann);
        }

        // Add dictionary if available
        if let Some(ref dict) = dictionary {
            compiler = compiler.with_dictionary(dict);
        }

        let body = compiler.compile(&input)?;
        Ok(format!("{}\n{}\n{}", template_header, body, template_footer))
    })
    .await
    .map_err(|e| format!("Compilation task failed: {}", e))?
}

/// Compile an imported document using the patching system for round-trip fidelity.
///
/// This command takes edited DSL, the original segment manifest, and original body XML,
/// computes the differences, and reconstructs the XML with minimal changes.
#[allow(clippy::too_many_arguments)]
#[tauri::command(async, rename_all = "camelCase")]
pub async fn compile_imported(
    edited_dsl: String,
    segments_json: String,
    preamble: String,
    postamble: String,
    entities_json: Option<String>,
    normalizer_json: Option<String>,
    entity_mappings_json: Option<String>,
    custom_mappings: Option<HashMap<String, String>>,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        // Deserialize the imported document (accept either full manifest or raw segments list)
        let imported_doc: ImportedDocument = match serde_json::from_str(&segments_json) {
            Ok(doc) => doc,
            Err(_) => {
                let segments: Vec<Segment> = serde_json::from_str(&segments_json)
                    .map_err(|e| format!("Failed to parse segments: {}", e))?;
                ImportedDocument {
                    segments,
                    is_menota: false,
                }
            }
        };
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
            if let Some(custom) = custom_mappings {
                dict.add_entity_mappings(custom);
            }
        }

        // Configure compiler for multi-level output
        let config = CompilerConfig {
            word_wrap: true,
            auto_line_numbers: false,
            multi_level: true,
            wrap_pages: false,
        };

        let mut compiler = Compiler::new()
            .with_entities(&registry)
            .with_config(config);

        if let Some(ref dict) = dictionary {
            compiler = compiler.with_dictionary(dict);
        }

        // Compute patches between original segments and edited DSL
        let patches = compute_patches(&imported_doc.segments, &edited_dsl);

        // Apply patches and reconstruct the body XML
        let body = apply_patches_and_reconstruct(&imported_doc.segments, &patches, &mut compiler);

        // Combine with preamble (everything before body) and postamble (everything after)
        Ok(format!("{}{}{}", preamble, body, postamble))
    })
    .await
    .map_err(|e| format!("Compilation task failed: {}", e))?
}
