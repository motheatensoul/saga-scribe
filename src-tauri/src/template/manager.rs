use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Template {
    pub id: String,
    pub name: String,
    pub description: String,
    pub header: String,
    pub footer: String,
    #[serde(default)]
    pub word_wrap: bool,
    #[serde(default)]
    pub auto_line_numbers: bool,
    #[serde(default)]
    pub multi_level: bool,
}

pub struct TemplateManager {
    templates_dir: PathBuf,
}

impl TemplateManager {
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        let app_data = app.path().app_data_dir().map_err(|e| e.to_string())?;

        let templates_dir = app_data.join("templates");
        fs::create_dir_all(&templates_dir).map_err(|e| e.to_string())?;

        Ok(Self { templates_dir })
    }

    pub fn list_templates(&self) -> Result<Vec<Template>, String> {
        let mut templates = Vec::new();

        // Include built-in templates
        templates.push(self.tei_p5_template());
        templates.push(self.menota_template());

        // Load user templates
        if let Ok(entries) = fs::read_dir(&self.templates_dir) {
            for entry in entries.flatten() {
                if entry.path().extension().is_some_and(|e| e == "json") {
                    if let Ok(content) = fs::read_to_string(entry.path()) {
                        if let Ok(template) = serde_json::from_str::<Template>(&content) {
                            templates.push(template);
                        }
                    }
                }
            }
        }

        Ok(templates)
    }

    pub fn get_template(&self, id: &str) -> Result<Template, String> {
        match id {
            "tei-p5" => Ok(self.tei_p5_template()),
            "menota" => Ok(self.menota_template()),
            _ => {
                let path = self.templates_dir.join(format!("{}.json", id));
                let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
                serde_json::from_str(&content).map_err(|e| e.to_string())
            }
        }
    }

    pub fn save_template(&self, template: &Template) -> Result<(), String> {
        let path = self.templates_dir.join(format!("{}.json", template.id));
        let content = serde_json::to_string_pretty(template).map_err(|e| e.to_string())?;
        fs::write(&path, content).map_err(|e| e.to_string())
    }

    pub fn delete_template(&self, id: &str) -> Result<(), String> {
        let path = self.templates_dir.join(format!("{}.json", id));
        if path.exists() {
            fs::remove_file(&path).map_err(|e| e.to_string())
        } else {
            Err(format!("Template '{}' not found", id))
        }
    }

    fn tei_p5_template(&self) -> Template {
        Template {
            id: "tei-p5".to_string(),
            name: "TEI P5".to_string(),
            description: "Standard TEI P5 document structure".to_string(),
            header: r#"<?xml version="1.0" encoding="UTF-8"?>
<TEI xmlns="http://www.tei-c.org/ns/1.0">
  <teiHeader>
    <fileDesc>
      <titleStmt>
        <title><!-- Title --></title>
      </titleStmt>
      <publicationStmt>
        <p><!-- Publication info --></p>
      </publicationStmt>
      <sourceDesc>
        <p><!-- Source description --></p>
      </sourceDesc>
    </fileDesc>
  </teiHeader>
  <text>
    <body>"#
                .to_string(),
            footer: r#"    </body>
  </text>
</TEI>"#
                .to_string(),
            word_wrap: false,
            auto_line_numbers: false,
            multi_level: false,
        }
    }

    fn menota_template(&self) -> Template {
        Template {
            id: "menota".to_string(),
            name: "Menota".to_string(),
            description: "Menota handbook compatible structure for medieval Nordic texts"
                .to_string(),
            header: r#"<?xml version="1.0" encoding="UTF-8"?>
<TEI xmlns="http://www.tei-c.org/ns/1.0" xmlns:me="http://www.menota.org/ns/1.0">
  <teiHeader>
    <fileDesc>
      <titleStmt>
        <title><!-- Title --></title>
      </titleStmt>
      <publicationStmt>
        <p><!-- Publication info --></p>
      </publicationStmt>
      <sourceDesc>
        <msDesc>
          <msIdentifier>
            <settlement><!-- Location --></settlement>
            <repository><!-- Repository --></repository>
            <idno><!-- Shelfmark --></idno>
          </msIdentifier>
        </msDesc>
      </sourceDesc>
    </fileDesc>
    <encodingDesc>
      <editorialDecl>
        <normalization>
          <p>Transcription follows Menota handbook v3.0</p>
        </normalization>
      </editorialDecl>
    </encodingDesc>
  </teiHeader>
  <text>
    <body>"#
                .to_string(),
            footer: r#"    </body>
  </text>
</TEI>"#
                .to_string(),
            word_wrap: true,
            auto_line_numbers: true,
            multi_level: true,
        }
    }
}
