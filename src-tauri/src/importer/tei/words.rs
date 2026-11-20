//! What an imported file says about its words: lemma, analysis and the text
//! of each level, numbered as the rendered text counts words. The frontend
//! takes the file's lemmas from these (`importedLemmas`), so the manifest
//! needn't cross to it.
//!
//! The rules are those the frontend used on the segments' XML: a level's text
//! is its XML without tags, whitespace collapsed (references stay as
//! written, `&thorn;`), and words nested in another element stored as a word
//! (a `<head>`, `<supplied>`, …) count in document order, with their
//! attributes as written.

use super::patching::Originals;
use super::segments::Segment;

/// One word of an imported file.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedWord {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lemma: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msa: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diplomatic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facsimile: Option<String>,
}

/// Every word of an imported document, in order.
pub fn imported_words(segments: &[Segment], originals: Originals) -> Vec<ImportedWord> {
    let mut words = Vec::new();
    for segment in segments {
        if !matches!(segment, Segment::Word { .. }) {
            continue;
        }
        let xml = originals.xml(segment);
        let trimmed = xml.trim_start();
        if trimmed.starts_with("<w") {
            let attribute = |name: &str| originals.attribute(segment, name);
            words.push(ImportedWord {
                lemma: attribute("lemma"),
                msa: attribute("me:msa").or_else(|| attribute("msa")),
                ..levels(&xml)
            });
        } else if trimmed.starts_with('<') {
            words.extend(nested_words(trimmed));
        }
    }
    words
}

/// The text of each level in a word's XML.
fn levels(xml: &str) -> ImportedWord {
    ImportedWord {
        normalized: level_text(xml, "norm"),
        diplomatic: level_text(xml, "dipl"),
        facsimile: level_text(xml, "facs"),
        ..ImportedWord::default()
    }
}

/// The text of the first `<me:tag>` (or else `<tag>`) in `xml` that has any.
fn level_text(xml: &str, tag: &str) -> Option<String> {
    for name in [format!("me:{tag}"), tag.to_string()] {
        if let Some(content) = element_content(xml, &name) {
            let text = strip_tags(content);
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    None
}

/// The content of the first element `name` in `xml` (names compared without
/// regard to ASCII case): from the end of its start tag to the next end tag
/// of that name.
fn element_content<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let lower = xml.to_ascii_lowercase();
    let open = format!("<{}", name.to_ascii_lowercase());
    let close = format!("</{}>", name.to_ascii_lowercase());
    let mut from = 0;
    while let Some(at) = lower[from..].find(&open).map(|i| from + i) {
        let after = at + open.len();
        // The name ends there (`<me:dipl` isn't `<me:diplx`)
        let boundary = lower[after..].chars().next().is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
        if boundary {
            let start = after + lower[after..].find('>')? + 1;
            let end = start + lower[start..].find(&close)?;
            return xml.get(start..end);
        }
        from = after;
    }
    None
}

/// `text` without tags, whitespace collapsed and trimmed.
fn strip_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(lt) = rest.find('<') {
        out.push_str(&rest[..lt]);
        match rest[lt + 1..].find('>') {
            // An empty `<>` isn't a tag
            Some(gt) if gt > 0 => rest = &rest[lt + 1 + gt + 1..],
            _ => {
                out.push('<');
                rest = &rest[lt + 1..];
            }
        }
    }
    out.push_str(rest);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Each `<w>` in `xml`, in order, with its attributes as written.
fn nested_words(xml: &str) -> Vec<ImportedWord> {
    let mut words = Vec::new();
    let mut from = 0;
    while let Some(at) = xml[from..].find("<w").map(|i| from + i) {
        let after = at + 2;
        // `<w>` or `<w attributes>`
        let tag = match xml[after..].chars().next() {
            Some('>') => Some(("", after + 1)),
            Some(c) if c.is_whitespace() => xml[after..]
                .find('>')
                .map(|gt| (&xml[after + c.len_utf8()..after + gt], after + gt + 1)),
            _ => None,
        };
        let Some((attributes, start)) = tag else {
            from = after;
            continue;
        };
        let Some(end) = xml[start..].find("</w>").map(|i| start + i) else { break };
        let content = &xml[start..end];
        words.push(ImportedWord {
            lemma: quoted_attribute(attributes, "lemma"),
            msa: quoted_attribute(attributes, "me:msa").or_else(|| quoted_attribute(attributes, "msa")),
            ..levels(content)
        });
        from = end + 4;
    }
    words
}

/// The value of `name="…"` in a start tag's attributes, as written: the
/// first place `name="` occurs.
fn quoted_attribute(attributes: &str, name: &str) -> Option<String> {
    let key = format!("{name}=\"");
    let at = attributes.find(&key)? + key.len();
    let end = attributes[at..].find('"')?;
    Some(attributes[at..at + end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_are_text_without_tags() {
        let word = levels("<w><choice><me:facs>k<am>&kbar;</am>s</me:facs><me:dipl>k<ex>onung</ex>s</me:dipl><me:norm>konungs</me:norm></choice></w>");
        assert_eq!(word.facsimile.as_deref(), Some("k&kbar;s"));
        assert_eq!(word.diplomatic.as_deref(), Some("konungs"));
        assert_eq!(word.normalized.as_deref(), Some("konungs"));
        // Without MENOTA levels, none; an empty level is none
        assert_eq!(levels("<w>ok</w>"), ImportedWord::default());
        assert_eq!(levels("<w><me:norm> </me:norm><norm>ok</norm></w>").normalized.as_deref(), Some("ok"));
    }

    #[test]
    fn nested_words_count_in_order() {
        let words = nested_words(
            r#"<head><w lemma="saga" me:msa="xNC"><me:dipl>Saga</me:dipl></w> <wx/> <w><me:dipl>Njáls</me:dipl></w></head>"#,
        );
        assert_eq!(words.len(), 2);
        assert_eq!((words[0].lemma.as_deref(), words[0].msa.as_deref()), (Some("saga"), Some("xNC")));
        assert_eq!(words[1].diplomatic.as_deref(), Some("Njáls"));
        assert_eq!(quoted_attribute(r#"msa="a" me:msa="b""#, "me:msa").as_deref(), Some("b"));
    }
}

