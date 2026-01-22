//! # DSL Parser Module
//!
//! This module implements the core DSL-to-TEI-XML compilation pipeline for Saga-Scribe.
//!
//! ## Architecture Overview
//!
//! The parser follows a three-stage pipeline:
//!
//! ```text
//! DSL Text → [Lexer] → AST Nodes → [WordTokenizer] → Word-Wrapped Nodes → [Compiler] → TEI-XML
//! ```
//!
//! ### Stage 1: Lexical Analysis ([`Lexer`])
//! The lexer tokenizes the DSL input into an abstract syntax tree (AST). It recognizes
//! DSL constructs like `//` (line breaks), `.abbr[]{}`(abbreviations), `[...]` (gaps),
//! and `:entity:` (entity references).
//!
//! ### Stage 2: Word Tokenization ([`WordTokenizer`])
//! The word tokenizer groups flat AST nodes into TEI `<w>` (word) and `<pc>` (punctuation)
//! elements. It handles word boundaries from whitespace, punctuation, and explicit `|` markers.
//!
//! ### Stage 3: Compilation ([`Compiler`])
//! The compiler transforms the tokenized AST into TEI-XML. In multi-level mode, it generates
//! MENOTA-compliant three-level transcriptions with `<me:facs>`, `<me:dipl>`, and `<me:norm>`.
//!
//! ## DSL Syntax Quick Reference
//!
//! | Syntax | TEI Output | Description |
//! |--------|------------|-------------|
//! | `//` | `<lb/>` | Line break |
//! | `//n` | `<lb n="n"/>` | Numbered line break |
//! | `///n` | `<pb n="n"/>` | Page break |
//! | `.abbr[a]{b}` | `<choice><abbr>a</abbr><expan>b</expan></choice>` | Abbreviation |
//! | `[...]` | `<gap/>` | Gap/lacuna |
//! | `[...3]` | `<gap quantity="3"/>` | Gap with character count |
//! | `[...<txt>]` | `<gap/><supplied>txt</supplied>` | Gap with supplied reading |
//! | `<txt>` | `<supplied>txt</supplied>` | Editorial supplied text |
//! | `-{txt}-` | `<del>txt</del>` | Deletion |
//! | `+{txt}+` | `<add>txt</add>` | Addition |
//! | `?{txt}?` | `<unclear>txt</unclear>` | Unclear reading |
//! | `^{txt}` | `<note>txt</note>` | Editorial note |
//! | `:name:` | `&name;` | Entity reference |
//! | `~//` | Word continuation across line break | |
//! | `upp~haf` | Compound word join | |
//! | `\|` | Explicit word boundary | |
//!
//! ## Multi-Level MENOTA Output
//!
//! When `multi_level` is enabled, words are compiled to:
//! ```xml
//! <w>
//!   <choice>
//!     <me:facs>facsimile text</me:facs>
//!     <me:dipl>diplomatic text</me:dipl>
//!     <me:norm>normalized text</me:norm>
//!   </choice>
//! </w>
//! ```
//!
//! ## Usage Example
//!
//! ```rust,ignore
//! use saga_scribe::parser::{Compiler, CompilerConfig};
//!
//! let mut compiler = Compiler::new()
//!     .with_config(CompilerConfig {
//!         word_wrap: true,
//!         multi_level: true,
//!         ..Default::default()
//!     });
//!
//! let xml = compiler.compile("///1r //1 Hello world")?;
//! ```

mod ast;
mod compiler;
mod lexer;
mod wordtokenizer;

#[cfg(test)]
mod tests;

pub use compiler::{Compiler, CompilerConfig, LemmaMapping};

// Re-export for use by importer/patching
pub use ast::Node;
pub use lexer::Lexer;
pub use wordtokenizer::WordTokenizer;
