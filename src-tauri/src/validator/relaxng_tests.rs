
#[cfg(test)]
mod tests {
    
    use crate::validator::relaxng::{RelaxNgSchema, RelaxNgValidator};

    // A simple RelaxNG schema for testing (matches <root> with text content)
    const SIMPLE_RNG: &str = r#"
        <element name="root" xmlns="http://relaxng.org/ns/structure/1.0">
            <text/>
        </element>
    "#;

    // A more complex RelaxNG schema (matches <person> with <name> and optional <age>)
    const COMPLEX_RNG: &str = r#"
        <element name="person" xmlns="http://relaxng.org/ns/structure/1.0">
            <element name="name"><text/></element>
            <optional>
                <element name="age"><text/></element>
            </optional>
        </element>
    "#;

    #[test]
    fn test_relaxng_parse_string() {
        let result = RelaxNgSchema::parse_string(SIMPLE_RNG);
        assert!(result.is_ok(), "Should parse valid RelaxNG schema string");
    }

    #[test]
    fn test_relaxng_parse_invalid_string() {
        let invalid_rng = "<element name='root'><unknown/></element>"; // Missing namespace or invalid structure
        let result = RelaxNgSchema::parse_string(invalid_rng);
        assert!(result.is_err(), "Should fail to parse invalid RelaxNG schema");
    }

    #[test]
    fn test_relaxng_validate_valid_xml() {
        let schema = RelaxNgSchema::parse_string(SIMPLE_RNG).unwrap();
        let xml = "<root>Hello</root>";
        let result = schema.validate(xml, "test_schema");
        
        assert!(result.is_ok(), "Validation execution failed");
        let validation = result.unwrap();
        assert!(validation.valid, "Valid XML should pass validation");
        assert!(validation.errors.is_empty(), "Should have no errors");
    }

    #[test]
    fn test_relaxng_validate_invalid_xml() {
        let schema = RelaxNgSchema::parse_string(SIMPLE_RNG).unwrap();
        let xml = "<wrong>Hello</unknown>"; // Mismatched root
        let result = schema.validate(xml, "test_schema");
        
        assert!(result.is_ok(), "Validation execution failed");
        let validation = result.unwrap();
        assert!(!validation.valid, "Invalid XML should fail validation");
        assert!(!validation.errors.is_empty(), "Should have validation errors");
    }

    #[test]
    fn test_relaxng_validate_complex_valid() {
        let schema = RelaxNgSchema::parse_string(COMPLEX_RNG).unwrap();
        let xml1 = "<person><name>John</name><age>30</age></person>";
        let xml2 = "<person><name>Jane</name></person>";
        
        let val1 = schema.validate(xml1, "complex").unwrap();
        assert!(val1.valid, "Valid XML with optional element should pass");

        let val2 = schema.validate(xml2, "complex").unwrap();
        assert!(val2.valid, "Valid XML without optional element should pass");
    }

    #[test]
    fn test_relaxng_validate_complex_invalid() {
        let schema = RelaxNgSchema::parse_string(COMPLEX_RNG).unwrap();
        let xml = "<person><age>30</age></person>"; // Missing required name
        
        let val = schema.validate(xml, "complex").unwrap();
        assert!(!val.valid, "XML missing required element should fail");
        assert!(!val.errors.is_empty());
    }

    #[test]
    fn test_validator_static_method() {
        let xml = "<root>Test</root>";
        let result = RelaxNgValidator::validate_with_schema_string(xml, SIMPLE_RNG, "static_test");
        
        assert!(result.is_ok());
        assert!(result.unwrap().valid);
    }
}
