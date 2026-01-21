use crate::annotations::AnnotationSet;
use crate::importer::tei::segments::ImportedDocument;
use crate::metadata::Metadata;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

#[derive(Debug, Serialize, Deserialize)]
pub struct FileContent {
    pub path: String,
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
    fs::write(&path, &content).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn export_tei(path: String, tei_content: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    fs::write(&path, &tei_content).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn export_html(path: String, html_content: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    fs::write(&path, &html_content).map_err(|e| e.to_string())
}

// Project archive format (.teis)
// A ZIP file containing:
// - source.dsl: The DSL source text
// - output.xml: The compiled TEI-XML
// - confirmations.json: Word index -> lemma mapping
// - manifest.json: Project metadata

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectManifest {
    pub version: String,
    pub template_id: String,
    pub created: String,
    pub modified: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LemmaConfirmation {
    pub lemma: String,
    pub msa: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectData {
    pub source: String,
    pub output: String,
    /// Legacy lemma confirmations (for backward compat, derived from annotations)
    pub confirmations: HashMap<u32, LemmaConfirmation>,
    pub manifest: ProjectManifest,
    /// Optional manuscript metadata (new in v1.1)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
    /// Full annotation set (new in v1.2)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<AnnotationSet>,
    /// Imported document manifest for round-trip fidelity (new in v1.3)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imported_document: Option<ImportedDocument>,
    /// Original body XML for imported files (new in v1.3)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_body_xml: Option<String>,
    /// Original XML preamble (everything before <body>, new in v1.4)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_preamble: Option<String>,
    /// Original XML postamble (everything after </body>, new in v1.4)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_postamble: Option<String>,
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn save_project(
    path: String,
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
) -> Result<(), String> {
    let path = PathBuf::from(&path);
    let file = File::create(&path).map_err(|e| format!("Failed to create file: {}", e))?;
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
        version: "1.4".to_string(),
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

#[tauri::command]
pub fn open_project(path: String) -> Result<ProjectData, String> {
    let path = PathBuf::from(&path);
    let file = File::open(&path).map_err(|e| format!("Failed to open file: {}", e))?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| format!("Failed to read archive: {}", e))?;

    // Read source.dsl
    let source = read_zip_file(&mut archive, "source.dsl")?;

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

    // Read imported document data (optional, new in v1.3)
    let imported_document: Option<ImportedDocument> = read_zip_file(&mut archive, "segments.json")
        .ok()
        .and_then(|seg_str| serde_json::from_str(&seg_str).ok());

    let original_body_xml = read_zip_file(&mut archive, "original_body.xml").ok();
    let original_preamble = read_zip_file(&mut archive, "original_preamble.xml").ok();
    let original_postamble = read_zip_file(&mut archive, "original_postamble.xml").ok();

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
