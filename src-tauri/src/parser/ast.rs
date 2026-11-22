//! # Compiler Nodes
//!
//! The flat node sequence consumed by the word tokenizer and the compiler.
//!
//! The DSL itself is parsed into a hierarchical, span-annotated tree by
//! [`super::syntax`]; this flat form is that tree lowered for the existing
//! pipeline. Container nodes like `Word` and `Punctuation` are introduced by
//! the word tokenizer stage. Block constructs still carry their content as DSL
//! text here; the tree has it parsed.

use serde::{Deserialize, Serialize};

/// A node in the DSL abstract syntax tree.
///
/// The AST starts as a flat sequence from the lexer, then the word tokenizer
/// groups content into `Word` and `Punctuation` container nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Node {
    /// Plain text content
    Text(String),
    /// Line break: // or //n where n is optional line number
    LineBreak(Option<String>),
    /// Page break: ///n where n is the page number
    PageBreak(String),
    /// Abbreviation with expansion: .abbr[abbr]{expansion}
    Abbreviation { abbr: String, expansion: String },
    /// An abbreviation mark and its expansion, inside a word:
    /// `{mark=expansion}` (MENOTA's `<am>` on the facsimile level and `<ex>`
    /// on the diplomatic one); `mark` may be empty (`{=n}`)
    Expansion { mark: String, expansion: String },
    /// `{mark}`: an abbreviation mark whose expansion is the project's
    /// default for it (the abbreviation store)
    StoredExpansion { mark: String },
    /// Gap/lacuna: [...] or [...n] or [...<text>] or [...n<text>]
    Gap {
        /// Extent in characters; compiled as `quantity` with `unit="chars"`.
        /// (`unit` is optional in MENOTA's schema and only meaningful with a quantity.)
        quantity: Option<u32>,
        supplied: Option<String>,
    },
    /// Supplied text (standalone): <text>
    Supplied(String),
    /// Supplied block wrapper: .supplied{text}
    SuppliedBlock(String),
    /// Deletion: -{text}-
    Deletion(String),
    /// Addition: +{text}+
    Addition(String),
    /// Note: ^{text}
    Note(String),
    /// Heading: .head{text}
    Head(String),
    /// Normalized-only wrapper: .norm{text}
    Norm(String),
    /// Unclear reading: ?{text}?
    Unclear(String),
    /// Custom entity: :name:
    Entity(String),
    /// A character written with `\` before it: text, and part of the word
    /// even where it would otherwise be punctuation or syntax (`ꝼæ\;ðꝛ`)
    Literal(String),
    /// Numeral: `#.x.#` (`<num>`), its content as DSL text
    Numeral(String),
    /// Word continuation marker: ~ (used before line/page breaks)
    WordContinuation,
    /// Compound word join: ~ between words (upp~haf → upphaf in norm)
    /// Outputs space in facs/dipl but joins in norm
    CompoundJoin,
    /// Explicit word boundary marker: |
    WordBoundary,
    /// Paragraph break: `.p` (closes the open paragraph, starts a new one)
    ParagraphBreak,
    /// Column break: `///c=B` (`<cb n="B"/>`)
    ColumnBreak(String),
    /// Change of hand: `.hs{hand}` (`<handShift new="hand"/>`)
    HandShift(String),
    /// Space left in the manuscript: `.space[n]` (`<space quantity="n"/>`)
    Space(Option<String>),
    /// Markup inside a word (`*{A}*nnat`, `.c{S}tephanus`), its content as
    /// DSL text; `second` is the other half of a pair (`.corr{sic=corr}`)
    Inline { markup: Markup, content: String, second: Option<String> },
    /// The start of markup around tokens (`.q{` before the words it holds)
    Open(Markup),
    /// Between the halves of a pair around tokens (the `=` of `.corr{a b=c d}`)
    Middle(Markup),
    /// The end of markup around tokens
    Close(Markup),
    /// Word container (groups nodes into a single word)
    Word(Vec<Node>),
    /// Punctuation container (groups punctuation nodes)
    Punctuation(Vec<Node>),
}

/// An element the shorthand writes around text or tokens: `<q>`, `<hi>`,
/// `<seg>`, or a pair in a `<choice>` (`<sic>`/`<corr>`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Markup {
    /// The element (`q`, `persName`); `choice` for a pair
    pub name: String,
    pub attributes: Vec<(String, String)>,
    /// The two elements of a pair (`sic`, `corr`)
    pub pair: Option<(String, String)>,
    /// The shorthand before the content (`.q{`, `*{`, `.rdg[A]{`)
    pub opener: String,
    /// The shorthand after it (`}`, `}*`)
    pub closer: String,
}

impl Markup {
    fn attributes_xml(&self) -> String {
        self.attributes
            .iter()
            .map(|(name, value)| format!(" {name}=\"{}\"", escape_attribute(value)))
            .collect()
    }

    /// The start tag (both, for a pair: `<choice><sic>`).
    pub fn open_xml(&self) -> String {
        match &self.pair {
            Some((first, _)) => format!("<{}{}><{first}>", self.name, self.attributes_xml()),
            None => format!("<{}{}>", self.name, self.attributes_xml()),
        }
    }

    /// Between a pair's halves (`</sic><corr>`); empty for other markup.
    pub fn middle_xml(&self) -> String {
        match &self.pair {
            Some((first, second)) => format!("</{first}><{second}>"),
            None => String::new(),
        }
    }

    /// The end tag (both, for a pair: `</corr></choice>`).
    pub fn close_xml(&self) -> String {
        match &self.pair {
            Some((_, second)) => format!("</{second}></{}>", self.name),
            None => format!("</{}>", self.name),
        }
    }
}

fn escape_attribute(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('"', "&quot;")
}

/// A document is a sequence of nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub nodes: Vec<Node>,
}

impl Document {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}
