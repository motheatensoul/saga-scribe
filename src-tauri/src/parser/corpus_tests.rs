//! The shared DSL corpus (`tests/fixtures/dsl-corpus.json`): DSL inputs with
//! the top-level constructs this parser finds in them. The editor's
//! highlighting grammar (`src/lib/parser/tei-dsl.grammar`) is a second
//! implementation of the DSL; its test (`src/lib/parser/corpus.test.ts`)
//! checks that it finds the same constructs, so the two can't drift apart.
//!
//! The file's `cases` are written by hand (only `dsl` is needed), its
//! `generated` cases come from `generated_inputs`. After changing either, or
//! the parser, regenerate the expectations with
//! `UPDATE_DSL_CORPUS=1 cargo test corpus`.

use super::syntax::{self, SyntaxKind};
use serde_json::{json, Value};

const CORPUS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/fixtures/dsl-corpus.json");

/// The construct names shared with the grammar's test.
fn kind_name(kind: &SyntaxKind) -> &'static str {
    match kind {
        SyntaxKind::Text => "Text",
        SyntaxKind::LineBreak { .. } => "LineBreak",
        SyntaxKind::PageBreak { .. } => "PageBreak",
        SyntaxKind::Abbreviation { .. } => "Abbreviation",
        SyntaxKind::Gap { .. } => "Gap",
        SyntaxKind::Supplied { .. } => "Supplied",
        SyntaxKind::SuppliedBlock { .. } => "SuppliedBlock",
        SyntaxKind::Deletion { .. } => "Deletion",
        SyntaxKind::Addition { .. } => "Addition",
        SyntaxKind::Note { .. } => "Note",
        SyntaxKind::Head { .. } => "Head",
        SyntaxKind::Norm { .. } => "Norm",
        SyntaxKind::Unclear { .. } => "Unclear",
        SyntaxKind::Entity { .. } => "Entity",
        SyntaxKind::WordContinuation => "WordContinuation",
        SyntaxKind::CompoundJoin => "CompoundJoin",
        SyntaxKind::WordBoundary => "WordBoundary",
        SyntaxKind::ParagraphBreak => "ParagraphBreak",
    }
}

/// `[name, from, to]` for each top-level construct, in UTF-16 code units (as
/// the editor counts), adjacent text merged; `None` if `dsl` has errors.
fn constructs(dsl: &str) -> Option<Value> {
    let tree = syntax::parse(dsl);
    if !tree.diagnostics.is_empty() {
        return None;
    }
    // Byte offsets to UTF-16 ones, counted on from the previous (nodes are in order)
    let (mut byte, mut unit) = (0, 0);
    let mut utf16 = |to: usize| {
        unit += dsl[byte..to].encode_utf16().count();
        byte = to;
        unit
    };
    let mut out: Vec<(&str, usize, usize)> = Vec::new();
    for node in &tree.nodes {
        let (from, to) = (utf16(node.span.start), utf16(node.span.end));
        let name = kind_name(&node.kind);
        match out.last_mut() {
            Some(last) if name == "Text" && last.0 == "Text" && last.2 == from => last.2 = to,
            _ => out.push((name, from, to)),
        }
    }
    Some(Value::Array(out.into_iter().map(|(n, f, t)| json!([n, f, t])).collect()))
}

/// Random DSL made of the pieces the syntax is made of, kept where it's valid.
fn generated_inputs() -> Vec<String> {
    const PIECES: &[&str] = &[
        "//", "///", "~", "|", ":", "et", "a", "þ", " ", "\n", "1", "3v", "-", "+", "?", "^",
        "{", "}", "[", "]", "<", ">", "-{", "}-", "+{", "}+", "?{", "}?", "^{", "[...", ".abbr[",
        ".head{", ".supplied{", ".norm{", ".", "~//", "~///", ":et:", "ꝛ", "𝔞", ".p", "p", "h",
    ];
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = |n: usize| {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        ((state.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 33) as usize) % n
    };
    let mut inputs = Vec::new();
    while inputs.len() < 1000 {
        let len = 1 + next(16);
        let source: String = (0..len).map(|_| PIECES[next(PIECES.len())]).collect();
        if syntax::parse(&source).diagnostics.is_empty() && !inputs.contains(&source) {
            inputs.push(source);
        }
    }
    inputs
}

fn case(dsl: &str) -> String {
    let nodes = constructs(dsl).unwrap_or_else(|| panic!("corpus case {dsl:?} has syntax errors"));
    serde_json::to_string(&json!({ "dsl": dsl, "nodes": nodes })).unwrap()
}

#[test]
fn test_dsl_corpus_is_current() {
    let text = std::fs::read_to_string(CORPUS).unwrap();
    let corpus: Value = serde_json::from_str(&text).unwrap();
    let handwritten: Vec<String> = corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| case(c["dsl"].as_str().unwrap()))
        .collect();
    let generated: Vec<String> = generated_inputs().iter().map(|dsl| case(dsl)).collect();

    // One case per line, so changes read well in diffs
    let lines = |cases: &[String]| cases.iter().map(|c| format!("    {c}")).collect::<Vec<_>>().join(",\n");
    let expected = format!(
        "{{\n  \"about\": {},\n  \"cases\": [\n{}\n  ],\n  \"generated\": [\n{}\n  ]\n}}\n",
        serde_json::to_string(&corpus["about"]).unwrap(),
        lines(&handwritten),
        lines(&generated),
    );
    if std::env::var_os("UPDATE_DSL_CORPUS").is_some() {
        std::fs::write(CORPUS, &expected).unwrap();
    } else {
        assert!(
            text == expected,
            "tests/fixtures/dsl-corpus.json is out of date: run `UPDATE_DSL_CORPUS=1 cargo test corpus` and check the diff"
        );
    }
}
