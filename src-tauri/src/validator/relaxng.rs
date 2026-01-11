use super::types::{ValidationError, ValidationResult};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::path::Path;

// FFI Definitions
#[repr(C)]
pub struct xmlRelaxNG {
    _private: [u8; 0],
}
#[repr(C)]
struct xmlRelaxNGParserCtxt {
    _private: [u8; 0],
}
#[repr(C)]
struct xmlRelaxNGValidCtxt {
    _private: [u8; 0],
}

// xmlError struct (simplified for what we need)
#[repr(C)]
struct xmlError {
    domain: c_int,
    code: c_int,
    message: *const c_char,
    level: c_int,
    file: *const c_char,
    line: c_int,
    str1: *const c_char,
    str2: *const c_char,
    str3: *const c_char,
    int1: c_int,
    int2: c_int,
    ctxt: *mut c_void,
    node: *mut c_void,
}

// Function pointer type for error handling
type XmlStructuredErrorFunc = extern "C" fn(ctx: *mut c_void, error: *const xmlError);

#[link(name = "xml2")]
extern "C" {
    fn xmlRelaxNGNewParserCtxt(url: *const c_char) -> *mut xmlRelaxNGParserCtxt;
    fn xmlRelaxNGNewMemParserCtxt(buffer: *const c_char, size: c_int) -> *mut xmlRelaxNGParserCtxt;
    fn xmlRelaxNGParse(ctxt: *mut xmlRelaxNGParserCtxt) -> *mut xmlRelaxNG;
    fn xmlRelaxNGFreeParserCtxt(ctxt: *mut xmlRelaxNGParserCtxt);
    fn xmlRelaxNGFree(schema: *mut xmlRelaxNG);
    
    fn xmlRelaxNGNewValidCtxt(schema: *mut xmlRelaxNG) -> *mut xmlRelaxNGValidCtxt;
    fn xmlRelaxNGFreeValidCtxt(ctxt: *mut xmlRelaxNGValidCtxt);
    fn xmlRelaxNGSetValidStructuredErrors(
        ctxt: *mut xmlRelaxNGValidCtxt,
        serror: XmlStructuredErrorFunc,
        ctx: *mut c_void,
    );
    fn xmlRelaxNGValidateDoc(ctxt: *mut xmlRelaxNGValidCtxt, doc: *mut c_void) -> c_int;
}

// Error collection context
struct ErrorContext {
    errors: Vec<ValidationError>,
}

extern "C" fn error_handler(ctx: *mut c_void, error: *const xmlError) {
    unsafe {
        if ctx.is_null() || error.is_null() {
            return;
        }
        
        let error_ctx = &mut *(ctx as *mut ErrorContext);
        let error_struct = &*error;
        
        let message = if !error_struct.message.is_null() {
            CStr::from_ptr(error_struct.message)
                .to_string_lossy()
                .trim()
                .to_string()
        } else {
            "Unknown error".to_string()
        };

        // libxml2 uses 0 for errors, 1 for warning, 2 for error, 3 for fatal
        let is_warning = error_struct.level == 1;

        error_ctx.errors.push(ValidationError {
            message,
            line: if error_struct.line > 0 { Some(error_struct.line as u32) } else { None },
            column: None,
            is_warning,
        });
    }
}

/// Safe wrapper around xmlRelaxNG pointer
pub struct RelaxNgSchema {
    ptr: *mut xmlRelaxNG,
}

// It is safe to send the pointer to another thread as long as we don't use it concurrently
// from multiple threads without synchronization. Since we use it in an actor (single thread),
// or pass ownership, this is fine. libxml2 schemas are generally read-only after parsing.
unsafe impl Send for RelaxNgSchema {}
unsafe impl Sync for RelaxNgSchema {}

impl Drop for RelaxNgSchema {
    fn drop(&mut self) {
        unsafe {
            if !self.ptr.is_null() {
                xmlRelaxNGFree(self.ptr);
            }
        }
    }
}

impl RelaxNgSchema {
    pub fn parse_file(path: &Path) -> Result<Self, String> {
        let path_str = path.to_str().ok_or("Invalid schema path")?;
        let c_url = CString::new(path_str).map_err(|_| "Invalid schema path CString")?;
        
        unsafe {
            let parser_ctxt = xmlRelaxNGNewParserCtxt(c_url.as_ptr());
            if parser_ctxt.is_null() {
                return Err("Failed to create RelaxNG parser context".to_string());
            }
            
            let schema = xmlRelaxNGParse(parser_ctxt);
            xmlRelaxNGFreeParserCtxt(parser_ctxt);
            
            if schema.is_null() {
                return Err("Failed to parse RelaxNG schema".to_string());
            }
            
            Ok(Self { ptr: schema })
        }
    }

    pub fn parse_string(content: &str) -> Result<Self, String> {
        let c_content = CString::new(content).map_err(|_| "Invalid schema content")?;
        let len = content.len() as c_int;
        
        unsafe {
            let parser_ctxt = xmlRelaxNGNewMemParserCtxt(c_content.as_ptr(), len);
            if parser_ctxt.is_null() {
                return Err("Failed to create RelaxNG parser context".to_string());
            }
            
            let schema = xmlRelaxNGParse(parser_ctxt);
            xmlRelaxNGFreeParserCtxt(parser_ctxt);
            
            if schema.is_null() {
                return Err("Failed to parse RelaxNG schema".to_string());
            }
            
            Ok(Self { ptr: schema })
        }
    }

    pub fn validate(&self, xml_content: &str, schema_name: &str) -> Result<ValidationResult, String> {
        // Parse XML using libxml crate
        let parser = libxml::parser::Parser::default();
        let doc = parser.parse_string(xml_content)
            .map_err(|e| format!("Failed to parse XML: {}", e))?;
        
        let doc_ptr = doc.doc_ptr();

        unsafe {
            let valid_ctxt = xmlRelaxNGNewValidCtxt(self.ptr);
            if valid_ctxt.is_null() {
                return Err("Failed to create RelaxNG validation context".to_string());
            }

            let mut error_ctx = ErrorContext { errors: Vec::new() };
            xmlRelaxNGSetValidStructuredErrors(
                valid_ctxt, 
                error_handler, 
                &mut error_ctx as *mut _ as *mut c_void
            );

            let ret = xmlRelaxNGValidateDoc(valid_ctxt, doc_ptr as *mut c_void);
            
            xmlRelaxNGFreeValidCtxt(valid_ctxt);

            if ret == 0 {
                Ok(ValidationResult::success(schema_name))
            } else if ret > 0 {
                Ok(ValidationResult::with_errors(schema_name, error_ctx.errors))
            } else {
                Err("Internal error during RelaxNG validation".to_string())
            }
        }
    }
}

#[allow(dead_code)]
pub struct RelaxNgValidator;

impl RelaxNgValidator {
    #[allow(dead_code)]
    pub fn validate(
        xml_content: &str, 
        schema_path: &Path
    ) -> Result<ValidationResult, String> {
        let schema_name = schema_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        let schema = RelaxNgSchema::parse_file(schema_path)?;
        schema.validate(xml_content, schema_name)
    }
    
    pub fn validate_with_schema_string(
        xml_content: &str,
        schema_content: &str,
        schema_name: &str,
    ) -> Result<ValidationResult, String> {
        let schema = RelaxNgSchema::parse_string(schema_content)?;
        schema.validate(xml_content, schema_name)
    }
}

#[cfg(test)]
#[path = "relaxng_tests.rs"]
mod tests;
