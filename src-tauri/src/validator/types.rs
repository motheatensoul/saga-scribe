use serde::{Deserialize, Serialize};

/// Information about an available schema
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub file_name: String,
}

/// A single validation error or warning
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationError {
    pub message: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub is_warning: bool,
}

impl ValidationError {
    pub fn error(message: String, line: Option<u32>, column: Option<u32>) -> Self {
        Self {
            message,
            line,
            column,
            is_warning: false,
        }
    }

    #[allow(dead_code)]
    pub fn warning(message: String, line: Option<u32>, column: Option<u32>) -> Self {
        Self {
            message,
            line,
            column,
            is_warning: true,
        }
    }
}

/// Result of validating an XML document against a schema
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResult {
    pub valid: bool,
    pub schema_name: String,
    pub errors: Vec<ValidationError>,
    pub error_count: u32,
    pub warning_count: u32,
}

impl ValidationResult {
    pub fn success(schema_name: &str) -> Self {
        Self {
            valid: true,
            schema_name: schema_name.to_string(),
            errors: vec![],
            error_count: 0,
            warning_count: 0,
        }
    }

    pub fn with_errors(schema_name: &str, errors: Vec<ValidationError>) -> Self {
        let error_count = errors.iter().filter(|e| !e.is_warning).count() as u32;
        let warning_count = errors.iter().filter(|e| e.is_warning).count() as u32;

        Self {
            valid: error_count == 0,
            schema_name: schema_name.to_string(),
            errors,
            error_count,
            warning_count,
        }
    }
}
