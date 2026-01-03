use crate::entities::EntityRegistry;
use crate::parser::{Compiler, CompilerConfig};

#[tauri::command]
pub fn compile_dsl(
    input: String,
    template_header: String,
    template_footer: String,
    word_wrap: Option<bool>,
    auto_line_numbers: Option<bool>,
    entities_json: Option<String>,
) -> Result<String, String> {
    // Load entities if provided
    let mut registry = EntityRegistry::new();
    if let Some(json) = entities_json {
        registry.load_from_str(&json)?;
    }

    // Configure compiler
    let config = CompilerConfig {
        word_wrap: word_wrap.unwrap_or(false),
        auto_line_numbers: auto_line_numbers.unwrap_or(false),
    };

    let mut compiler = Compiler::new()
        .with_entities(&registry)
        .with_config(config);

    let body = compiler.compile(&input)?;
    Ok(format!("{}\n{}\n{}", template_header, body, template_footer))
}
