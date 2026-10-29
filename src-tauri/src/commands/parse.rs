use crate::annotations::AnnotationSet;
use crate::entities::EntityRegistry;
use crate::importer::tei::patching::{compiler_config_for, compute_patches, render_body};
use crate::importer::tei::segments::ImportedDocument;
use crate::normalizer::LevelDictionary;
use crate::parser::{Compiler, CompilerConfig, LemmaMapping};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::State;

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

/// The imported document being edited, held in the backend.
///
/// The segment manifest of a large manuscript is tens of megabytes of JSON.
/// Keeping it here means it crosses IPC once (via [`set_imported_document`])
/// instead of with every compile.
#[derive(Default, Clone)]
pub struct ImportedSession(Arc<Mutex<Option<ImportedSource>>>);

/// Everything needed to compile edited DSL back into the original file.
pub struct ImportedSource {
    document: ImportedDocument,
    original_body_xml: String,
    preamble: String,
    postamble: String,
}

/// Sets (or, with no manifest, clears) the imported document used by [`compile_imported`].
#[tauri::command(async, rename_all = "camelCase")]
pub async fn set_imported_document(
    session: State<'_, ImportedSession>,
    segments_json: Option<String>,
    original_body_xml: Option<String>,
    preamble: Option<String>,
    postamble: Option<String>,
) -> Result<(), String> {
    let source = match segments_json {
        Some(json) => Some(
            tauri::async_runtime::spawn_blocking(move || {
                ImportedDocument::from_json(&json).map(|document| ImportedSource {
                    document,
                    original_body_xml: original_body_xml.unwrap_or_default(),
                    preamble: preamble.unwrap_or_default(),
                    postamble: postamble.unwrap_or_default(),
                })
            })
            .await
            .map_err(|e| format!("Loading imported document failed: {}", e))??,
        ),
        None => None,
    };
    *session.0.lock().map_err(|_| "Imported document state is poisoned")? = source;
    Ok(())
}

/// Compile an imported document using the patching system for round-trip fidelity.
///
/// Takes the edited DSL, computes the differences against the imported
/// document held in [`ImportedSession`], and splices them into the original
/// XML. Unedited content is emitted byte-for-byte.
///
/// # Errors
///
/// Returns an error if no imported document is loaded or the edited DSL does
/// not parse; callers must keep the previous output rather than writing anything.
#[tauri::command(async, rename_all = "camelCase")]
pub async fn compile_imported(
    session: State<'_, ImportedSession>,
    edited_dsl: String,
    entities_json: Option<String>,
    normalizer_json: Option<String>,
    entity_mappings_json: Option<String>,
    custom_mappings: Option<HashMap<String, String>>,
) -> Result<String, String> {
    let session = session.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let guard = session
            .0
            .lock()
            .map_err(|_| "Imported document state is poisoned")?;
        let source = guard.as_ref().ok_or("No imported document is loaded")?;

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

        // Match the document's own encoding style
        let config = compiler_config_for(&source.document);

        let mut compiler = Compiler::new()
            .with_entities(&registry)
            .with_config(config);

        if let Some(ref dict) = dictionary {
            compiler = compiler.with_dictionary(dict);
        }

        // Compute patches between original segments and edited DSL
        let patches = compute_patches(&source.document.segments, &edited_dsl)?;

        // Apply patches to the original body XML
        let body = render_body(
            &source.document,
            Some(&source.original_body_xml),
            &patches,
            &mut compiler,
        )?;

        // Combine with preamble (everything before body) and postamble (everything after)
        Ok(format!("{}{}{}", source.preamble, body, source.postamble))
    })
    .await
    .map_err(|e| format!("Compilation task failed: {}", e))?
}

/// A DSL syntax error, positioned for the editor.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct DslDiagnostic {
    /// Start offset in UTF-16 code units (the editor's positions)
    pub from: usize,
    /// End offset in UTF-16 code units, at most the end of the starting line
    pub to: usize,
    pub message: String,
}

/// Reports every syntax error in DSL source, for the editor's linter.
#[tauri::command(async)]
pub async fn check_dsl(input: String) -> Vec<DslDiagnostic> {
    dsl_diagnostics(&input)
}

fn dsl_diagnostics(input: &str) -> Vec<DslDiagnostic> {
    let utf16 = |byte: usize| input[..byte].encode_utf16().count();
    crate::parser::syntax::parse(input)
        .diagnostics
        .into_iter()
        .map(|d| {
            // An unclosed bracket's span runs to the end of the document;
            // underlining the rest of its line is enough to point at it.
            let line_end = input[d.span.start..]
                .find('\n')
                .map_or(input.len(), |i| d.span.start + i);
            let end = d.span.end.min(line_end).max(d.span.start + 1).min(input.len());
            DslDiagnostic {
                from: utf16(d.span.start),
                to: utf16(end),
                message: d.message,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dsl_diagnostics_use_utf16_offsets_and_stay_on_one_line() {
        // "𝔞" is outside the BMP: 4 bytes, 2 UTF-16 units
        let input = "𝔞 -{open\nnext line";
        let diagnostics = dsl_diagnostics(input);
        assert_eq!(
            diagnostics,
            vec![DslDiagnostic {
                from: 3,
                to: 9,
                message: "Unclosed bracket, expected '}'".to_string(),
            }]
        );
    }

    #[test]
    fn test_dsl_diagnostics_empty_for_valid_input() {
        assert!(dsl_diagnostics(".head{//1 a :et:} -{x}-").is_empty());
    }
}
