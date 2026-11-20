use crate::commands::parse::{ImportedSession, ImportedSource, StagedDocument};
use crate::importer::tei::patching::Originals;
use crate::importer::tei::words::{imported_words, ImportedWord};
use crate::importer::tei::{self, ImportResult};
use crate::metadata::Metadata;
use std::fs;
use std::path::Path;
use tauri::State;

/// An imported file, as the editor gets it: the document itself stays in
/// the backend, staged (see [`ImportedSession`]).
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReply {
    pub dsl: String,
    pub metadata: Option<Metadata>,
    pub is_imported_mode: bool,
    /// The staged document (imported mode)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imported: Option<StagedDocument>,
    /// The file's words with their lemmas and levels (imported mode)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub words: Option<Vec<ImportedWord>>,
}

/// Import a file and convert it to DSL format, also extracting metadata if available.
///
/// This command is async, which means Tauri executes it on a separate async task
/// (not the main thread), preventing UI blocking. The actual file I/O and parsing
/// runs on a blocking thread pool via spawn_blocking.
#[tauri::command(async)]
pub async fn import_file(path: String, session: State<'_, ImportedSession>) -> Result<ImportReply, String> {
    let session = session.inner().clone();
    // spawn_blocking moves the CPU-bound work to a thread pool,
    // while the async command itself runs off the main thread
    tauri::async_runtime::spawn_blocking(move || {
        let timer = crate::perf::timer();
        let result = import_from(&path).and_then(|result| stage(result, &session));
        timer.done("import_file", [("ok", result.is_ok().into())]);
        result
    })
    .await
    .map_err(|e| format!("Import task failed: {}", e))?
}

/// Stages an imported document in `session` and replies with what the
/// editor needs of it.
pub(crate) fn stage(result: ImportResult, session: &ImportedSession) -> Result<ImportReply, String> {
    let ImportResult { dsl, metadata, imported_document, original_body_xml, original_preamble, original_postamble, is_imported_mode } =
        result;
    let (imported, words) = match (imported_document, original_body_xml) {
        (Some(document), Some(body)) if is_imported_mode => {
            let words = imported_words(&document.segments, Originals::default());
            let source = ImportedSource::new(
                document,
                body,
                original_preamble.unwrap_or_default(),
                original_postamble.unwrap_or_default(),
            );
            (Some(session.stage(source)?), Some(words))
        }
        _ => (None, None),
    };
    Ok(ImportReply { dsl, metadata, is_imported_mode: imported.is_some(), imported, words })
}

fn import_from(path: &str) -> Result<ImportResult, String> {
    let path_obj = Path::new(path);
    let extension = path_obj
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;

    match extension.as_str() {
        "xml" | "tei" => tei::parse(&content),
        // Plain text files - return as DSL with no metadata or segments
        _ => Ok(ImportResult {
            dsl: content,
            metadata: None,
            imported_document: None,
            original_body_xml: None,
            original_preamble: None,
            original_postamble: None,
            is_imported_mode: false,
        }),
    }
}
