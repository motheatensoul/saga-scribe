use crate::annotations::AnnotationSet;
use crate::entities::EntityRegistry;
use crate::identity::{numbers_by_key, renumbering, Guidance, Renumbering, Tracked};
use crate::importer::tei::patching::{compiler_config_for, render_body, write_lemmas, Patcher};
use crate::importer::tei::segments::ImportedDocument;
use crate::normalizer::LevelDictionary;
use crate::parser::{Compiler, CompilerConfig, LemmaMapping, WordKey};
use crate::perf;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::State;

/// Compile DSL input to TEI-XML.
///
/// This command is async to avoid blocking the UI during compilation.
/// The actual compilation runs on a blocking thread pool.
#[allow(clippy::too_many_arguments)]
#[tauri::command(async, rename_all = "camelCase")]
pub async fn compile_dsl(
    input: String,
    template_header: String,
    template_footer: String,
    word_wrap: Option<bool>,
    auto_line_numbers: Option<bool>,
    multi_level: Option<bool>,
    wrap_pages: Option<bool>,
    entities_json: Option<String>,
    normalizer_json: Option<String>,
    entity_mappings_json: Option<String>,
    custom_mappings: Option<HashMap<String, String>>,
    lemma_mappings_json: Option<String>,
    annotations_json: Option<String>,
    guidance: Option<Guidance>,
    xml_base: Option<u64>,
    session: State<'_, NativeSession>,
    sent: State<'_, SentOutput>,
) -> Result<CompileReply, String> {
    let session = session.inner().clone();
    // Move all the work to a blocking thread pool to avoid blocking the UI
    tauri::async_runtime::spawn_blocking(move || {
        let total = perf::timer();
        let mut timer = perf::timer();
        // Load entities if provided
        let mut registry = EntityRegistry::new();
        if let Some(json) = entities_json {
            registry.load_from_str(&json)?;
        }

        // Load level dictionary if provided
        let mut dictionary = match normalizer_json {
            Some(json) => Some(LevelDictionary::load(&json)?),
            None => None,
        };

        // Load entity base letter mappings if provided
        if let Some(ref mut dict) = dictionary {
            if let Some(ref json) = entity_mappings_json {
                dict.load_entity_mappings(json)?;
            }
            // Apply custom mappings (overrides base mappings)
            if let Some(custom) = custom_mappings {
                dict.add_entity_mappings(custom);
            }
        }

        timer.lap("compile.load_entities", []);

        // Parse lemma mappings if provided (keyed by word INDEX)
        let lemma_json_bytes = lemma_mappings_json.as_ref().map_or(0, String::len);
        let lemma_mappings: HashMap<u32, LemmaMapping> = match lemma_mappings_json {
            Some(json) => serde_json::from_str(&json)
                .map_err(|e| format!("Failed to parse lemma mappings: {}", e))?,
            None => HashMap::new(),
        };

        // Parse annotations if provided
        let annotations: Option<AnnotationSet> = match annotations_json {
            Some(json) => Some(
                serde_json::from_str(&json)
                    .map_err(|e| format!("Failed to parse annotations: {}", e))?,
            ),
            None => None,
        };

        timer.lap(
            "compile.read_annotations",
            [("lemmas", lemma_mappings.len().into()), ("json_bytes", lemma_json_bytes.into())],
        );

        // Configure compiler
        let config = CompilerConfig {
            word_wrap: word_wrap.unwrap_or(false),
            auto_line_numbers: auto_line_numbers.unwrap_or(false),
            multi_level: multi_level.unwrap_or(false),
            wrap_pages: wrap_pages.unwrap_or(false),
        };

        let mut compiler = Compiler::new()
            .with_entities(&registry)
            .with_config(config)
            .with_lemma_mappings(lemma_mappings);

        // Add annotations if available
        if let Some(ref ann) = annotations {
            compiler = compiler.with_annotations(ann);
        }

        // Add dictionary if available
        if let Some(ref dict) = dictionary {
            compiler = compiler.with_dictionary(dict);
        }

        let mut state = session.0.lock().map_err(|_| "Compile state is poisoned")?;
        timer.lap("compile.wait_for_session", []);
        let (body, revision, renumbered) =
            compile_tracking_words(compiler, &input, guidance.as_ref(), &mut state)?;

        let xml = format!("{}\n{}\n{}", template_header, body, template_footer);
        total.done(
            "compile_dsl",
            [
                ("revision", revision.into()),
                ("guided", guidance.is_some().into()),
                ("dsl_bytes", input.len().into()),
                ("xml_bytes", xml.len().into()),
            ],
        );
        Ok(CompileOutput { xml, revision, renumbering: renumbered, edited_words: Vec::new() })
    })
    .await
    .map_err(|e| format!("Compilation task failed: {}", e))?
    .map(|output| sent.reply(output, xml_base))
}

/// The last native-mode compile: which token and word is which, so the next
/// compile can tell where words went.
#[derive(Default, Clone)]
pub struct NativeSession(Arc<Mutex<Option<NativeState>>>);

pub struct NativeState {
    tracked: Tracked,
    /// The key of every word, in document order
    words: Vec<WordKey>,
}

/// Compiles `input`, keeping track of which word is which (see
/// [`crate::identity`]): the compiler's annotations and lemma mappings are
/// numbered by the previous compile in `state` and follow their words.
/// Without `guidance` the document is taken to be new: no previous compile.
/// Returns the body, its revision and the renumbering of the previous words.
pub(crate) fn compile_tracking_words(
    compiler: Compiler,
    input: &str,
    guidance: Option<&Guidance>,
    state: &mut Option<NativeState>,
) -> Result<(String, u64, Option<Renumbering>), String> {
    let mut timer = perf::timer();
    let previous = state.as_ref().filter(|_| guidance.is_some());
    let tracked = Tracked::track(previous.map(|p| &p.tracked), guidance, input, 0)?;
    let previous_numbers = previous.map(|p| numbers_by_key(&p.words));
    let mut compiler = compiler.with_word_identity(tracked.ids.clone(), previous_numbers);
    timer.lap("compile.track_words", []);

    let body = compiler.compile(input)?;
    let words = compiler.word_keys().to_vec();
    timer.lap("compile.compile", [("words", words.len().into())]);
    let renumbered = previous.and_then(|p| renumbering(&p.words, &words));
    timer.done("compile.renumber", []);
    let revision = tracked.revision;
    *state = Some(NativeState { tracked, words });
    Ok((body, revision, renumbered))
}

/// The imported document being edited, held in the backend.
///
/// The segment manifest of a large manuscript is tens of megabytes of JSON.
/// Keeping it here means it crosses IPC once (via [`set_imported_document`])
/// instead of with every compile.
#[derive(Default, Clone)]
pub struct ImportedSession(Arc<Mutex<Option<ImportedSource>>>);

/// Everything needed to compile edited DSL back into the original file.
pub struct ImportedSource {
    document: ImportedDocument,
    original_body_xml: String,
    preamble: String,
    postamble: String,
    /// Which word is which across edits; built on the first compile
    patcher: Option<Patcher>,
    /// The key of every word of the last compile, in document order
    words: Option<Vec<WordKey>>,
}

impl ImportedSource {
    pub fn new(document: ImportedDocument, original_body_xml: String, preamble: String, postamble: String) -> Self {
        Self { document, original_body_xml, preamble, postamble, patcher: None, words: None }
    }
}

/// Sets (or, with no manifest, clears) the imported document used by [`compile_imported`].
#[tauri::command(async, rename_all = "camelCase")]
pub async fn set_imported_document(
    session: State<'_, ImportedSession>,
    segments_json: Option<String>,
    original_body_xml: Option<String>,
    preamble: Option<String>,
    postamble: Option<String>,
) -> Result<(), String> {
    let source = match segments_json {
        Some(json) => Some(
            tauri::async_runtime::spawn_blocking(move || {
                let timer = perf::timer();
                let json_bytes = json.len();
                let document = ImportedDocument::from_json(&json);
                timer.done("set_imported_document", [("json_bytes", json_bytes.into())]);
                document.map(|document| {
                    ImportedSource::new(
                        document,
                        original_body_xml.unwrap_or_default(),
                        preamble.unwrap_or_default(),
                        postamble.unwrap_or_default(),
                    )
                })
            })
            .await
            .map_err(|e| format!("Loading imported document failed: {}", e))??,
        ),
        None => None,
    };
    *session.0.lock().map_err(|_| "Imported document state is poisoned")? = source;
    Ok(())
}

/// Compile an imported document using the patching system for round-trip fidelity.
///
/// Takes the edited DSL, computes the differences against the imported
/// document held in [`ImportedSession`], and splices them into the original
/// XML. Unedited content is emitted byte-for-byte.
///
/// # Errors
///
/// Returns an error if no imported document is loaded or the edited DSL does
/// not parse; callers must keep the previous output rather than writing anything.
#[allow(clippy::too_many_arguments)]
#[tauri::command(async, rename_all = "camelCase")]
pub async fn compile_imported(
    session: State<'_, ImportedSession>,
    edited_dsl: String,
    entities_json: Option<String>,
    normalizer_json: Option<String>,
    entity_mappings_json: Option<String>,
    custom_mappings: Option<HashMap<String, String>>,
    lemma_mappings_json: Option<String>,
    guidance: Option<Guidance>,
    xml_base: Option<u64>,
    sent: State<'_, SentOutput>,
) -> Result<CompileReply, String> {
    let session = session.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let total = perf::timer();
        let mut timer = perf::timer();
        // Confirmed lemmas, numbered by the previous compile (see CompileOutput)
        let lemma_json_bytes = lemma_mappings_json.as_ref().map_or(0, String::len);
        let lemmas: HashMap<u32, LemmaMapping> = match lemma_mappings_json {
            Some(json) => serde_json::from_str(&json)
                .map_err(|e| format!("Failed to parse lemma mappings: {}", e))?,
            None => HashMap::new(),
        };
        timer.lap(
            "compile.read_annotations",
            [("lemmas", lemmas.len().into()), ("json_bytes", lemma_json_bytes.into())],
        );
        let mut guard = session
            .0
            .lock()
            .map_err(|_| "Imported document state is poisoned")?;
        let source = guard.as_mut().ok_or("No imported document is loaded")?;
        timer.lap("compile.wait_for_session", []);

        // Load entities if provided
        let mut registry = EntityRegistry::new();
        if let Some(json) = entities_json {
            registry.load_from_str(&json)?;
        }

        // Load level dictionary if provided
        let mut dictionary = match normalizer_json {
            Some(json) => Some(LevelDictionary::load(&json)?),
            None => None,
        };

        // Load entity base letter mappings if provided
        if let Some(ref mut dict) = dictionary {
            if let Some(ref json) = entity_mappings_json {
                dict.load_entity_mappings(json)?;
            }
            if let Some(custom) = custom_mappings {
                dict.add_entity_mappings(custom);
            }
        }

        timer.done("compile.load_entities", []);

        // Match the document's own encoding style
        let config = compiler_config_for(&source.document);

        let mut compiler = Compiler::new()
            .with_entities(&registry)
            .with_config(config);

        if let Some(ref dict) = dictionary {
            compiler = compiler.with_dictionary(dict);
        }

        let output = compile_imported_source(source, &mut compiler, &edited_dsl, guidance.as_ref(), lemmas)?;
        total.done(
            "compile_imported",
            [
                ("revision", output.revision.into()),
                ("guided", guidance.is_some().into()),
                ("dsl_bytes", edited_dsl.len().into()),
                ("xml_bytes", output.xml.len().into()),
            ],
        );
        Ok(output)
    })
    .await
    .map_err(|e| format!("Compilation task failed: {}", e))?
    .map(|output| sent.reply(output, xml_base))
}

/// Compiles edited DSL into the imported document `source` (see
/// [`compile_imported`]): splices the edits into the original, keeps track of
/// which word is which, and writes confirmed `lemmas` (numbered by the
/// previous compile) into the words.
pub(crate) fn compile_imported_source(
    source: &mut ImportedSource,
    compiler: &mut Compiler,
    edited_dsl: &str,
    guidance: Option<&Guidance>,
    lemmas: HashMap<u32, LemmaMapping>,
) -> Result<CompileOutput, String> {
    // Compute patches between original segments and edited DSL, using the
    // editor's changes to keep track of which word is which
    let mut timer = perf::timer();
    let patcher = source.patcher.get_or_insert_with(|| {
        Patcher::new(&source.document.segments, source.document.dsl_rules)
    });
    let (patches, state) = patcher.compute(edited_dsl, guidance)?;
    timer.lap("compile.compute_patches", [("patches", patches.len().into())]);

    // Apply patches to the original body XML
    let body = render_body(
        &source.document,
        Some(&source.original_body_xml),
        &patches,
        compiler,
    )?;
    timer.lap("compile.render_body", []);

    // Which word is which: lemmas follow their words (no guidance: the
    // document was just loaded, numbered as it is now)
    let patcher = source.patcher.get_or_insert_with(|| {
        Patcher::new(&source.document.segments, source.document.dsl_rules)
    });
    let words = patcher.word_keys(&source.document, &body, &state);
    timer.lap("compile.word_keys", [("words", words.len().into())]);
    let previous = source.words.as_ref().filter(|_| guidance.is_some());
    let renumbered = previous.and_then(|previous| renumbering(previous, &words));
    let lemmas = match previous {
        Some(previous) => {
            let now = numbers_by_key(&words);
            lemmas
                .into_iter()
                .filter_map(|(n, lemma)| Some((*now.get(previous.get(n as usize)?)?, lemma)))
                .collect()
        }
        None => lemmas,
    };
    timer.lap("compile.renumber", []);
    let body_xml = write_lemmas(&body, &lemmas, source.document.is_menota);
    timer.lap("compile.write_lemmas", [("lemmas", lemmas.len().into())]);

    // Only a successful compile becomes the basis for the next one
    patcher.commit(state);
    source.words = Some(words);

    // Combine with preamble (everything before body) and postamble (everything after)
    Ok(CompileOutput {
        revision: patcher.revision(),
        renumbering: renumbered,
        edited_words: body.edited_word_indices(),
        xml: format!("{}{}{}", source.preamble, body_xml, source.postamble),
    })
}

/// The result of [`compile_dsl`] and [`compile_imported`].
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompileOutput {
    /// The full document
    pub xml: String,
    /// Identifies this compile's DSL; send it back as [`Guidance::base_revision`]
    /// with the editor's changes since
    pub revision: u64,
    /// Where the previous compile's words went: for each word number of the
    /// previous compile, its number now (`null` if deleted); absent if no
    /// word moved. Annotations stored by word number must be renumbered so.
    pub renumbering: Option<Renumbering>,
    /// Words (by number, in document order) edited in the DSL whose lemma, msa
    /// and normalized form were kept from the original and should be reviewed
    pub edited_words: Vec<u32>,
}

/// What the frontend gets of a compile: the document whole, or only what
/// changed since the output it holds (see [`SentOutput`]).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompileReply {
    /// The full document, unless sent as `delta`
    pub xml: Option<String>,
    /// The change to the output of revision `xml_base` (as the frontend asked)
    pub delta: Option<XmlDelta>,
    pub revision: u64,
    pub renumbering: Option<Renumbering>,
    pub edited_words: Vec<u32>,
}

/// A change to a document: `from..to` (in UTF-16 code units, as JavaScript
/// counts) replaced by `insert`.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct XmlDelta {
    pub from: usize,
    pub to: usize,
    pub insert: String,
}

/// The last output sent to the frontend, by revision. A large manuscript's
/// XML is megabytes; a keystroke changes a few bytes of it, so the next
/// compile sends only that change, if the frontend still holds this output.
#[derive(Default)]
pub struct SentOutput(Mutex<Option<(u64, String)>>);

impl SentOutput {
    /// `output` as sent to a frontend holding the output of revision
    /// `xml_base`: as a change to it if this is that output, else whole.
    pub fn reply(&self, output: CompileOutput, xml_base: Option<u64>) -> CompileReply {
        let mut timer = perf::timer();
        let mut last = self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let delta = match (&*last, xml_base) {
            (Some((revision, previous)), Some(base)) if *revision == base => Some(xml_delta(previous, &output.xml)),
            _ => None,
        };
        timer.lap(
            "compile.xml_delta",
            [("delta", delta.is_some().into()), ("insert_bytes", delta.as_ref().map_or(0, |d| d.insert.len()).into())],
        );
        let CompileOutput { xml, revision, renumbering, edited_words } = output;
        let reply = CompileReply {
            xml: if delta.is_some() { None } else { Some(xml.clone()) },
            delta,
            revision,
            renumbering,
            edited_words,
        };
        *last = Some((revision, xml));
        reply
    }
}

/// Length of the common prefix of `a` and `b` in bytes, ending on a char boundary.
fn common_prefix(a: &str, b: &str) -> usize {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let max = a.len().min(b.len());
    // Whole blocks first (compared with memcmp, fast in debug builds too)
    const BLOCK: usize = 4096;
    let mut n = 0;
    while n + BLOCK <= max && a[n..n + BLOCK] == b[n..n + BLOCK] {
        n += BLOCK;
    }
    while n < max && a[n] == b[n] {
        n += 1;
    }
    // Not inside a character (a continuation byte means one started before)
    while n > 0 && (a.get(n).is_some_and(|&c| c & 0xC0 == 0x80) || b.get(n).is_some_and(|&c| c & 0xC0 == 0x80)) {
        n -= 1;
    }
    n
}

/// Length of the common suffix of `a` and `b` in bytes, at most `max`, starting on a char boundary.
fn common_suffix(a: &str, b: &str, max: usize) -> usize {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    const BLOCK: usize = 4096;
    let mut n = 0;
    while n + BLOCK <= max && a[a.len() - n - BLOCK..a.len() - n] == b[b.len() - n - BLOCK..b.len() - n] {
        n += BLOCK;
    }
    while n < max && a[a.len() - n - 1] == b[b.len() - n - 1] {
        n += 1;
    }
    while n > 0 && a[a.len() - n] & 0xC0 == 0x80 {
        n -= 1;
    }
    n
}

/// UTF-16 code units in `bytes` (valid UTF-8): one per character, two for
/// those of four bytes.
fn utf16_len(bytes: &[u8]) -> usize {
    bytes.iter().map(|&b| usize::from(b & 0xC0 != 0x80) + usize::from(b >= 0xF0)).sum()
}

/// The change turning `previous` into `next`.
fn xml_delta(previous: &str, next: &str) -> XmlDelta {
    let prefix = common_prefix(previous, next);
    let suffix = common_suffix(previous, next, previous.len().min(next.len()) - prefix);
    let from = utf16_len(&previous.as_bytes()[..prefix]);
    XmlDelta {
        from,
        to: from + utf16_len(&previous.as_bytes()[prefix..previous.len() - suffix]),
        insert: next[prefix..next.len() - suffix].to_string(),
    }
}

/// A DSL syntax error, positioned for the editor.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct DslDiagnostic {
    /// Start offset in UTF-16 code units (the editor's positions)
    pub from: usize,
    /// End offset in UTF-16 code units, at most the end of the starting line
    pub to: usize,
    pub message: String,
}

/// Reports every syntax error in DSL source, for the editor's linter.
#[tauri::command(async)]
pub async fn check_dsl(input: String) -> Vec<DslDiagnostic> {
    let timer = perf::timer();
    let diagnostics = dsl_diagnostics(&input);
    timer.done("check_dsl", [("dsl_bytes", input.len().into()), ("errors", diagnostics.len().into())]);
    diagnostics
}

fn dsl_diagnostics(input: &str) -> Vec<DslDiagnostic> {
    let utf16 = |byte: usize| input[..byte].encode_utf16().count();
    crate::parser::syntax::parse(input)
        .diagnostics
        .into_iter()
        .map(|d| {
            // An unclosed bracket's span runs to the end of the document;
            // underlining the rest of its line is enough to point at it.
            let line_end = input[d.span.start..]
                .find('\n')
                .map_or(input.len(), |i| d.span.start + i);
            let end = d.span.end.min(line_end).max(d.span.start + 1).min(input.len());
            DslDiagnostic {
                from: utf16(d.span.start),
                to: utf16(end),
                message: d.message,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `delta` applied to `previous` as JavaScript would, in UTF-16 units.
    fn apply_delta(previous: &str, delta: &XmlDelta) -> String {
        let units: Vec<u16> = previous.encode_utf16().collect();
        let mut out = String::from_utf16(&units[..delta.from]).unwrap();
        out.push_str(&delta.insert);
        out.push_str(&String::from_utf16(&units[delta.to..]).unwrap());
        out
    }

    #[test]
    fn test_xml_delta_rebuilds_the_new_document() {
        let long = "ꝛ𝔞 þat ".repeat(2000);
        let cases = [
            ("", ""),
            ("abc", "abc"),
            ("abc", "abXc"),
            ("abc", ""),
            ("", "abc"),
            ("aþc", "aðc"),
            ("a𝔞c", "a𝔟c"),
            ("ꝛꝛꝛ", "ꝛꝛ"),
            ("𝔞𝔞", "𝔞"),
        ];
        for (previous, next) in cases {
            assert_eq!(apply_delta(previous, &xml_delta(previous, next)), next, "{previous:?} -> {next:?}");
        }
        // Long enough for the block comparisons, edited in the middle and at the ends
        let mid = long.char_indices().nth(7000).unwrap().0;
        for next in [
            format!("{}𝔟{}", &long[..mid], &long[mid..]),
            format!("{}{}", &long[..mid], &long[mid + 'ꝛ'.len_utf8().min(long.len() - mid)..]),
            format!("x{long}"),
            format!("{long}x"),
        ] {
            let delta = xml_delta(&long, &next);
            assert_eq!(apply_delta(&long, &delta), next);
            assert!(delta.insert.len() <= 4, "{:?}", delta.insert);
        }
    }

    #[test]
    fn test_output_is_sent_as_a_change_only_to_the_output_the_frontend_holds() {
        let sent = SentOutput::default();
        let output = |xml: &str, revision| CompileOutput { xml: xml.into(), revision, renumbering: None, edited_words: vec![] };
        // Nothing held yet: whole
        let first = sent.reply(output("<w>ok</w>", 1), Some(1));
        assert_eq!((first.xml.as_deref(), first.delta), (Some("<w>ok</w>"), None));
        // Holding revision 1: the change to it
        let second = sent.reply(output("<w>ek</w>", 2), Some(1));
        assert_eq!(second.xml, None);
        assert_eq!(second.delta, Some(XmlDelta { from: 3, to: 4, insert: "e".into() }));
        // Holding something else (or nothing): whole
        assert_eq!(sent.reply(output("<w>ak</w>", 3), Some(1)).xml.as_deref(), Some("<w>ak</w>"));
        assert_eq!(sent.reply(output("<w>uk</w>", 4), None).xml.as_deref(), Some("<w>uk</w>"));
    }

    #[test]
    fn test_dsl_diagnostics_use_utf16_offsets_and_stay_on_one_line() {
        // "𝔞" is outside the BMP: 4 bytes, 2 UTF-16 units
        let input = "𝔞 -{open\nnext line";
        let diagnostics = dsl_diagnostics(input);
        assert_eq!(
            diagnostics,
            vec![DslDiagnostic {
                from: 3,
                to: 9,
                message: "Unclosed bracket, expected '}'".to_string(),
            }]
        );
    }

    #[test]
    fn test_dsl_diagnostics_empty_for_valid_input() {
        assert!(dsl_diagnostics(".head{//1 a :et:} -{x}-").is_empty());
    }
}

#[cfg(test)]
mod word_identity_tests {
    use super::*;
    use crate::identity::TextChange;

    /// Compiles `dsl` word-wrapped with lemmas `lemmas` (numbered as the
    /// frontend holds them), as the native compile command does.
    fn compile(
        state: &mut Option<NativeState>,
        dsl: &str,
        guidance: Option<Guidance>,
        lemmas: &[(u32, &str)],
    ) -> (String, Option<Renumbering>) {
        let mappings: HashMap<u32, LemmaMapping> = lemmas
            .iter()
            .map(|(n, lemma)| (*n, LemmaMapping { lemma: lemma.to_string(), msa: String::new(), normalized: None }))
            .collect();
        let compiler = Compiler::new()
            .with_config(CompilerConfig { word_wrap: true, ..Default::default() })
            .with_lemma_mappings(mappings);
        let (body, _, renumbered) = compile_tracking_words(compiler, dsl, guidance.as_ref(), state).unwrap();
        (body, renumbered)
    }

    /// Whether `word` is written with lemma `lemma`.
    fn has_lemma(body: &str, lemma: &str, word: &str) -> bool {
        body.contains(&format!(r#"<w lemma="{lemma}" me:msa="">{word}</w>"#))
    }

    /// Guidance for one edit since the last compile in `state`.
    fn edit(state: &Option<NativeState>, from_a: usize, to_a: usize, inserted: usize) -> Option<Guidance> {
        let base_revision = state.as_ref().unwrap().tracked.revision;
        Some(Guidance {
            base_revision,
            changes: vec![TextChange { from_a, to_a, from_b: from_a, to_b: from_a + inserted }],
        })
    }

    #[test]
    fn test_lemma_follows_its_word_when_a_word_is_inserted_before_it() {
        let mut state = None;
        let (body, renumbered) = compile(&mut state, "a b c", None, &[(1, "bee")]);
        assert!(has_lemma(&body, "bee", "b"), "{body}");
        assert_eq!(renumbered, None);

        // "x " inserted at the start; the frontend still numbers b as 1
        let guidance = edit(&state, 0, 0, 2);
        let (body, renumbered) = compile(&mut state, "x a b c", guidance, &[(1, "bee")]);
        assert!(has_lemma(&body, "bee", "b"), "{body}");
        assert!(!has_lemma(&body, "bee", "a"), "{body}");
        assert_eq!(renumbered, Some(vec![Some(1), Some(2), Some(3)]));
    }

    #[test]
    fn test_deleted_word_is_reported_gone_and_its_lemma_lands_nowhere() {
        let mut state = None;
        compile(&mut state, "a b c", None, &[(1, "bee")]);
        // "b " deleted
        let guidance = edit(&state, 2, 4, 0);
        let (body, renumbered) = compile(&mut state, "a c", guidance, &[(1, "bee")]);
        assert!(!body.contains("bee"), "{body}");
        assert_eq!(renumbered, Some(vec![Some(0), None, Some(1)]));
    }

    #[test]
    fn test_without_guidance_a_document_is_new() {
        let mut state = None;
        compile(&mut state, "a b c", None, &[]);
        // Another document: numbered by itself, nothing renumbered
        let (body, renumbered) = compile(&mut state, "x y", None, &[(1, "why")]);
        assert!(has_lemma(&body, "why", "y"), "{body}");
        assert_eq!(renumbered, None);
    }

    #[test]
    fn test_words_in_headings_are_numbered_like_the_previews() {
        // The previews count every <w>: x=0, a=1, b=2 (in the heading), y=3
        let mut state = None;
        let (body, _) = compile(&mut state, "x .head{a b} y", None, &[(2, "bee"), (3, "why")]);
        assert!(has_lemma(&body, "bee", "b"), "{body}");
        assert!(has_lemma(&body, "why", "y"), "{body}");
    }

    #[test]
    fn test_out_of_step_guidance_still_follows_words() {
        let mut state = None;
        compile(&mut state, "a b c", None, &[(1, "bee")]);
        let stale = Some(Guidance { base_revision: 99, changes: vec![] });
        let (body, renumbered) = compile(&mut state, "x a b c", stale, &[(1, "bee")]);
        assert!(has_lemma(&body, "bee", "b"), "{body}");
        assert_eq!(renumbered, Some(vec![Some(1), Some(2), Some(3)]));
    }
}

#[cfg(test)]
mod imported_identity_tests {
    use super::*;
    use crate::identity::TextChange;

    const TWO_OKS: &str = r#"<TEI xmlns:me="http://www.menota.org/ns/1.0"><text><body><p><w lemma="ok" me:msa="xCC"><choice><me:facs>ok</me:facs><me:dipl>ok</me:dipl><me:norm>ok</me:norm></choice></w> <w><choice><me:facs>maþr</me:facs><me:dipl>maþr</me:dipl><me:norm>maðr</me:norm></choice></w></p></body></text></TEI>"#;

    fn open(xml: &str) -> (ImportedSource, String) {
        let import = crate::importer::tei::parse(xml).unwrap();
        let source = ImportedSource::new(
            import.imported_document.unwrap(),
            import.original_body_xml.unwrap(),
            import.original_preamble.unwrap_or_default(),
            import.original_postamble.unwrap_or_default(),
        );
        (source, import.dsl)
    }

    fn compile(
        source: &mut ImportedSource,
        dsl: &str,
        guidance: Option<Guidance>,
        lemmas: &[(u32, &str, &str)],
    ) -> CompileOutput {
        let lemmas: Vec<_> = lemmas.iter().map(|(n, lemma, msa)| (*n, *lemma, *msa, None)).collect();
        compile_with_norms(source, dsl, guidance, &lemmas)
    }

    /// Like `compile`, with lemmas carrying normalized forms.
    fn compile_with_norms(
        source: &mut ImportedSource,
        dsl: &str,
        guidance: Option<Guidance>,
        lemmas: &[(u32, &str, &str, Option<&str>)],
    ) -> CompileOutput {
        let mut compiler = Compiler::new().with_config(compiler_config_for(&source.document));
        let lemmas = lemmas
            .iter()
            .map(|(n, lemma, msa, norm)| {
                let mapping = LemmaMapping { lemma: lemma.to_string(), msa: msa.to_string(), normalized: norm.map(str::to_string) };
                (*n, mapping)
            })
            .collect();
        compile_imported_source(source, &mut compiler, dsl, guidance.as_ref(), lemmas).unwrap()
    }

    #[test]
    fn test_lemmas_already_in_the_file_change_nothing() {
        let (mut source, dsl) = open(TWO_OKS);
        let out = compile(&mut source, &dsl, None, &[(0, "ok", "xCC")]);
        assert_eq!(out.xml, TWO_OKS);
    }

    #[test]
    fn test_confirmed_lemma_is_written_into_the_start_tag_only() {
        let (mut source, dsl) = open(TWO_OKS);
        let out = compile(&mut source, &dsl, None, &[(0, "ok", "xCC"), (1, "maðr", "xNC")]);
        assert_eq!(out.xml, TWO_OKS.replacen("<w><choice><me:facs>maþr", r#"<w lemma="maðr" me:msa="xNC"><choice><me:facs>maþr"#, 1));
    }

    #[test]
    fn test_lemma_follows_its_word_through_an_insertion() {
        let (mut source, dsl) = open(TWO_OKS);
        compile(&mut source, &dsl, None, &[]);
        // DSL ".p ok maþr": insert "ek " before "maþr"; the lemma is still
        // numbered as word 1 (maþr) by the previous compile
        let at = dsl.find("maþr").unwrap();
        let edited = format!("{}ek {}", &dsl[..at], &dsl[at..]);
        let guidance = Guidance {
            base_revision: source.patcher.as_ref().unwrap().revision(),
            changes: vec![TextChange { from_a: at, to_a: at, from_b: at, to_b: at + 3 }],
        };
        let out = compile(&mut source, &edited, Some(guidance), &[(1, "maðr", "xNC")]);
        assert!(out.xml.contains(r#"<w lemma="maðr" me:msa="xNC"><choice><me:facs>maþr"#), "{}", out.xml);
        assert_eq!(out.renumbering, Some(vec![Some(0), Some(2)]));
    }

    #[test]
    fn test_menota_file_with_its_own_lemmas_stays_byte_identical() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/HolmPerg-34-4to-MLL.xml");
        let Ok(xml) = std::fs::read_to_string(path) else { return };
        let (mut source, dsl) = open(&xml);

        // The lemmas the app loads on import: every <w> with lemma and msa
        let body = source.original_body_xml.clone();
        let words: Vec<usize> = body.match_indices("<w").map(|(i, _)| i).filter(|&i| body[i + 2..].starts_with([' ', '>'])).collect();
        let lemmas: Vec<(u32, String, String)> = words
            .iter()
            .enumerate()
            .filter_map(|(n, &at)| {
                let tag = &body[at..at + crate::importer::tei::source_map::tag_length(&body[at..])?];
                let attr = crate::importer::tei::source_map::tag_attribute;
                Some((n as u32, attr(tag, "lemma")?, attr(tag, "me:msa").or_else(|| attr(tag, "msa"))?))
            })
            .collect();
        assert!(lemmas.len() > 40_000, "{}", lemmas.len());
        let lemmas: Vec<(u32, &str, &str)> = lemmas.iter().map(|(n, l, m)| (*n, l.as_str(), m.as_str())).collect();

        let out = compile(&mut source, &dsl, None, &lemmas);
        assert!(out.xml == xml, "the file's own lemmas changed it");

        // With the normalized forms the app reads with them: <me:norm> text,
        // tags removed and whitespace collapsed
        let norms: Vec<Option<String>> = words
            .iter()
            .map(|&at| {
                let word = &body[at..at + body[at..].find("</w>")?];
                let start = word.find("<me:norm>")? + "<me:norm>".len();
                let inner = &word[start..start + word[start..].find("</me:norm>")?];
                let mut text = String::new();
                let mut in_tag = false;
                for c in inner.chars() {
                    match c {
                        '<' => in_tag = true,
                        '>' => in_tag = false,
                        c if !in_tag => text.push(c),
                        _ => {}
                    }
                }
                Some(text.split_whitespace().collect::<Vec<_>>().join(" "))
            })
            .collect();
        let with_norms: Vec<_> = lemmas
            .iter()
            .map(|&(n, lemma, msa)| (n, lemma, msa, norms[n as usize].as_deref()))
            .collect();
        assert!(with_norms.iter().filter(|l| l.3.is_some()).count() > 40_000);
        let out = compile_with_norms(&mut source, &dsl, None, &with_norms);
        assert!(out.xml == xml, "the file's own normalized forms changed it");
    }

    /// Replaces `old` with `new` in the DSL, as one editor change.
    fn replace_word(source: &mut ImportedSource, dsl: &str, old: &str, new: &str, lemmas: &[(u32, &str, &str)]) -> CompileOutput {
        let at = dsl.find(old).unwrap();
        let edited = format!("{}{}{}", &dsl[..at], new, &dsl[at + old.len()..]);
        let units = |s: &str| s.encode_utf16().count();
        let from = units(&dsl[..at]);
        let guidance = Guidance {
            base_revision: source.patcher.as_ref().unwrap().revision(),
            changes: vec![TextChange { from_a: from, to_a: from + units(old), from_b: from, to_b: from + units(new) }],
        };
        compile(source, &edited, Some(guidance), lemmas)
    }

    #[test]
    fn test_another_word_in_place_of_one_is_a_new_word() {
        let (mut source, dsl) = open(TWO_OKS);
        compile(&mut source, &dsl, None, &[(0, "ok", "xCC")]);
        // "ok" (lemma ok) replaced with "geck": nothing of "ok" carries over
        let out = replace_word(&mut source, &dsl, "ok", "geck", &[(0, "ok", "xCC")]);
        assert!(out.xml.contains("<w>\n  <choice>\n    <me:facs>geck</me:facs>"), "{}", out.xml);
        assert!(out.xml.contains("<me:norm>geck</me:norm>"), "{}", out.xml);
        assert!(!out.xml.contains(r#"lemma="ok""#), "{}", out.xml);
        assert!(out.edited_words.is_empty());
        // The old word (and so its lemma) is gone
        assert_eq!(out.renumbering, Some(vec![None, Some(1)]));
    }

    #[test]
    fn test_an_edited_word_keeps_its_lemma() {
        let (mut source, dsl) = open(TWO_OKS);
        compile(&mut source, &dsl, None, &[(0, "ok", "xCC")]);
        let out = replace_word(&mut source, &dsl, "ok", "ek", &[(0, "ok", "xCC")]);
        assert!(out.xml.contains(r#"<w lemma="ok" me:msa="xCC">"#), "{}", out.xml);
        assert_eq!(out.edited_words, vec![0]);
        assert_eq!(out.renumbering, None);
    }

    #[test]
    fn test_removed_lemma_is_removed_from_the_file() {
        let (mut source, dsl) = open(TWO_OKS);
        // An empty lemma: removed by the user
        let out = compile(&mut source, &dsl, None, &[(0, "", ""), (1, "", "")]);
        assert_eq!(out.xml, TWO_OKS.replacen(r#"<w lemma="ok" me:msa="xCC">"#, "<w>", 1));
    }

    #[test]
    fn test_changed_normalized_form_is_written_into_the_file() {
        let (mut source, dsl) = open(TWO_OKS);
        // Word 1 has no lemma: only its normalized form is set
        let out = compile_with_norms(&mut source, &dsl, None, &[(0, "ok", "xCC", Some("ok")), (1, "", "", Some("maðr & co"))]);
        assert_eq!(out.xml, TWO_OKS.replacen("<me:norm>maðr</me:norm>", "<me:norm>maðr &amp; co</me:norm>", 1));
    }

    #[test]
    fn test_normalized_form_keeps_the_line_breaks_in_it() {
        let xml = TWO_OKS.replacen("<me:norm>maðr</me:norm>", r#"<me:norm>ma<lb n="2"/>ðr</me:norm>"#, 1);
        let (mut source, dsl) = open(&xml);
        let out = compile_with_norms(&mut source, &dsl, None, &[(1, "", "", Some("maðr"))]);
        assert_eq!(out.xml, xml, "the same text, around a line break");
        let out = compile_with_norms(&mut source, &dsl, None, &[(1, "", "", Some("mannr"))]);
        assert!(out.xml.contains(r#"<me:norm>mannr<lb n="2"/></me:norm>"#), "{}", out.xml);
    }

    #[test]
    fn test_reordered_analysis_leaves_the_file_alone() {
        let xml = TWO_OKS.replacen(r#"me:msa="xCC""#, r#"me:msa="xNC gM nS""#, 1);
        let (mut source, dsl) = open(&xml);
        let out = compile(&mut source, &dsl, None, &[(0, "ok", "nS xNC gM")]);
        assert_eq!(out.xml, xml);
    }
}
