//! # File Operations Commands
//!
//! This module provides file I/O commands for project management and export.
//!
//! ## Project Archive Format (.teis)
//!
//! Saga-Scribe projects are stored as ZIP archives with the following structure:
//!
//! ```text
//! project.teis (ZIP)
//! ├── source.dsl           # DSL source text
//! ├── output.xml           # Compiled TEI-XML
//! ├── confirmations.json   # Legacy lemma mappings (v1.0 compat)
//! ├── annotations.json     # Full annotation set (v1.2+)
//! ├── metadata.json        # Manuscript metadata (v1.1+)
//! ├── segments.json        # Import segment manifest (v1.3+)
//! ├── original_body.xml    # Original body for round-trip (v1.3+)
//! ├── original_preamble.xml # XML before <body> (v1.4+)
//! ├── original_postamble.xml # XML after </body> (v1.4+)
//! └── manifest.json        # Project metadata
//! ```
//!
//! ## Version History
//!
//! | Version | Changes |
//! |---------|---------|
//! | 1.0 | Initial format with source, output, confirmations |
//! | 1.1 | Added metadata.json |
//! | 1.2 | Added annotations.json (full annotation set) |
//! | 1.3 | Added segments.json and original_body.xml for round-trip |
//! | 1.4 | Added original_preamble.xml and original_postamble.xml |
//! | 1.5 | segments.json is always the full manifest (incl. `is_menota`, source spans) |
//! | 1.6 | Paragraph start tags are `paragraph` segments (`.p` in the DSL); spans cover all tags |
//! | 1.7 | `\` escapes a character and `#…#` is a numeral; older shorthand is escaped when opened |
//! | 1.8 | segments.json holds the answers for replaced and deleted words (`decisions`) |
//!
//! All writes are atomic: data is written to a temporary file next to the target
//! and renamed over it, so a crash mid-save never leaves a truncated file.

use crate::annotations::AnnotationSet;
use crate::commands::parse::{ImportedSession, ImportedSource, StagedDocument};
use crate::importer::tei::recompute_spans;
use crate::importer::tei::segments::{Decision, ImportedDocument, Segment};
use crate::metadata::Metadata;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tauri::State;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

/// Result of opening a text file.
#[derive(Debug, Serialize, Deserialize)]
pub struct FileContent {
    /// The path that was opened
    pub path: String,
    /// The file's text content
    pub content: String,
}

#[tauri::command]
pub fn open_file(path: String) -> Result<FileContent, String> {
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    Ok(FileContent { path, content })
}

#[tauri::command]
pub fn load_text_file(path: String) -> Result<String, String> {
    fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_file(path: String, content: String) -> Result<(), String> {
    write_file_atomic(Path::new(&path), content.as_bytes())
}

/// Writes the TEI-XML export, after libxml2 has read it: XML that isn't
/// well-formed is refused, not written.
#[tauri::command(async)]
pub async fn export_tei(path: String, tei_content: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        check_well_formed(&tei_content)?;
        write_file_atomic(Path::new(&path), tei_content.as_bytes())
    })
    .await
    .map_err(|e| format!("Export failed: {e}"))?
}

/// An error unless libxml2 reads `xml` as a well-formed document.
fn check_well_formed(xml: &str) -> Result<(), String> {
    let timer = crate::perf::timer();
    let result = crate::xml::check_well_formed(xml);
    timer.done("export.check", [("ok", result.is_ok().into())]);
    result.map_err(|e| format!("Not exported: the XML is not well-formed ({e}). Please report this as a bug."))
}

#[tauri::command]
pub fn export_html(path: String, html_content: String) -> Result<(), String> {
    write_file_atomic(Path::new(&path), html_content.as_bytes())
}

/// Writes `contents` to `path` without ever leaving a partially written file.
pub fn write_file_atomic(path: &Path, contents: &[u8]) -> Result<(), String> {
    write_atomic_with(path, |file| {
        file.write_all(contents)
            .map_err(|e| format!("Failed to write file: {}", e))
    })
}

/// Creates a temporary file beside `path`, lets `write` fill it, syncs it to disk
/// and renames it over `path`. On any failure the original file is untouched.
fn write_atomic_with(
    path: &Path,
    write: impl FnOnce(&mut File) -> Result<(), String>,
) -> Result<(), String> {
    let file_name = path
        .file_name()
        .ok_or_else(|| format!("Invalid file path: {}", path.display()))?;
    let tmp_path = path.with_file_name(format!(".{}.tmp", file_name.to_string_lossy()));

    let result = File::create(&tmp_path)
        .map_err(|e| format!("Failed to create file: {}", e))
        .and_then(|mut file| {
            write(&mut file)?;
            file.sync_all()
                .map_err(|e| format!("Failed to flush file: {}", e))
        })
        .and_then(|_| {
            fs::rename(&tmp_path, path).map_err(|e| format!("Failed to replace file: {}", e))
        });

    if result.is_err() {
        let _ = fs::remove_file(&tmp_path);
    }
    result
}

// Project archive format (.teis)
// A ZIP file containing:
// - source.dsl: The DSL source text
// - output.xml: The compiled TEI-XML
// - confirmations.json: Word index -> lemma mapping
// - manifest.json: Project metadata

/// Project metadata stored in manifest.json within the archive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectManifest {
    /// Archive format version (e.g., "1.4")
    pub version: String,
    /// ID of the TEI template used
    pub template_id: String,
    /// ISO 8601 timestamp of creation
    pub created: String,
    /// ISO 8601 timestamp of last modification
    pub modified: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LemmaConfirmation {
    pub lemma: String,
    pub msa: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized: Option<String>,
}

/// Complete project data loaded from a .teis archive.
///
/// This struct is returned by [`open_project`] and contains all data
/// needed to restore the editor state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectData {
    /// DSL source text from source.dsl
    pub source: String,
    /// Compiled TEI-XML from output.xml
    pub output: String,
    /// Legacy lemma confirmations (v1.0 backward compatibility)
    pub confirmations: HashMap<u32, LemmaConfirmation>,
    /// Project metadata from manifest.json
    pub manifest: ProjectManifest,
    /// Manuscript metadata (v1.1+)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
    /// Full annotation set including lemmas, semantics, paleography (v1.2+)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<AnnotationSet>,
    /// Segment manifest for imported documents (v1.3+)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imported_document: Option<ImportedDocument>,
    /// Original body XML for round-trip fidelity (v1.3+)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_body_xml: Option<String>,
    /// XML content before `<body>` element (v1.4+)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_preamble: Option<String>,
    /// XML content after `</body>` element (v1.4+)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_postamble: Option<String>,
}

/// Saves a project to a .teis archive file.
///
/// Creates a ZIP archive containing all project data. The archive format
/// is versioned to support backward compatibility with older app versions.
///
/// With `decisions_json` (the answers for replaced words), the project is an
/// imported document: its manifest and original file come from the
/// document the backend holds ([`ImportedSession`]).
///
/// Replies with why the compiled TEI-XML (`output.xml`) isn't well-formed,
/// if it isn't: the project is saved all the same, as its shorthand is what
/// it is made from, but the editor warns about it.
#[tauri::command(async)]
#[allow(clippy::too_many_arguments)]
pub async fn save_project(
    session: State<'_, ImportedSession>,
    path: String,
    source: String,
    output: String,
    confirmations_json: String,
    template_id: String,
    metadata_json: Option<String>,
    annotations_json: Option<String>,
    decisions_json: Option<String>,
) -> Result<Option<String>, String> {
    let session = session.inner().clone();
    let fields = ProjectFields { source, output, confirmations_json, template_id, metadata_json, annotations_json };
    tauri::async_runtime::spawn_blocking(move || {
        let timer = crate::perf::timer();
        let not_well_formed = crate::xml::check_well_formed(&fields.output).err();
        let result = save_project_at(&session, Path::new(&path), fields, decisions_json);
        timer.done("save_project", [("ok", result.is_ok().into()), ("well_formed", not_well_formed.is_none().into())]);
        result.map(|()| not_well_formed)
    })
    .await
    .map_err(|e| format!("Saving failed: {e}"))?
}

/// What the editor sends to save a project, besides an imported document's
/// answers for replaced words.
pub(crate) struct ProjectFields {
    pub source: String,
    pub output: String,
    pub confirmations_json: String,
    pub template_id: String,
    pub metadata_json: Option<String>,
    pub annotations_json: Option<String>,
}

/// Saves a project at `path` (see [`save_project`]).
pub(crate) fn save_project_at(
    session: &ImportedSession,
    path: &Path,
    fields: ProjectFields,
    decisions_json: Option<String>,
) -> Result<(), String> {
    let ProjectFields { source, output, confirmations_json, template_id, metadata_json, annotations_json } = fields;
    let imported = match decisions_json {
        Some(json) => {
            let decisions: HashMap<usize, Decision> =
                serde_json::from_str(&json).map_err(|e| format!("Failed to read the answers for replaced words: {e}"))?;
            let parts = session.with_current(|document| {
                let (body, preamble, postamble) = document.original_parts();
                document
                    .manifest_json(decisions)
                    .map(|manifest| (manifest, body.to_string(), preamble.to_string(), postamble.to_string()))
            })?;
            Some(parts.ok_or("No imported document is loaded")??)
        }
        None => None,
    };
    let (segments_json, original_body_xml, original_preamble, original_postamble) = match imported {
        Some((manifest, body, preamble, postamble)) => (Some(manifest), Some(body), Some(preamble), Some(postamble)),
        None => (None, None, None, None),
    };
    write_atomic_with(path, |file| {
        write_project_archive(
            file,
            ProjectContents {
                source,
                output,
                confirmations_json,
                template_id,
                metadata_json,
                annotations_json,
                segments_json,
                original_body_xml,
                original_preamble,
                original_postamble,
            },
        )
    })
}

/// Everything written into a project archive.
struct ProjectContents {
    source: String,
    output: String,
    confirmations_json: String,
    template_id: String,
    metadata_json: Option<String>,
    annotations_json: Option<String>,
    segments_json: Option<String>,
    original_body_xml: Option<String>,
    original_preamble: Option<String>,
    original_postamble: Option<String>,
}

fn write_project_archive(file: &mut File, contents: ProjectContents) -> Result<(), String> {
    let ProjectContents {
        source,
        output,
        confirmations_json,
        template_id,
        metadata_json,
        annotations_json,
        segments_json,
        original_body_xml,
        original_preamble,
        original_postamble,
    } = contents;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    // Write source.dsl
    zip.start_file("source.dsl", options)
        .map_err(|e| format!("Failed to start source.dsl: {}", e))?;
    zip.write_all(source.as_bytes())
        .map_err(|e| format!("Failed to write source.dsl: {}", e))?;

    // Write output.xml
    zip.start_file("output.xml", options)
        .map_err(|e| format!("Failed to start output.xml: {}", e))?;
    zip.write_all(output.as_bytes())
        .map_err(|e| format!("Failed to write output.xml: {}", e))?;

    // Write confirmations.json (backward compat for older app versions)
    zip.start_file("confirmations.json", options)
        .map_err(|e| format!("Failed to start confirmations.json: {}", e))?;
    zip.write_all(confirmations_json.as_bytes())
        .map_err(|e| format!("Failed to write confirmations.json: {}", e))?;

    // Write annotations.json (new in v1.2, full annotation set)
    if let Some(ref ann_json) = annotations_json {
        zip.start_file("annotations.json", options)
            .map_err(|e| format!("Failed to start annotations.json: {}", e))?;
        zip.write_all(ann_json.as_bytes())
            .map_err(|e| format!("Failed to write annotations.json: {}", e))?;
    }

    // Write metadata.json if provided
    if let Some(ref meta_json) = metadata_json {
        zip.start_file("metadata.json", options)
            .map_err(|e| format!("Failed to start metadata.json: {}", e))?;
        zip.write_all(meta_json.as_bytes())
            .map_err(|e| format!("Failed to write metadata.json: {}", e))?;
    }

    // Write segments.json if provided (new in v1.3, imported document manifest)
    if let Some(ref seg_json) = segments_json {
        zip.start_file("segments.json", options)
            .map_err(|e| format!("Failed to start segments.json: {}", e))?;
        zip.write_all(seg_json.as_bytes())
            .map_err(|e| format!("Failed to write segments.json: {}", e))?;
    }

    // Write original_body.xml if provided (new in v1.3, original body for round-trip)
    if let Some(ref body_xml) = original_body_xml {
        zip.start_file("original_body.xml", options)
            .map_err(|e| format!("Failed to start original_body.xml: {}", e))?;
        zip.write_all(body_xml.as_bytes())
            .map_err(|e| format!("Failed to write original_body.xml: {}", e))?;
    }

    // Write original preamble/postamble if provided (new in v1.4)
    if let Some(ref preamble_xml) = original_preamble {
        zip.start_file("original_preamble.xml", options)
            .map_err(|e| format!("Failed to start original_preamble.xml: {}", e))?;
        zip.write_all(preamble_xml.as_bytes())
            .map_err(|e| format!("Failed to write original_preamble.xml: {}", e))?;
    }

    if let Some(ref postamble_xml) = original_postamble {
        zip.start_file("original_postamble.xml", options)
            .map_err(|e| format!("Failed to start original_postamble.xml: {}", e))?;
        zip.write_all(postamble_xml.as_bytes())
            .map_err(|e| format!("Failed to write original_postamble.xml: {}", e))?;
    }

    // Create and write manifest.json
    let now = chrono_lite_now();
    let manifest = ProjectManifest {
        version: "1.8".to_string(),
        template_id,
        created: now.clone(),
        modified: now,
    };
    let manifest_json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("Failed to serialize manifest: {}", e))?;
    zip.start_file("manifest.json", options)
        .map_err(|e| format!("Failed to start manifest.json: {}", e))?;
    zip.write_all(manifest_json.as_bytes())
        .map_err(|e| format!("Failed to write manifest.json: {}", e))?;

    zip.finish()
        .map_err(|e| format!("Failed to finalize archive: {}", e))?;

    Ok(())
}

/// A project as the editor gets it: an imported document stays in the
/// backend, staged ([`ImportedSession::stage`]), instead of its manifest and
/// original file.
#[derive(Debug, Serialize)]
pub struct OpenedProject {
    #[serde(flatten)]
    pub project: ProjectData,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imported: Option<StagedDocument>,
    /// How many places of the shorthand were rewritten to read as the file
    /// is projected now (a project saved with older import rules)
    #[serde(skip_serializing_if = "is_zero")]
    pub rewritten: usize,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// Opens a .teis project archive and loads all contained data; an imported
/// document is read into its tree and staged.
#[tauri::command(async)]
pub async fn open_project(path: String, session: State<'_, ImportedSession>) -> Result<OpenedProject, String> {
    let session = session.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let timer = crate::perf::timer();
        let mut project = read_project(&path)?;
        let (imported, rewritten) = stage_project(&mut project, &session)?;
        timer.done("open_project", [("imported", imported.is_some().into()), ("rewritten", rewritten.into())]);
        Ok(OpenedProject { project, imported, rewritten })
    })
    .await
    .map_err(|e| format!("Opening failed: {e}"))?
}

/// Stages the imported document of `project`, if it has one, on its file's
/// tree as projected now: its shorthand is rewritten where unedited words
/// read differently with the current import rules (see
/// [`onto_tree`](crate::importer::tei::onto_tree)). Returns the staged
/// document and how many places were rewritten.
pub(crate) fn stage_project(project: &mut ProjectData, session: &ImportedSession) -> Result<(Option<StagedDocument>, usize), String> {
    let preamble = project.original_preamble.take().unwrap_or_default();
    let postamble = project.original_postamble.take().unwrap_or_default();
    let (Some(stored), Some(body)) = (project.imported_document.take(), project.original_body_xml.take()) else {
        return Ok((None, 0));
    };
    let (document, tree, rewritten) = match crate::importer::tei::onto_tree(&stored, &project.source, &preamble, &body, &postamble) {
        Some(on_tree) => {
            project.source = on_tree.dsl;
            (on_tree.document, Some(on_tree.tree), on_tree.rewritten)
        }
        None => {
            log::warn!("The project's file couldn't be projected afresh; its stored segments are used");
            (stored, None, 0)
        }
    };
    let source = ImportedSource::with_tree(document, tree, body, preamble, postamble);
    Ok((Some(session.stage(source)?), rewritten))
}

/// Reads a .teis project archive.
///
/// Handles backward compatibility with older archive versions by
/// gracefully handling missing optional files.
pub fn read_project(path: &str) -> Result<ProjectData, String> {
    let path = PathBuf::from(path);
    let file = File::open(&path).map_err(|e| format!("Failed to open file: {}", e))?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| format!("Failed to read archive: {}", e))?;

    // Read source.dsl
    let mut source = read_zip_file(&mut archive, "source.dsl")?;

    // Read output.xml
    let output = read_zip_file(&mut archive, "output.xml")?;

    // Read confirmations.json (always present for backward compat)
    let confirmations_str = read_zip_file(&mut archive, "confirmations.json")?;
    let confirmations: HashMap<u32, LemmaConfirmation> =
        serde_json::from_str(&confirmations_str)
            .map_err(|e| format!("Failed to parse confirmations.json: {}", e))?;

    // Read manifest.json
    let manifest_str = read_zip_file(&mut archive, "manifest.json")?;
    let manifest: ProjectManifest = serde_json::from_str(&manifest_str)
        .map_err(|e| format!("Failed to parse manifest.json: {}", e))?;

    // Read metadata.json (optional, new in v1.1)
    let metadata: Option<Metadata> = read_zip_file(&mut archive, "metadata.json")
        .ok()
        .and_then(|meta_str| serde_json::from_str(&meta_str).ok());

    // Read annotations.json (optional, new in v1.2)
    let annotations: Option<AnnotationSet> = read_zip_file(&mut archive, "annotations.json")
        .ok()
        .and_then(|ann_str| serde_json::from_str(&ann_str).ok());

    let original_body_xml = read_zip_file(&mut archive, "original_body.xml").ok();
    let original_preamble = read_zip_file(&mut archive, "original_preamble.xml").ok();
    let original_postamble = read_zip_file(&mut archive, "original_postamble.xml").ok();

    // Read imported document data (optional, new in v1.3)
    let mut imported_document = match read_zip_file(&mut archive, "segments.json") {
        Ok(seg_str) => Some(ImportedDocument::from_json(&seg_str)?),
        Err(_) => None,
    };

    // Before v1.7, `\` and `#` were text; since, `\` escapes a character
    // and `#…#` is a numeral. The shorthand and the segments it was made
    // from are escaped alike, so they still match.
    if version_before(&manifest.version, 1, 7) {
        source = escape_pre_1_7(&source);
        if let Some(doc) = imported_document.as_mut() {
            for segment in &mut doc.segments {
                if let Segment::Word { dsl_content, .. } | Segment::Punctuation { dsl_content, .. } = segment {
                    *dsl_content = escape_pre_1_7(dsl_content);
                }
            }
        }
    }

    // Projects saved before v1.5 have no source spans, and before v1.6 none
    // for element tags (needed to split paragraphs); derive them from the
    // stored original XML so edits can be spliced in byte-for-byte.
    if let (Some(doc), Some(body)) = (imported_document.as_mut(), original_body_xml.as_deref()) {
        if !has_tag_spans(doc) {
            doc.spans = recompute_spans(
                original_preamble.as_deref().unwrap_or_default(),
                body,
                original_postamble.as_deref().unwrap_or_default(),
                &doc.segments,
            );
        }
    }

    Ok(ProjectData {
        source,
        output,
        confirmations,
        manifest,
        metadata,
        annotations,
        imported_document,
        original_body_xml,
        original_preamble,
        original_postamble,
    })
}

/// Whether an archive format version (`"1.6"`) is older than `major.minor`.
fn version_before(version: &str, major: u32, minor: u32) -> bool {
    let mut parts = version.split('.').map(|part| part.parse::<u32>().unwrap_or(0));
    (parts.next().unwrap_or(0), parts.next().unwrap_or(0)) < (major, minor)
}

/// Shorthand from before v1.7, where `\` and `#` were text, as text in the
/// shorthand since.
fn escape_pre_1_7(dsl: &str) -> String {
    dsl.replace('\\', "\\\\").replace('#', "\\#")
}

fn read_zip_file<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Result<String, String> {
    let mut file = archive
        .by_name(name)
        .map_err(|e| format!("Failed to find {} in archive: {}", name, e))?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|e| format!("Failed to read {}: {}", name, e))?;
    Ok(contents)
}

// Simple ISO 8601 timestamp without external crate
fn chrono_lite_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    // Convert to rough ISO format (not perfect but good enough)
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Approximate year/month/day calculation
    let mut year = 1970;
    let mut remaining_days = days_since_epoch as i64;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    let days_in_months: [i64; 12] = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1;
    for days in days_in_months.iter() {
        if remaining_days < *days {
            break;
        }
        remaining_days -= days;
        month += 1;
    }
    let day = remaining_days + 1;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hours, minutes, seconds
    )
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Whether the manifest has source spans for every element tag (comments
/// never have one).
fn has_tag_spans(doc: &ImportedDocument) -> bool {
    let Some(spans) = &doc.spans else { return false };
    doc.segments.iter().all(|segment| match segment {
        Segment::Structural { xml, .. } if !xml.starts_with("<!--") => {
            spans.get(segment.id()).copied().flatten().is_some()
        }
        Segment::Paragraph { .. } => spans.get(segment.id()).copied().flatten().is_some(),
        _ => true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_projects_without_tag_spans_get_them_recomputed() {
        let xml = "<TEI><text><body><p><s>a b</s> <s>c</s></p></body></text></TEI>";
        let import = crate::importer::tei::parse(xml).unwrap();
        let mut doc = import.imported_document.unwrap();
        assert!(has_tag_spans(&doc));

        // As saved by 0.6.0 pre-releases (format 1.5): spans for words only
        let tags: Vec<usize> = doc
            .segments
            .iter()
            .filter(|s| matches!(s, Segment::Structural { .. } | Segment::Paragraph { .. }))
            .map(Segment::id)
            .collect();
        for id in tags {
            doc.spans.as_mut().unwrap()[id] = None;
        }
        assert!(!has_tag_spans(&doc));

        let body = import.original_body_xml.unwrap();
        let (pre, post) = xml.split_once(&body).unwrap();
        doc.spans = recompute_spans(pre, &body, post, &doc.segments);
        assert!(has_tag_spans(&doc));
    }

    #[test]
    fn test_parse_imported_document_accepts_full_manifest() {
        let json = r#"{"segments":[{"type":"whitespace","id":0,"content":" "}],"is_menota":true,"spans":[null]}"#;
        let doc = ImportedDocument::from_json(json).unwrap();
        assert!(doc.is_menota);
        assert_eq!(doc.spans, Some(vec![None]));
    }

    #[test]
    fn test_parse_imported_document_accepts_legacy_bare_array() {
        // Projects up to v1.4 saved a bare segment array.
        let json = r#"[{"type":"word","id":0,"original_xml":"<w><choice><me:facs>a</me:facs></choice></w>","attributes":{},"dsl_content":"a","has_inline_lb":false}]"#;
        let doc = ImportedDocument::from_json(json).unwrap();
        assert_eq!(doc.segments.len(), 1);
        assert!(doc.is_menota, "MENOTA structure should be re-detected");
        assert!(doc.spans.is_none());
    }

    #[test]
    fn test_shorthand_before_1_7_is_escaped() {
        assert!(version_before("1.6", 1, 7));
        assert!(version_before("1.0", 1, 7));
        assert!(!version_before("1.7", 1, 7));
        assert!(!version_before("2.0", 1, 7));
        assert_eq!(escape_pre_1_7(r"a#b\c ^{#1}"), r"a\#b\\c ^{\#1}");
    }

    #[test]
    fn test_parse_imported_document_reports_garbage() {
        assert!(ImportedDocument::from_json("{not json").is_err());
    }

    #[test]
    fn test_project_archive_roundtrip() {
        let dir = std::env::temp_dir().join(format!("saga-scribe-project-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("p.teis");
        let manifest_json = r#"{"segments":[],"is_menota":true,"spans":[]}"#;

        write_atomic_with(&path, |file| {
            write_project_archive(
                file,
                ProjectContents {
                    source: "//1 maþr".into(),
                    output: "<TEI/>".into(),
                    confirmations_json: "{}".into(),
                    template_id: "menota".into(),
                    metadata_json: None,
                    annotations_json: None,
                    segments_json: Some(manifest_json.into()),
                    original_body_xml: Some("<body></body>".into()),
                    original_preamble: Some("<TEI><text>".into()),
                    original_postamble: Some("</text></TEI>".into()),
                },
            )
        })
        .unwrap();
        let project = read_project(&path.to_string_lossy()).unwrap();

        assert_eq!(project.source, "//1 maþr");
        assert_eq!(project.output, "<TEI/>");
        assert_eq!(project.manifest.version, "1.8");
        assert_eq!(project.manifest.template_id, "menota");
        assert!(project.imported_document.unwrap().is_menota);
        assert_eq!(project.original_body_xml.as_deref(), Some("<body></body>"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_write_file_atomic_replaces_and_cleans_up() {
        let dir = std::env::temp_dir().join(format!("saga-scribe-atomic-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("out.xml");
        fs::write(&path, "old").unwrap();

        write_file_atomic(&path, b"new").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
        assert!(!dir.join(".out.xml.tmp").exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_export_refuses_xml_that_is_not_well_formed() {
        assert!(check_well_formed("<TEI><text>a &thorn; b</text></TEI>").is_ok());
        let error = check_well_formed("<TEI><text><w>a</text></TEI>").unwrap_err();
        assert!(error.starts_with("Not exported"), "{error}");
    }
}
