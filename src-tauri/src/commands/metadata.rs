//! Tauri commands for metadata operations.

use crate::metadata::Metadata;

/// Generate a TEI header from structured metadata
#[tauri::command]
pub fn generate_tei_header(metadata_json: String, include_menota_ns: bool) -> Result<String, String> {
    let metadata: Metadata = serde_json::from_str(&metadata_json)
        .map_err(|e| format!("Failed to parse metadata: {}", e))?;

    Ok(metadata.to_tei_header(include_menota_ns))
}

/// Generate a TEI footer (closing tags)
#[tauri::command]
pub fn generate_tei_footer() -> String {
    Metadata::tei_footer()
}

/// Validate metadata JSON structure
#[tauri::command]
pub fn validate_metadata(metadata_json: String) -> Result<bool, String> {
    let _: Metadata = serde_json::from_str(&metadata_json)
        .map_err(|e| format!("Invalid metadata: {}", e))?;
    Ok(true)
}

/// Create empty metadata with default values
#[tauri::command]
pub fn create_empty_metadata() -> String {
    let metadata = Metadata::new();
    serde_json::to_string(&metadata).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_tei_header() {
        let json = r#"{"titleStmt":{"title":"Test"},"publicationStmt":{},"msIdentifier":{},"msContents":{},"physDesc":{},"history":{},"languages":[]}"#;
        let result = generate_tei_header(json.to_string(), false);
        assert!(result.is_ok());
        let header = result.unwrap();
        assert!(header.contains("<title>Test</title>"));
    }

    #[test]
    fn test_validate_metadata() {
        let valid = r#"{"titleStmt":{},"publicationStmt":{},"msIdentifier":{},"msContents":{},"physDesc":{},"history":{},"languages":[]}"#;
        assert!(validate_metadata(valid.to_string()).is_ok());

        let invalid = r#"{"bad": "json"#;
        assert!(validate_metadata(invalid.to_string()).is_err());
    }

    #[test]
    fn test_create_empty_metadata() {
        let json = create_empty_metadata();
        assert!(json.contains("titleStmt"));
        assert!(validate_metadata(json).is_ok());
    }
}
