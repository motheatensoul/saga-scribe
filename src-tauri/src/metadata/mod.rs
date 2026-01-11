//! Manuscript metadata schema for TEI P5 and MENOTA documents.
//!
//! This module provides structured metadata types that map to TEI header elements,
//! allowing form-based editing instead of raw XML manipulation.

use serde::{Deserialize, Serialize};

/// Complete manuscript metadata covering TEI P5 header elements.
/// All fields are optional to support incremental editing.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    /// Title statement (titleStmt)
    #[serde(default)]
    pub title_stmt: TitleStmt,

    /// Publication statement (publicationStmt)
    #[serde(default)]
    pub publication_stmt: PublicationStmt,

    /// Source description - manuscript identifier (sourceDesc/msDesc/msIdentifier)
    #[serde(default)]
    pub ms_identifier: MsIdentifier,

    /// Manuscript contents summary (msDesc/msContents)
    #[serde(default)]
    pub ms_contents: MsContents,

    /// Physical description (msDesc/physDesc)
    #[serde(default)]
    pub phys_desc: PhysDesc,

    /// History of the manuscript (msDesc/history)
    #[serde(default)]
    pub history: History,

    /// Language profile (profileDesc/langUsage)
    #[serde(default)]
    pub languages: Vec<Language>,

    /// Encoding description notes (encodingDesc)
    #[serde(default)]
    pub encoding_notes: Option<String>,
}

/// Title statement - who created and is responsible for the digital edition
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TitleStmt {
    /// Main title of the transcription
    pub title: Option<String>,

    /// Subtitle or alternative title
    pub subtitle: Option<String>,

    /// Original author of the text (if known)
    pub author: Option<String>,

    /// Modern editor(s) responsible for the transcription
    #[serde(default)]
    pub editors: Vec<Person>,

    /// Other responsibility statements (transcriber, encoder, etc.)
    #[serde(default)]
    pub resp_stmts: Vec<RespStmt>,
}

/// A person with optional identifiers
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    /// Person's name
    pub name: String,

    /// Optional ORCID, VIAF, or other identifier
    pub identifier: Option<String>,

    /// Identifier type (orcid, viaf, etc.)
    pub identifier_type: Option<String>,
}

/// Responsibility statement - who did what
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RespStmt {
    /// What they did (e.g., "Transcription", "Encoding", "Proofreading")
    pub resp: String,

    /// Who did it
    pub name: String,
}

/// Publication information for the digital edition
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PublicationStmt {
    /// Publisher or institution
    pub publisher: Option<String>,

    /// Place of publication
    pub pub_place: Option<String>,

    /// Publication date (ISO format preferred)
    pub date: Option<String>,

    /// Distributor (if different from publisher)
    pub distributor: Option<String>,

    /// License or availability statement
    pub availability: Option<Availability>,

    /// Identifier for the digital edition (DOI, URN, etc.)
    pub idno: Option<String>,

    /// Type of identifier
    pub idno_type: Option<String>,
}

/// License and availability information
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Availability {
    /// Status: "free", "restricted", "unknown"
    pub status: Option<String>,

    /// License name (e.g., "CC BY 4.0")
    pub license: Option<String>,

    /// License URL
    pub license_url: Option<String>,

    /// Additional availability notes
    pub note: Option<String>,
}

/// Manuscript identifier - where the physical manuscript is held
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MsIdentifier {
    /// Country where manuscript is held
    pub country: Option<String>,

    /// City/settlement
    pub settlement: Option<String>,

    /// Institution/repository name
    pub repository: Option<String>,

    /// Collection name (if applicable)
    pub collection: Option<String>,

    /// Shelfmark/call number
    pub idno: Option<String>,

    /// Alternative identifiers
    #[serde(default)]
    pub alt_identifiers: Vec<AltIdentifier>,

    /// Common name for the manuscript (e.g., "Codex Regius")
    pub ms_name: Option<String>,
}

/// Alternative identifier
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AltIdentifier {
    /// Type of identifier (e.g., "former", "catalog")
    pub id_type: String,

    /// The identifier value
    pub idno: String,
}

/// Summary of manuscript contents
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MsContents {
    /// Brief summary of the manuscript contents
    pub summary: Option<String>,

    /// Primary language of the text (ISO 639 code)
    pub text_lang: Option<String>,

    /// Additional language notes
    pub text_lang_note: Option<String>,
}

/// Physical description of the manuscript
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PhysDesc {
    /// Material (parchment, paper, etc.)
    pub material: Option<String>,

    /// Extent (number of leaves, dimensions)
    pub extent: Option<String>,

    /// Foliation description
    pub foliation: Option<String>,

    /// Layout description (columns, lines per page)
    pub layout: Option<String>,

    /// Description of scribal hands
    #[serde(default)]
    pub hands: Vec<HandNote>,

    /// General condition
    pub condition: Option<String>,
}

/// Description of a scribal hand
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HandNote {
    /// Hand identifier (e.g., "H1", "main")
    pub id: Option<String>,

    /// Script type (e.g., "carolingian", "gothic")
    pub script: Option<String>,

    /// Scope (e.g., "major", "minor", "sole")
    pub scope: Option<String>,

    /// Description
    pub description: Option<String>,
}

/// Historical information about the manuscript
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct History {
    /// When the manuscript was created
    pub orig_date: Option<DateRange>,

    /// Where the manuscript was created
    pub orig_place: Option<String>,

    /// Provenance notes
    pub provenance: Option<String>,

    /// How the repository acquired it
    pub acquisition: Option<String>,
}

/// A date or date range
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DateRange {
    /// Displayed date text (e.g., "ca. 1270-1280")
    pub display: Option<String>,

    /// ISO date for not-before (YYYY or YYYY-MM-DD)
    pub not_before: Option<String>,

    /// ISO date for not-after
    pub not_after: Option<String>,

    /// Single date if known exactly
    pub when: Option<String>,
}

/// Language declaration
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Language {
    /// ISO 639 language code (e.g., "non" for Old Norse)
    pub ident: String,

    /// Percentage of text in this language (optional)
    pub usage: Option<u8>,

    /// Human-readable name
    pub name: Option<String>,
}

impl Metadata {
    /// Create empty metadata with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if metadata has any meaningful content
    pub fn is_empty(&self) -> bool {
        self.title_stmt.title.is_none()
            && self.title_stmt.author.is_none()
            && self.title_stmt.editors.is_empty()
            && self.ms_identifier.repository.is_none()
            && self.ms_identifier.idno.is_none()
            && self.languages.is_empty()
    }

    /// Generate TEI XML for the teiHeader based on metadata
    pub fn to_tei_header(&self, include_menota_ns: bool) -> String {
        let mut xml = String::new();

        // XML declaration and TEI root
        xml.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
        xml.push('\n');

        if include_menota_ns {
            xml.push_str(
                r#"<TEI xmlns="http://www.tei-c.org/ns/1.0" xmlns:me="http://www.menota.org/ns/1.0">"#,
            );
        } else {
            xml.push_str(r#"<TEI xmlns="http://www.tei-c.org/ns/1.0">"#);
        }
        xml.push('\n');

        xml.push_str("  <teiHeader>\n");
        xml.push_str("    <fileDesc>\n");

        // titleStmt
        xml.push_str(&self.title_stmt_to_xml());

        // publicationStmt
        xml.push_str(&self.publication_stmt_to_xml());

        // sourceDesc
        xml.push_str(&self.source_desc_to_xml());

        xml.push_str("    </fileDesc>\n");

        // encodingDesc (if notes present)
        if let Some(ref notes) = self.encoding_notes {
            if !notes.is_empty() {
                xml.push_str("    <encodingDesc>\n");
                xml.push_str("      <editorialDecl>\n");
                xml.push_str("        <normalization>\n");
                xml.push_str(&format!("          <p>{}</p>\n", escape_xml(notes)));
                xml.push_str("        </normalization>\n");
                xml.push_str("      </editorialDecl>\n");
                xml.push_str("    </encodingDesc>\n");
            }
        }

        // profileDesc (languages)
        if !self.languages.is_empty() {
            xml.push_str("    <profileDesc>\n");
            xml.push_str("      <langUsage>\n");
            for lang in &self.languages {
                let mut attrs = format!(r#"ident="{}""#, escape_xml(&lang.ident));
                if let Some(usage) = lang.usage {
                    attrs.push_str(&format!(r#" usage="{}""#, usage));
                }
                let content = lang.name.as_deref().unwrap_or(&lang.ident);
                xml.push_str(&format!(
                    "        <language {}>{}</language>\n",
                    attrs,
                    escape_xml(content)
                ));
            }
            xml.push_str("      </langUsage>\n");
            xml.push_str("    </profileDesc>\n");
        }

        xml.push_str("  </teiHeader>\n");
        xml.push_str("  <text>\n");
        xml.push_str("    <body>\n");

        xml
    }

    fn title_stmt_to_xml(&self) -> String {
        let mut xml = String::from("      <titleStmt>\n");

        // Title (required in TEI, use placeholder if empty)
        let title = self
            .title_stmt
            .title
            .as_deref()
            .filter(|t| !t.is_empty())
            .unwrap_or("Untitled");
        xml.push_str(&format!("        <title>{}</title>\n", escape_xml(title)));

        // Subtitle
        if let Some(ref subtitle) = self.title_stmt.subtitle {
            if !subtitle.is_empty() {
                xml.push_str(&format!(
                    "        <title type=\"sub\">{}</title>\n",
                    escape_xml(subtitle)
                ));
            }
        }

        // Author
        if let Some(ref author) = self.title_stmt.author {
            if !author.is_empty() {
                xml.push_str(&format!("        <author>{}</author>\n", escape_xml(author)));
            }
        }

        // Editors
        for editor in &self.title_stmt.editors {
            if !editor.name.is_empty() {
                let mut elem = format!("<editor>{}</editor>", escape_xml(&editor.name));
                if let (Some(ref id), Some(ref id_type)) =
                    (&editor.identifier, &editor.identifier_type)
                {
                    elem = format!(
                        r#"<editor ref="{}:{}">{}</editor>"#,
                        escape_xml(id_type),
                        escape_xml(id),
                        escape_xml(&editor.name)
                    );
                }
                xml.push_str(&format!("        {}\n", elem));
            }
        }

        // Responsibility statements
        for resp in &self.title_stmt.resp_stmts {
            if !resp.name.is_empty() && !resp.resp.is_empty() {
                xml.push_str("        <respStmt>\n");
                xml.push_str(&format!("          <resp>{}</resp>\n", escape_xml(&resp.resp)));
                xml.push_str(&format!("          <name>{}</name>\n", escape_xml(&resp.name)));
                xml.push_str("        </respStmt>\n");
            }
        }

        xml.push_str("      </titleStmt>\n");
        xml
    }

    fn publication_stmt_to_xml(&self) -> String {
        let mut xml = String::from("      <publicationStmt>\n");
        let ps = &self.publication_stmt;

        // Publisher
        if let Some(ref publisher) = ps.publisher {
            if !publisher.is_empty() {
                xml.push_str(&format!(
                    "        <publisher>{}</publisher>\n",
                    escape_xml(publisher)
                ));
            }
        }

        // Publication place
        if let Some(ref place) = ps.pub_place {
            if !place.is_empty() {
                xml.push_str(&format!(
                    "        <pubPlace>{}</pubPlace>\n",
                    escape_xml(place)
                ));
            }
        }

        // Date
        if let Some(ref date) = ps.date {
            if !date.is_empty() {
                xml.push_str(&format!(
                    "        <date when=\"{}\">{}</date>\n",
                    escape_xml(date),
                    escape_xml(date)
                ));
            }
        }

        // Distributor
        if let Some(ref dist) = ps.distributor {
            if !dist.is_empty() {
                xml.push_str(&format!(
                    "        <distributor>{}</distributor>\n",
                    escape_xml(dist)
                ));
            }
        }

        // Identifier
        if let Some(ref idno) = ps.idno {
            if !idno.is_empty() {
                let type_attr = ps
                    .idno_type
                    .as_ref()
                    .map(|t| format!(r#" type="{}""#, escape_xml(t)))
                    .unwrap_or_default();
                xml.push_str(&format!(
                    "        <idno{}>{}</idno>\n",
                    type_attr,
                    escape_xml(idno)
                ));
            }
        }

        // Availability
        if let Some(ref avail) = ps.availability {
            let status_attr = avail
                .status
                .as_ref()
                .map(|s| format!(r#" status="{}""#, escape_xml(s)))
                .unwrap_or_default();
            xml.push_str(&format!("        <availability{}>\n", status_attr));

            if let Some(ref license) = avail.license {
                if !license.is_empty() {
                    let target_attr = avail
                        .license_url
                        .as_ref()
                        .map(|u| format!(r#" target="{}""#, escape_xml(u)))
                        .unwrap_or_default();
                    xml.push_str(&format!(
                        "          <licence{}>{}</licence>\n",
                        target_attr,
                        escape_xml(license)
                    ));
                }
            }

            if let Some(ref note) = avail.note {
                if !note.is_empty() {
                    xml.push_str(&format!("          <p>{}</p>\n", escape_xml(note)));
                }
            }

            xml.push_str("        </availability>\n");
        }

        // Fallback if nothing was added
        if xml == "      <publicationStmt>\n" {
            xml.push_str("        <p>Unpublished transcription</p>\n");
        }

        xml.push_str("      </publicationStmt>\n");
        xml
    }

    fn source_desc_to_xml(&self) -> String {
        let mut xml = String::from("      <sourceDesc>\n");

        // Check if we have manuscript description fields
        let has_ms_desc = self.ms_identifier.repository.is_some()
            || self.ms_identifier.idno.is_some()
            || self.ms_identifier.settlement.is_some()
            || self.ms_contents.summary.is_some()
            || !self.phys_desc.hands.is_empty()
            || self.history.orig_date.is_some();

        if has_ms_desc {
            xml.push_str("        <msDesc>\n");

            // msIdentifier
            xml.push_str("          <msIdentifier>\n");
            if let Some(ref country) = self.ms_identifier.country {
                if !country.is_empty() {
                    xml.push_str(&format!(
                        "            <country>{}</country>\n",
                        escape_xml(country)
                    ));
                }
            }
            if let Some(ref settlement) = self.ms_identifier.settlement {
                if !settlement.is_empty() {
                    xml.push_str(&format!(
                        "            <settlement>{}</settlement>\n",
                        escape_xml(settlement)
                    ));
                }
            }
            if let Some(ref repository) = self.ms_identifier.repository {
                if !repository.is_empty() {
                    xml.push_str(&format!(
                        "            <repository>{}</repository>\n",
                        escape_xml(repository)
                    ));
                }
            }
            if let Some(ref collection) = self.ms_identifier.collection {
                if !collection.is_empty() {
                    xml.push_str(&format!(
                        "            <collection>{}</collection>\n",
                        escape_xml(collection)
                    ));
                }
            }
            if let Some(ref idno) = self.ms_identifier.idno {
                if !idno.is_empty() {
                    xml.push_str(&format!(
                        "            <idno>{}</idno>\n",
                        escape_xml(idno)
                    ));
                }
            }
            if let Some(ref ms_name) = self.ms_identifier.ms_name {
                if !ms_name.is_empty() {
                    xml.push_str(&format!(
                        "            <msName>{}</msName>\n",
                        escape_xml(ms_name)
                    ));
                }
            }
            for alt in &self.ms_identifier.alt_identifiers {
                xml.push_str(&format!(
                    "            <altIdentifier type=\"{}\">\n              <idno>{}</idno>\n            </altIdentifier>\n",
                    escape_xml(&alt.id_type),
                    escape_xml(&alt.idno)
                ));
            }
            xml.push_str("          </msIdentifier>\n");

            // msContents
            if self.ms_contents.summary.is_some() || self.ms_contents.text_lang.is_some() {
                xml.push_str("          <msContents>\n");
                if let Some(ref summary) = self.ms_contents.summary {
                    if !summary.is_empty() {
                        xml.push_str(&format!(
                            "            <summary>{}</summary>\n",
                            escape_xml(summary)
                        ));
                    }
                }
                if let Some(ref lang) = self.ms_contents.text_lang {
                    if !lang.is_empty() {
                        let note = self
                            .ms_contents
                            .text_lang_note
                            .as_deref()
                            .unwrap_or(lang);
                        xml.push_str(&format!(
                            "            <textLang mainLang=\"{}\">{}</textLang>\n",
                            escape_xml(lang),
                            escape_xml(note)
                        ));
                    }
                }
                xml.push_str("          </msContents>\n");
            }

            // physDesc
            if self.phys_desc.material.is_some()
                || self.phys_desc.extent.is_some()
                || !self.phys_desc.hands.is_empty()
            {
                xml.push_str("          <physDesc>\n");

                if self.phys_desc.material.is_some() || self.phys_desc.extent.is_some() {
                    xml.push_str("            <objectDesc>\n");
                    xml.push_str("              <supportDesc>\n");
                    if let Some(ref material) = self.phys_desc.material {
                        if !material.is_empty() {
                            xml.push_str(&format!(
                                "                <support>{}</support>\n",
                                escape_xml(material)
                            ));
                        }
                    }
                    if let Some(ref extent) = self.phys_desc.extent {
                        if !extent.is_empty() {
                            xml.push_str(&format!(
                                "                <extent>{}</extent>\n",
                                escape_xml(extent)
                            ));
                        }
                    }
                    if let Some(ref foliation) = self.phys_desc.foliation {
                        if !foliation.is_empty() {
                            xml.push_str(&format!(
                                "                <foliation>{}</foliation>\n",
                                escape_xml(foliation)
                            ));
                        }
                    }
                    if let Some(ref condition) = self.phys_desc.condition {
                        if !condition.is_empty() {
                            xml.push_str(&format!(
                                "                <condition>{}</condition>\n",
                                escape_xml(condition)
                            ));
                        }
                    }
                    xml.push_str("              </supportDesc>\n");
                    if let Some(ref layout) = self.phys_desc.layout {
                        if !layout.is_empty() {
                            xml.push_str("              <layoutDesc>\n");
                            xml.push_str(&format!(
                                "                <layout>{}</layout>\n",
                                escape_xml(layout)
                            ));
                            xml.push_str("              </layoutDesc>\n");
                        }
                    }
                    xml.push_str("            </objectDesc>\n");
                }

                if !self.phys_desc.hands.is_empty() {
                    xml.push_str("            <handDesc>\n");
                    for hand in &self.phys_desc.hands {
                        let mut attrs = String::new();
                        if let Some(ref id) = hand.id {
                            attrs.push_str(&format!(r#" xml:id="{}""#, escape_xml(id)));
                        }
                        if let Some(ref script) = hand.script {
                            attrs.push_str(&format!(r#" script="{}""#, escape_xml(script)));
                        }
                        if let Some(ref scope) = hand.scope {
                            attrs.push_str(&format!(r#" scope="{}""#, escape_xml(scope)));
                        }
                        let desc = hand.description.as_deref().unwrap_or("");
                        xml.push_str(&format!(
                            "              <handNote{}>{}</handNote>\n",
                            attrs,
                            escape_xml(desc)
                        ));
                    }
                    xml.push_str("            </handDesc>\n");
                }

                xml.push_str("          </physDesc>\n");
            }

            // history
            if self.history.orig_date.is_some()
                || self.history.orig_place.is_some()
                || self.history.provenance.is_some()
            {
                xml.push_str("          <history>\n");

                if self.history.orig_date.is_some() || self.history.orig_place.is_some() {
                    xml.push_str("            <origin>\n");
                    if let Some(ref date) = self.history.orig_date {
                        let mut attrs = String::new();
                        if let Some(ref when) = date.when {
                            attrs.push_str(&format!(r#" when="{}""#, escape_xml(when)));
                        }
                        if let Some(ref nb) = date.not_before {
                            attrs.push_str(&format!(r#" notBefore="{}""#, escape_xml(nb)));
                        }
                        if let Some(ref na) = date.not_after {
                            attrs.push_str(&format!(r#" notAfter="{}""#, escape_xml(na)));
                        }
                        let display = date.display.as_deref().unwrap_or("");
                        xml.push_str(&format!(
                            "              <origDate{}>{}</origDate>\n",
                            attrs,
                            escape_xml(display)
                        ));
                    }
                    if let Some(ref place) = self.history.orig_place {
                        if !place.is_empty() {
                            xml.push_str(&format!(
                                "              <origPlace>{}</origPlace>\n",
                                escape_xml(place)
                            ));
                        }
                    }
                    xml.push_str("            </origin>\n");
                }

                if let Some(ref prov) = self.history.provenance {
                    if !prov.is_empty() {
                        xml.push_str(&format!(
                            "            <provenance>{}</provenance>\n",
                            escape_xml(prov)
                        ));
                    }
                }

                if let Some(ref acq) = self.history.acquisition {
                    if !acq.is_empty() {
                        xml.push_str(&format!(
                            "            <acquisition>{}</acquisition>\n",
                            escape_xml(acq)
                        ));
                    }
                }

                xml.push_str("          </history>\n");
            }

            xml.push_str("        </msDesc>\n");
        } else {
            // Fallback for empty source description
            xml.push_str("        <p>No source description available.</p>\n");
        }

        xml.push_str("      </sourceDesc>\n");
        xml
    }

    /// Generate just the TEI footer (closing tags)
    pub fn tei_footer() -> String {
        String::from(
            r#"    </body>
  </text>
</TEI>"#,
        )
    }
}

/// Escape special XML characters
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_metadata() {
        let meta = Metadata::new();
        assert!(meta.is_empty());
    }

    #[test]
    fn test_metadata_with_title() {
        let mut meta = Metadata::new();
        meta.title_stmt.title = Some("Test Title".to_string());
        assert!(!meta.is_empty());
    }

    #[test]
    fn test_basic_tei_header_generation() {
        let mut meta = Metadata::new();
        meta.title_stmt.title = Some("Njáls saga".to_string());
        meta.title_stmt.author = Some("Anonymous".to_string());
        meta.ms_identifier.settlement = Some("Reykjavík".to_string());
        meta.ms_identifier.repository = Some("Stofnun Árna Magnússonar".to_string());
        meta.ms_identifier.idno = Some("GKS 2870 4to".to_string());

        let header = meta.to_tei_header(false);

        assert!(header.contains("<title>Njáls saga</title>"));
        assert!(header.contains("<author>Anonymous</author>"));
        assert!(header.contains("<settlement>Reykjavík</settlement>"));
        assert!(header.contains("<repository>Stofnun Árna Magnússonar</repository>"));
        assert!(header.contains("<idno>GKS 2870 4to</idno>"));
    }

    #[test]
    fn test_menota_namespace() {
        let meta = Metadata::new();
        let header = meta.to_tei_header(true);
        assert!(header.contains(r#"xmlns:me="http://www.menota.org/ns/1.0""#));
    }

    #[test]
    fn test_escape_xml_chars() {
        let mut meta = Metadata::new();
        meta.title_stmt.title = Some("Title with <special> & \"chars\"".to_string());

        let header = meta.to_tei_header(false);
        assert!(header.contains("&lt;special&gt;"));
        assert!(header.contains("&amp;"));
        assert!(header.contains("&quot;chars&quot;"));
    }

    #[test]
    fn test_language_declaration() {
        let mut meta = Metadata::new();
        meta.title_stmt.title = Some("Test".to_string());
        meta.languages.push(Language {
            ident: "non".to_string(),
            usage: Some(95),
            name: Some("Old Norse".to_string()),
        });
        meta.languages.push(Language {
            ident: "lat".to_string(),
            usage: Some(5),
            name: Some("Latin".to_string()),
        });

        let header = meta.to_tei_header(false);
        assert!(header.contains(r#"<language ident="non" usage="95">Old Norse</language>"#));
        assert!(header.contains(r#"<language ident="lat" usage="5">Latin</language>"#));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut meta = Metadata::new();
        meta.title_stmt.title = Some("Test".to_string());
        meta.ms_identifier.idno = Some("MS 123".to_string());

        let json = serde_json::to_string(&meta).unwrap();
        let restored: Metadata = serde_json::from_str(&json).unwrap();

        assert_eq!(meta, restored);
    }
}
