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
///
/// `line_edition` chooses the lineation `//n` shows in a file with several
/// (`""` for all of them); without it, one is inferred.
#[tauri::command(async, rename_all = "camelCase")]
pub async fn import_file(path: String, line_edition: Option<String>, session: State<'_, ImportedSession>) -> Result<ImportReply, String> {
    let session = session.inner().clone();
    // spawn_blocking moves the CPU-bound work to a thread pool,
    // while the async command itself runs off the main thread
    tauri::async_runtime::spawn_blocking(move || {
        let timer = crate::perf::timer();
        let line_edition = line_edition.map(|ed| (!ed.is_empty()).then_some(ed));
        let result = import_from_path(&path, line_edition, &session);
        timer.done("import_file", [("ok", result.is_ok().into())]);
        result
    })
    .await
    .map_err(|e| format!("Import task failed: {}", e))?
}

/// Imports the file at `path` and stages it in `session`.
fn import_from_path(path: &str, line_edition: Option<Option<String>>, session: &ImportedSession) -> Result<ImportReply, String> {
    let (content, extension) = read_import(path)?;
    let mut reply = stage(import_content(&content, &extension, line_edition)?, session)?;
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

/// Makes a new document's file: `xml` (compiled once from the template and
/// the shorthand so far) is staged like an imported file, and the document
/// is edited as its tree from then on, as every document is. Its profile is
/// the file's if it has words, otherwise the template's (`multi_level`: the
/// three MENOTA levels in a `<choice>`; else words without levels; words
/// are `<w>` if `word_wrap`). `dsl`
/// is kept where it reads as the file.
///
/// # Errors
///
/// If `xml` doesn't read as a TEI document with a body.
#[tauri::command(async, rename_all = "camelCase")]
pub async fn create_document(
    xml: String,
    dsl: String,
    multi_level: bool,
    word_wrap: bool,
    auto_line_numbers: bool,
    wrap_pages: bool,
    session: State<'_, ImportedSession>,
) -> Result<ImportReply, String> {
    let session = session.inner().clone();
    let options = TemplateOptions { multi_level, word_wrap, auto_line_numbers, wrap_pages };
    tauri::async_runtime::spawn_blocking(move || created(&xml, &dsl, options, &session))
        .await
        .map_err(|e| format!("Creating the document failed: {e}"))?
}

/// What a new document's template says about how it is encoded.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct TemplateOptions {
    pub multi_level: bool,
    pub word_wrap: bool,
    pub auto_line_numbers: bool,
    pub wrap_pages: bool,
}

pub(crate) fn created(xml: &str, dsl: &str, options: TemplateOptions, session: &ImportedSession) -> Result<ImportReply, String> {
    use crate::document::profile::Profile;
    let TemplateOptions { multi_level, word_wrap, .. } = options;
    let mut import = tei::parse(xml)?;
    let document = import.imported_document.as_mut().ok_or("The new document has no body")?;
    let has_words = document.profile.as_ref().is_some_and(|p| p.counts.tokens > 0);
    if !has_words {
        let counts = document.profile.take().map(|p| p.counts).unwrap_or_default();
        let template = if multi_level { Profile::menota() } else { Profile::plain() };
        document.profile = Some(Profile { counts, ..template });
        // Words are `<w>` (the template's choice), with none in it yet
        document.is_menota = word_wrap;
    }
    if let Some(profile) = document.profile.as_mut() {
        profile.auto_line_numbers = options.auto_line_numbers;
        profile.pages_as_paragraphs = options.wrap_pages;
    }
    import.dsl = crate::importer::tei::patching::keep_as_written(dsl, &import.dsl).0;
    import.is_imported_mode = true;
    stage(import, session)
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

fn import_content(content: &str, extension: &str, line_edition: Option<Option<String>>) -> Result<ImportResult, String> {
    if is_xml(extension) {
        return tei::parse_with(content, line_edition);
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

        let reply = import_from_path(path.to_str().unwrap(), None, &session).unwrap();
        assert!(reply.dsl.contains("ok"));
        assert!(reply.not_well_formed.is_some());

        fs::write(&path, "<TEI><text><body><p><w>ok</w></p></body></text></TEI>").unwrap();
        let reply = import_from_path(path.to_str().unwrap(), None, &session).unwrap();
        assert_eq!(reply.not_well_formed, None);
        fs::remove_dir_all(&dir).ok();
    }
}

#[cfg(test)]
mod created_tests {
    use super::*;
    use crate::parser::{Compiler, CompilerConfig};

    /// A new document as the app makes it: compiled once from the template,
    /// then a tree edited like an imported file, words added with the
    /// template's levels.
    #[test]
    fn a_new_document_is_a_tree_from_the_template() {
        let header = Metadata::default().to_tei_header(true);
        let footer = Metadata::tei_footer();
        let config = CompilerConfig { word_wrap: true, auto_line_numbers: false, multi_level: true, wrap_pages: false, levels: None };
        let body = Compiler::new().with_config(config).compile("").unwrap();
        let xml = format!("{header}{body}{footer}");
        let session = ImportedSession::default();
        let options = TemplateOptions { multi_level: true, word_wrap: true, auto_line_numbers: true, wrap_pages: true };
        let reply = created(&xml, "", options, &session).unwrap();
        assert!(reply.is_imported_mode);
        session.load(reply.imported.map(|s| s.handle)).unwrap();
        let out = session.with_current_mut(|s| s.compile_plainly(".p ok //2 maþr")).unwrap().unwrap().unwrap();
        assert!(out.xml.contains("<me:facs>ok</me:facs>") && out.xml.contains("<me:norm>maþr</me:norm>"), "{}", out.xml);
        assert!(out.xml.contains("<lb n=\"2\"/>") && out.xml.contains("<teiHeader>"), "{}", out.xml);
        // And edited again: the words stay, one more goes in
        let out = session.with_current_mut(|s| s.compile_plainly(".p ok //2 maþr ok")).unwrap().unwrap().unwrap();
        assert_eq!(out.xml.matches("<w>").count(), 3, "{}", out.xml);
        // The template's options as edits: a typed `//` gets the next number,
        // a page break starts the page's paragraph
        let out = session.with_current_mut(|s| s.compile_plainly(".p ok //2 maþr // ok ///2r þa")).unwrap().unwrap().unwrap();
        assert!(out.xml.contains(r#"<lb n="3"/>"#), "{}", out.xml);
        let squashed: String = out.xml.split_whitespace().collect();
        assert!(squashed.contains(r#"</p><pbn="2r"/><p>"#), "{}", out.xml);
    }
}
