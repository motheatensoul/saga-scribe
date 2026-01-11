use crate::validator::actor::{ValidationRequest, ValidationSender};
use crate::validator::{SchemaInfo, ValidationResult};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};
use tokio::sync::oneshot;

/// Get the schemas directory path
fn get_schemas_dir(app: &AppHandle) -> Result<PathBuf, String> {
    // In production, schemas are bundled as resources
    if let Ok(resource_dir) = app.path().resource_dir() {
        let schemas_path = resource_dir.join("schemas");
        if schemas_path.exists() {
            return Ok(schemas_path);
        }
    }

    // In development, use static folder
    let dev_path = PathBuf::from("../static/schemas");
    if dev_path.exists() {
        return Ok(dev_path);
    }

    // Try from current working directory
    let cwd_path = std::env::current_dir()
        .map_err(|e| e.to_string())?
        .join("static/schemas");
    if cwd_path.exists() {
        return Ok(cwd_path);
    }

    Err("Schemas directory not found".to_string())
}

/// List available schemas
#[tauri::command]
pub fn list_schemas(app: AppHandle) -> Result<Vec<SchemaInfo>, String> {
    let schemas_dir = get_schemas_dir(&app)?;

    let mut schemas = Vec::new();

    // Check for TEI P5 schema
    if schemas_dir.join("tei_all.xsd").exists() {
        schemas.push(SchemaInfo {
            id: "tei-p5".to_string(),
            name: "TEI P5".to_string(),
            description: "Text Encoding Initiative P5 Guidelines".to_string(),
            file_name: "tei_all.xsd".to_string(),
        });
    }

    // Check for MENOTA schema
    if schemas_dir.join("menota.xsd").exists() {
        schemas.push(SchemaInfo {
            id: "menota".to_string(),
            name: "MENOTA (XSD)".to_string(),
            description: "Medieval Nordic Text Archive schema".to_string(),
            file_name: "menota.xsd".to_string(),
        });
    }
    
    // Check for MENOTA P5 RNG schema
    if schemas_dir.join("menotaP5.rng").exists() {
        schemas.push(SchemaInfo {
            id: "menota-p5".to_string(),
            name: "MENOTA P5 (RelaxNG)".to_string(),
            description: "Medieval Nordic Text Archive P5 RelaxNG schema".to_string(),
            file_name: "menotaP5.rng".to_string(),
        });
    }

    // Scan for any custom schemas (*.xsd, *.rng files)
    // Exclude known dependency files and built-in schemas
    const EXCLUDED_FILES: &[&str] = &[
        "tei_all.xsd",
        "tei_all_teix.xsd", // TEI examples namespace (dependency)
        "tei_all_xml.xsd",  // XML namespace attributes (dependency)
        "menota.xsd",
        "menotaP5.rng",
    ];

    if let Ok(entries) = fs::read_dir(&schemas_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy();
                if ext_str == "xsd" || ext_str == "rng" {
                    let file_name = path.file_name().unwrap().to_string_lossy().to_string();
                    // Skip built-in and dependency schemas
                    if !EXCLUDED_FILES.contains(&file_name.as_str()) {
                        let id = path
                            .file_stem()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default();
                        schemas.push(SchemaInfo {
                            id: id.clone(),
                            name: id.clone(),
                            description: format!("Custom schema: {}", file_name),
                            file_name,
                        });
                    }
                }
            }
        }
    }

    Ok(schemas)
}

/// Validate XML content against a schema
#[tauri::command]
pub async fn validate_xml(
    app: AppHandle,
    sender: State<'_, ValidationSender>,
    xml_content: String,
    schema_id: String,
) -> Result<ValidationResult, String> {
    let schemas_dir = get_schemas_dir(&app)?;

    // Map schema ID to filename
    let schema_file = match schema_id.as_str() {
        "tei-p5" => "tei_all.xsd",
        "menota" => "menota.xsd",
        "menota-p5" => "menotaP5.rng",
        _ => &schema_id, // Assume it's a filename for custom schemas
    };

    let schema_path = schemas_dir.join(schema_file);

    if !schema_path.exists() {
        return Err(format!("Schema not found: {}", schema_id));
    }

    let is_rng = schema_path.extension().and_then(|s| s.to_str()) == Some("rng");

    let (tx, rx) = oneshot::channel();

    if is_rng {
        sender
            .0
            .send(ValidationRequest::RelaxNg {
                xml: xml_content,
                schema_path,
                reply: tx,
            })
            .map_err(|e| format!("Failed to send validation request: {}", e))?;
    } else {
        sender
            .0
            .send(ValidationRequest::Xsd {
                xml: xml_content,
                schema_path,
                reply: tx,
            })
            .map_err(|e| format!("Failed to send validation request: {}", e))?;
    }

    rx.await
        .map_err(|e| format!("Failed to receive validation response: {}", e))?
}

/// Validate XML content against a schema provided as a string
#[tauri::command]
pub async fn validate_xml_with_schema(
    sender: State<'_, ValidationSender>,
    xml_content: String,
    schema_content: String,
    schema_name: String,
) -> Result<ValidationResult, String> {
    let (tx, rx) = oneshot::channel();

    // Heuristic to detect if it's RNG or XSD
    let is_rng = schema_content.trim().contains("http://relaxng.org/ns/structure/1.0");

    if is_rng {
        sender
            .0
            .send(ValidationRequest::RelaxNgString {
                xml: xml_content,
                schema_content,
                schema_name,
                reply: tx,
            })
            .map_err(|e| format!("Failed to send validation request: {}", e))?;
    } else {
        sender
            .0
            .send(ValidationRequest::XsdString {
                xml: xml_content,
                schema_content,
                schema_name,
                reply: tx,
            })
            .map_err(|e| format!("Failed to send validation request: {}", e))?;
    }

    rx.await
        .map_err(|e| format!("Failed to receive validation response: {}", e))?
}
