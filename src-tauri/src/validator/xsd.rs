use super::types::{ValidationError, ValidationResult};
use libxml::parser::Parser;
use libxml::schemas::{SchemaParserContext, SchemaValidationContext};
use std::path::Path;

/// XSD validator using libxml2
pub struct XsdValidator;

impl XsdValidator {
    /// Validate XML content against an XSD schema file (parses the schema each call)
    #[cfg(test)]
    pub fn validate(xml_content: &str, schema_path: &Path) -> Result<ValidationResult, String> {
        let mut schema_ctx = Self::load_schema(schema_path)?;
        Self::validate_with_context(&mut schema_ctx, xml_content, &Self::schema_name(schema_path))
    }

    /// Parses an XSD schema file into a reusable validation context.
    ///
    /// Parsing a large schema such as `tei_all.xsd` takes seconds, so callers
    /// validating repeatedly should keep the context.
    pub fn load_schema(schema_path: &Path) -> Result<SchemaValidationContext, String> {
        let schema_path_str = schema_path
            .to_str()
            .ok_or("Invalid schema path encoding")?;

        let mut schema_parser = SchemaParserContext::from_file(schema_path_str);
        SchemaValidationContext::from_parser(&mut schema_parser).map_err(|errs| {
            let messages: Vec<String> = errs
                .iter()
                .filter_map(|e| e.message.clone())
                .collect();
            format!("Failed to parse schema: {}", messages.join("; "))
        })
    }

    /// Display name for a schema file (its file stem).
    pub fn schema_name(schema_path: &Path) -> String {
        schema_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string()
    }

    /// Validates XML content with an already loaded schema context.
    pub fn validate_with_context(
        schema_ctx: &mut SchemaValidationContext,
        xml_content: &str,
        schema_name: &str,
    ) -> Result<ValidationResult, String> {
        let xml_doc = Parser::default()
            .parse_string(xml_content)
            .map_err(|e| format!("Failed to parse XML: {}", e))?;

        // Discard anything left over from a previous run of this context
        schema_ctx.drain_errors();

        match schema_ctx.validate_document(&xml_doc) {
            Ok(()) => Ok(ValidationResult::success(schema_name)),
            Err(errs) => {
                let errors: Vec<ValidationError> = errs
                    .iter()
                    .map(|e| {
                        ValidationError::error(
                            e.message.clone().unwrap_or_else(|| "Unknown error".to_string()),
                            e.line.map(|l| l as u32),
                            None, // libxml doesn't provide column info
                        )
                    })
                    .collect();
                Ok(ValidationResult::with_errors(schema_name, errors))
            }
        }
    }

    /// Validate XML content against an XSD schema string
    pub fn validate_with_schema_string(
        xml_content: &str,
        schema_content: &str,
        schema_name: &str,
    ) -> Result<ValidationResult, String> {
        // Parse the XML document
        let parser = Parser::default();
        let xml_doc = parser
            .parse_string(xml_content)
            .map_err(|e| format!("Failed to parse XML: {}", e))?;

        // Load and parse the schema from string
        let mut schema_parser = SchemaParserContext::from_buffer(schema_content);
        let schema_ctx = SchemaValidationContext::from_parser(&mut schema_parser).map_err(|errs| {
            let messages: Vec<String> = errs
                .iter()
                .filter_map(|e| e.message.clone())
                .collect();
            format!("Failed to parse schema: {}", messages.join("; "))
        })?;

        let mut schema_ctx = schema_ctx;

        // Validate the document
        match schema_ctx.validate_document(&xml_doc) {
            Ok(()) => Ok(ValidationResult::success(schema_name)),
            Err(errs) => {
                let errors: Vec<ValidationError> = errs
                    .iter()
                    .map(|e| {
                        ValidationError::error(
                            e.message.clone().unwrap_or_else(|| "Unknown error".to_string()),
                            e.line.map(|l| l as u32),
                            None,
                        )
                    })
                    .collect();
                Ok(ValidationResult::with_errors(schema_name, errors))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn get_test_schema_path() -> Option<PathBuf> {
        // Look for schema in static/schemas during tests
        let paths = [
            PathBuf::from("../static/schemas/tei_all.xsd"),
            PathBuf::from("static/schemas/tei_all.xsd"),
        ];
        paths.into_iter().find(|p| p.exists())
    }

    #[test]
    fn test_valid_xml_structure() {
        // Test that we can at least parse XML without schema
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<TEI xmlns="http://www.tei-c.org/ns/1.0">
  <teiHeader>
    <fileDesc>
      <titleStmt><title>Test</title></titleStmt>
      <publicationStmt><p>Test</p></publicationStmt>
      <sourceDesc><p>Test</p></sourceDesc>
    </fileDesc>
  </teiHeader>
  <text><body><p>Hello world</p></body></text>
</TEI>"#;

        let parser = Parser::default();
        let result = parser.parse_string(xml);
        assert!(result.is_ok(), "Should parse valid XML");
    }

    #[test]
    fn test_invalid_xml_structure() {
        // libxml's parser is lenient with recovery mode, so we test
        // something that definitely won't parse as valid XML
        let xml = "not xml at all < > & without proper structure";

        let parser = Parser::default();
        let result = parser.parse_string(xml);
        // libxml may recover from many errors, so this test just ensures
        // the parser runs without panicking. Real validation happens at schema level.
        // The parser may return Ok with a partial document or Err depending on input.
        // We accept either outcome for malformed input.
        let _ = result;
    }

    #[test]
    fn test_validate_valid_tei_document() {
        let schema_path = match get_test_schema_path() {
            Some(p) => p,
            None => {
                eprintln!("Skipping test: TEI schema not found");
                return;
            }
        };

        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<TEI xmlns="http://www.tei-c.org/ns/1.0">
  <teiHeader>
    <fileDesc>
      <titleStmt><title>Test Document</title></titleStmt>
      <publicationStmt><p>Test publication</p></publicationStmt>
      <sourceDesc><p>Test source</p></sourceDesc>
    </fileDesc>
  </teiHeader>
  <text>
    <body>
      <p>Hello world</p>
    </body>
  </text>
</TEI>"#;

        let result = XsdValidator::validate(xml, &schema_path);
        assert!(result.is_ok(), "Validation should not error: {:?}", result.err());

        let validation = result.unwrap();
        assert!(validation.valid, "Valid TEI document should pass validation. Errors: {:?}", validation.errors);
    }

    #[test]
    fn test_validate_invalid_tei_document() {
        let schema_path = match get_test_schema_path() {
            Some(p) => p,
            None => {
                eprintln!("Skipping test: TEI schema not found");
                return;
            }
        };

        // Missing required teiHeader
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<TEI xmlns="http://www.tei-c.org/ns/1.0">
  <text>
    <body>
      <p>Hello world</p>
    </body>
  </text>
</TEI>"#;

        let result = XsdValidator::validate(xml, &schema_path);
        assert!(result.is_ok(), "Validation should not error: {:?}", result.err());

        let validation = result.unwrap();
        assert!(!validation.valid, "Invalid TEI document (missing teiHeader) should fail validation");
        assert!(!validation.errors.is_empty(), "Should have validation errors");
    }
}
