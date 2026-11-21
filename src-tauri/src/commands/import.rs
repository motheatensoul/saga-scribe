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
    /// Why the file isn't well-formed XML, if it isn't: libxml2 imported it
    /// as it repaired it, which the editor tells the user
    #[serde(skip_serializing_if = "Option::is_none")]
    pub not_well_formed: Option<String>,
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
        let result = import_from_path(&path, &session);
        timer.done("import_file", [("ok", result.is_ok().into())]);
        result
    })
    .await
    .map_err(|e| format!("Import task failed: {}", e))?
}

/// Imports the file at `path` and stages it in `session`.
fn import_from_path(path: &str, session: &ImportedSession) -> Result<ImportReply, String> {
    let (content, extension) = read_import(path)?;
    let mut reply = stage(import_content(&content, &extension)?, session)?;
    if is_xml(&extension) {
        reply.not_well_formed = crate::xml::check_well_formed(&content).err();
    }
    Ok(reply)
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
    Ok(ImportReply { dsl, metadata, is_imported_mode: imported.is_some(), imported, words, not_well_formed: None })
}

/// The file's text and its extension, lowercased.
fn read_import(path: &str) -> Result<(String, String), String> {
    let extension = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    Ok((content, extension))
}

/// Whether a file with this extension is imported as TEI-XML.
fn is_xml(extension: &str) -> bool {
    matches!(extension, "xml" | "tei")
}

fn import_content(content: &str, extension: &str) -> Result<ImportResult, String> {
    if is_xml(extension) {
        return tei::parse(content);
    }
    // Plain text files - return as DSL with no metadata or segments
    Ok(ImportResult {
        dsl: content.to_string(),
        metadata: None,
        imported_document: None,
        original_body_xml: None,
        original_preamble: None,
        original_postamble: None,
        is_imported_mode: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_that_is_not_well_formed_is_imported_and_said_to_be_so() {
        let dir = std::env::temp_dir().join(format!("saga-scribe-import-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("broken.xml");
        fs::write(&path, "<TEI><text><body><p><w>ok</w> <w>broken</p></body></text></TEI>").unwrap();
        let session = ImportedSession::default();

        let reply = import_from_path(path.to_str().unwrap(), &session).unwrap();
        assert!(reply.dsl.contains("ok"));
        assert!(reply.not_well_formed.is_some());

        fs::write(&path, "<TEI><text><body><p><w>ok</w></p></body></text></TEI>").unwrap();
        let reply = import_from_path(path.to_str().unwrap(), &session).unwrap();
        assert_eq!(reply.not_well_formed, None);
        fs::remove_dir_all(&dir).ok();
    }
}
