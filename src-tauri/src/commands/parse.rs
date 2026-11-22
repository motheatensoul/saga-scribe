use crate::annotations::AnnotationSet;
use crate::entities::EntityRegistry;
use crate::identity::{numbers_by_key, renumbering, Guidance, ParkedWord, Parking, Renumbering, ReturnedWord, Tracked};
use crate::importer::tei::patching::{
    apply_decisions, compiler_config_for, render_body, write_lemmas_placed, Originals, Patcher, SavedRecord, WordRecord,
};
use crate::importer::tei::apply::TreeSource;
use crate::importer::tei::segments::{Decision, ImportedDocument};
use crate::normalizer::LevelDictionary;
use crate::parser::{Compiler, CompilerConfig, LemmaMapping, WordKey};
use crate::perf;
use std::collections::{HashMap, HashSet};
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
    abbreviations_json: Option<String>,
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
            levels: None,
        };

        let mut compiler = Compiler::new()
            .with_entities(&registry)
            .with_config(config)
            .with_lemma_mappings(lemma_mappings);

        // Add annotations if available
        if let Some(ref ann) = annotations {
            compiler = compiler.with_annotations(ann);
        }
        let abbreviations = read_abbreviations(abbreviations_json.as_deref())?;
        compiler = compiler.with_abbreviations(&abbreviations);

        // Add dictionary if available
        if let Some(ref dict) = dictionary {
            compiler = compiler.with_dictionary(dict);
        }

        let pending = std::mem::take(&mut *session.1.lock().map_err(|_| "Compile state is poisoned")?);
        let mut state = session.0.lock().map_err(|_| "Compile state is poisoned")?;
        timer.lap("compile.wait_for_session", []);
        let (body, revision, renumbered) =
            compile_tracking_words_with(compiler, &input, guidance.as_ref(), &mut state, pending)?;

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
        Ok(CompileOutput { xml, revision, renumbering: renumbered, parked: Vec::new(), returned: Vec::new(), edited_words: Vec::new(), records: Vec::new() })
    })
    .await
    .map_err(|e| format!("Compilation task failed: {}", e))?
    .map(|output| sent.reply(output, xml_base))
}

/// The last native-mode compile: which token and word is which, so the next
/// compile can tell where words went. Also the words whose XML was set in the
/// word panel waiting for the first compile of a project being opened.
#[derive(Default, Clone)]
pub struct NativeSession(Arc<Mutex<Option<NativeState>>>, Arc<Mutex<Vec<SavedOverride>>>);

pub struct NativeState {
    tracked: Tracked,
    /// The key of every word, in document order
    words: Vec<WordKey>,
    /// Words whose XML was set in the word panel (new documents): the
    /// compiled word is replaced by it, and later edits are merged into it
    overrides: HashMap<WordKey, NativeOverride>,
    /// The last compiled body (with the overrides), and where each `<w>`
    /// in it starts
    body: String,
    word_offsets: Vec<usize>,
}

/// A word of a new document whose XML was set in the word panel: its XML,
/// and the compiled word it was set against (`None` until the next compile).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NativeOverride {
    pub xml: String,
    pub compiled: Option<String>,
}

/// An override as a project saves it (`overrides.json`): by word number.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedOverride {
    pub word: usize,
    #[serde(flatten)]
    pub word_override: NativeOverride,
}

impl NativeSession {
    /// The XML of word `word` (as the previews number them) as the last
    /// compile wrote it.
    pub fn word_xml(&self, word: usize) -> Result<String, String> {
        let state = self.0.lock().map_err(|_| "Compile state is poisoned")?;
        let state = state.as_ref().ok_or("Nothing is compiled yet")?;
        let at = *state.word_offsets.get(word).ok_or("There is no such word")?;
        element_at(&state.body, at).map(str::to_string).ok_or_else(|| "The word's XML can't be read".into())
    }

    /// Sets the XML of word `word` to `xml` (one well-formed `<w>`): the
    /// compile puts it in place of the compiled word from now on, and merges
    /// later edits of the word into it. Returns the word's shorthand, for the
    /// editor to put in place of the word's (the next compile takes the XML).
    pub fn set_word_xml(&self, word: usize, xml: &str) -> Result<WordXmlChange, String> {
        if !xml.trim_start().starts_with("<w") {
            return Err("The XML must be a single <w> element".into());
        }
        let dsl = crate::document::word_dsl(xml, None)?;
        let mut state = self.0.lock().map_err(|_| "Compile state is poisoned")?;
        let state = state.as_mut().ok_or("Nothing is compiled yet")?;
        let key = *state.words.get(word).ok_or("There is no such word")?;
        let j = state.tracked.ids.iter().position(|&id| id == key.0).ok_or("The word isn't in the shorthand")?;
        let utf16 = |offset: usize| state.tracked.dsl.get(..offset).map_or(0, |text| text.encode_utf16().count());
        let (from, to) = (utf16(state.tracked.spans[j].start), utf16(state.tracked.spans[j].end));
        state.overrides.insert(key, NativeOverride { xml: xml.trim().to_string(), compiled: None });
        let tree = crate::document::Tree::parse(&format!(
            r#"<x xmlns="http://www.tei-c.org/ns/1.0" xmlns:me="http://www.menota.org/ns/1.0">{xml}</x>"#
        ))?;
        let element = tree.root_element().and_then(|r| tree.children(r).iter().find_map(|&c| tree.element(c)).cloned());
        let attribute = |name: &str| element.as_ref().and_then(|e| e.attribute(name)).map(str::to_string);
        Ok(WordXmlChange { from, to, dsl, lemma: attribute("lemma"), msa: attribute("me:msa").or_else(|| attribute("msa")) })
    }

    /// The overrides of the last compile, for saving.
    pub fn saved_overrides(&self) -> Vec<SavedOverride> {
        let Ok(state) = self.0.lock() else { return Vec::new() };
        let Some(state) = state.as_ref() else { return Vec::new() };
        let mut saved: Vec<SavedOverride> = state
            .words
            .iter()
            .enumerate()
            .filter_map(|(n, key)| Some(SavedOverride { word: n, word_override: state.overrides.get(key)?.clone() }))
            .collect();
        saved.sort_by_key(|s| s.word);
        saved
    }

    /// Overrides of a project being opened, applied by its first compile.
    pub fn restore(&self, saved: Vec<SavedOverride>) {
        if let Ok(mut pending) = self.1.lock() {
            *pending = saved;
        }
    }
}

/// Where each `<w>` element of `xml` starts (comments skipped).
fn word_starts(xml: &str) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut i = 0;
    while let Some(lt) = xml[i..].find('<').map(|k| k + i) {
        let rest = &xml[lt..];
        if rest.starts_with("<!--") {
            i = lt + rest.find("-->").map_or(rest.len(), |e| e + 3);
            continue;
        }
        if rest.starts_with("<w") && rest[2..].starts_with(|c: char| c == '>' || c == '/' || c.is_whitespace()) {
            starts.push(lt);
        }
        i = lt + 1;
    }
    starts
}

/// Puts the overridden words into `body` (compiled, with `words` as its
/// words' keys): a word compiled as when its XML was set gets that XML; one
/// edited since gets the edit merged into it, with the attributes the
/// compiler changed (a lemma, `ana`). Overrides of words no longer there go.
fn apply_overrides(body: &mut String, words: &[WordKey], overrides: &mut HashMap<WordKey, NativeOverride>) {
    if overrides.is_empty() {
        return;
    }
    let offsets = word_starts(body);
    let index: HashMap<WordKey, usize> = words.iter().enumerate().map(|(n, k)| (*k, n)).collect();
    overrides.retain(|key, _| index.contains_key(key));
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for (key, word) in overrides.iter_mut() {
        let Some(&at) = offsets.get(index[key]) else { continue };
        let Some(compiled) = element_at(body, at) else { continue };
        let xml = match &word.compiled {
            Some(previous) if previous != compiled => {
                let merged = crate::document::merge_word_edit(&word.xml, previous, compiled).map_or_else(|| compiled.to_string(), |m| m.xml);
                crate::document::with_compiled_attributes(&merged, previous, compiled).unwrap_or(merged)
            }
            _ => word.xml.clone(),
        };
        edits.push((at, compiled.len(), xml.clone()));
        word.compiled = Some(compiled.to_string());
        word.xml = xml;
    }
    edits.sort_unstable_by_key(|(at, _, _)| std::cmp::Reverse(*at));
    for (at, len, xml) in edits {
        body.replace_range(at..at + len, &xml);
    }
}

/// Compiles `input`, keeping track of which word is which (see
/// [`crate::identity`]): the compiler's annotations and lemma mappings are
/// numbered by the previous compile in `state` and follow their words.
/// Without `guidance` the document is taken to be new: no previous compile.
/// Returns the body, its revision and the renumbering of the previous words.
#[cfg(test)]
pub(crate) fn compile_tracking_words(
    compiler: Compiler,
    input: &str,
    guidance: Option<&Guidance>,
    state: &mut Option<NativeState>,
) -> Result<(String, u64, Option<Renumbering>), String> {
    compile_tracking_words_with(compiler, input, guidance, state, Vec::new())
}

/// [`compile_tracking_words`], with the overrides of a project being opened
/// (`pending`, by word number) taken on by this compile.
pub(crate) fn compile_tracking_words_with(
    compiler: Compiler,
    input: &str,
    guidance: Option<&Guidance>,
    state: &mut Option<NativeState>,
    pending: Vec<SavedOverride>,
) -> Result<(String, u64, Option<Renumbering>), String> {
    let mut timer = perf::timer();
    let previous = state.as_ref().filter(|_| guidance.is_some());
    let tracked = Tracked::track(previous.map(|p| &p.tracked), guidance, input, 0)?;
    let previous_numbers = previous.map(|p| numbers_by_key(&p.words));
    let mut compiler = compiler.with_word_identity(tracked.ids.clone(), previous_numbers);
    timer.lap("compile.track_words", []);

    let mut body = compiler.compile(input)?;
    let words = compiler.word_keys().to_vec();
    timer.lap("compile.compile", [("words", words.len().into())]);
    let renumbered = previous.and_then(|p| renumbering(&p.words, &words));
    // Words whose XML was set: kept through guided compiles, taken from the
    // project when it is opened
    let mut overrides = match state.take().filter(|_| guidance.is_some()) {
        Some(previous) => previous.overrides,
        None => HashMap::new(),
    };
    for saved in pending {
        if let Some(key) = words.get(saved.word) {
            overrides.insert(*key, saved.word_override);
        }
    }
    apply_overrides(&mut body, &words, &mut overrides);
    let word_offsets = word_starts(&body);
    timer.done("compile.renumber", []);
    let revision = tracked.revision;
    *state = Some(NativeState { tracked, words, overrides, body: body.clone(), word_offsets });
    Ok((body, revision, renumbered))
}

/// The imported document being edited, held in the backend.
///
/// The segment manifest of a large manuscript is tens of megabytes of JSON:
/// it never crosses to the frontend. Importing a file or opening a project
/// reads the document here and stages it ([`ImportedSession::stage`]); the
/// editor then makes it the current one with [`set_imported_document`], so
/// it switches documents when it is ready to, and saving writes it from here.
#[derive(Default, Clone)]
pub struct ImportedSession(Arc<Mutex<ImportedSlots>>);

#[derive(Default)]
pub struct ImportedSlots {
    /// The document compiles go to
    current: Option<ImportedSource>,
    /// Read, not yet loaded by the editor: its handle and the document
    staged: Option<(u64, ImportedSource)>,
    next_handle: u64,
}

/// What the editor needs of a staged document.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StagedDocument {
    pub handle: u64,
    pub is_menota: bool,
    /// The abbreviation store its shorthand is written with
    pub abbreviations: crate::importer::tei::abbreviations::Store,
    /// How the file encodes its transcription, for the editor to confirm
    pub profile: Option<crate::document::profile::Profile>,
    pub dsl_rules: u32,
    pub decisions: HashMap<usize, Decision>,
}

impl ImportedSession {
    /// Keeps `source` until the editor loads it with [`set_imported_document`]
    /// (replacing one staged before and not loaded).
    pub fn stage(&self, source: ImportedSource) -> Result<StagedDocument, String> {
        let mut slots = self.0.lock().map_err(|_| "Imported document state is poisoned")?;
        slots.next_handle += 1;
        let handle = slots.next_handle;
        let staged = StagedDocument {
            handle,
            is_menota: source.document.is_menota,
            abbreviations: source.document.abbreviations.clone(),
            profile: source.document.profile.clone(),
            dsl_rules: source.document.dsl_rules,
            decisions: source.document.decisions.clone(),
        };
        slots.staged = Some((handle, source));
        Ok(staged)
    }

    /// Makes the staged document `handle` the current one, or with none,
    /// clears it (see [`set_imported_document`]).
    ///
    /// # Errors
    ///
    /// If no document with that handle is staged (another was staged since).
    pub fn load(&self, handle: Option<u64>) -> Result<(), String> {
        let mut slots = self.0.lock().map_err(|_| "Imported document state is poisoned")?;
        slots.current = match handle {
            None => None,
            Some(handle) => match slots.staged.take() {
                Some((staged, source)) if staged == handle => Some(source),
                other => {
                    slots.staged = other;
                    return Err("The imported document is no longer available; please open it again".into());
                }
            },
        };
        Ok(())
    }

    /// Runs `f` on the current document, to change it, if there is one.
    #[cfg(test)]
    pub(crate) fn with_current_mut<T>(&self, f: impl FnOnce(&mut ImportedSource) -> T) -> Result<Option<T>, String> {
        let mut slots = self.0.lock().map_err(|_| "Imported document state is poisoned")?;
        Ok(slots.current.as_mut().map(f))
    }

    /// Sets the levels of the current document's profile (see
    /// [`set_imported_levels`]); returns the profile.
    pub fn set_levels(&self, levels: Vec<crate::document::profile::Level>, choice: bool) -> Result<crate::document::profile::Profile, String> {
        use crate::document::profile::{Level, Profile};
        let mut slots = self.0.lock().map_err(|_| "Imported document state is poisoned")?;
        let source = slots.current.as_mut().ok_or("No imported document is loaded")?;
        let mut profile = source.document.profile.clone().unwrap_or_else(Profile::menota);
        profile.levels = Level::ALL.into_iter().filter(|l| levels.contains(l)).collect();
        profile.choice = choice && profile.levels.len() > 1;
        source.document.profile = Some(profile.clone());
        source.config = compiler_config_for(&source.document, Originals::new(source.tree.as_ref()));
        Ok(profile)
    }

    /// Runs `f` on the current document, if there is one.
    pub fn with_current<T>(&self, f: impl FnOnce(&ImportedSource) -> T) -> Result<Option<T>, String> {
        let slots = self.0.lock().map_err(|_| "Imported document state is poisoned")?;
        Ok(slots.current.as_ref().map(f))
    }
}

/// Everything needed to compile edited DSL back into the original file.
pub struct ImportedSource {
    /// The segments, without the copies of the file's XML they were saved
    /// with where the tree has it (see [`Originals`])
    document: ImportedDocument,
    original_body_xml: String,
    preamble: String,
    postamble: String,
    /// Which word is which across edits
    patcher: Patcher,
    /// The file as a tree, edits are applied to; `None` if the file couldn't
    /// be mapped onto its segments (edits then rebuild the body from them)
    tree: Option<TreeSource>,
    /// Compiler settings matching the file's encoding
    config: CompilerConfig,
    /// The key of every word of the last compile, in document order
    words: Option<Vec<WordKey>>,
    /// Words gone in a compile, which may come back (moved)
    parking: Parking,
    /// Records still open when the project was saved, for words of the
    /// file as saved: (segment ID, original token, record)
    saved: Vec<(usize, u64, SavedRecord)>,
    /// The last compile's records, for saving
    last_records: Vec<WordRecord>,
    /// The last compile's body, and where each of its `<w>` starts: the
    /// words as the word panel shows them
    last_body: String,
    last_word_offsets: Vec<usize>,
}

/// A word whose XML was set in the word panel: its shorthand now, to put in
/// the editor at `from..to` (UTF-16), and its lemma and analysis.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WordXmlChange {
    pub from: usize,
    pub to: usize,
    pub dsl: String,
    pub lemma: Option<String>,
    pub msa: Option<String>,
}

/// Writes what the annotations of `set` (numbered by the previous compile's
/// `previous` words, or by `words` without one) add to words into `body`
/// (whose words are `words`): attributes, notes, `<c>` for characters.
fn write_annotations(
    body: &mut crate::importer::tei::patching::RenderedBody,
    set: &AnnotationSet,
    words: &[WordKey],
    previous: Option<&Vec<WordKey>>,
    config: &CompilerConfig,
) {
    use crate::annotations::{AnnotationTarget, AnnotationType};
    let annotated: HashSet<u32> = set
        .annotations
        .iter()
        .filter(|a| a.annotation_type != AnnotationType::Lemma)
        .filter_map(|a| match &a.target {
            AnnotationTarget::Word { word_index } | AnnotationTarget::Character { word_index, .. } => Some(*word_index),
            AnnotationTarget::Span { .. } => None,
        })
        .collect();
    if annotated.is_empty() {
        return;
    }
    let compiler = Compiler::new().with_config(config.clone()).with_annotations(set);
    let previous_numbers = previous.map(|p| numbers_by_key(p));
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    for (n, key) in words.iter().enumerate() {
        let number = match &previous_numbers {
            Some(numbers) => match numbers.get(key) {
                Some(&p) => p,
                None => continue,
            },
            None => n as u32,
        };
        if !annotated.contains(&number) {
            continue;
        }
        let Some(&(at, _)) = body.words.get(n) else { continue };
        let Some(xml) = element_at(&body.xml, at) else { continue };
        let (attributes, notes) = compiler.annotation_markup(number);
        let chars = |text: &str| compiler.with_character_tags(text, number);
        if let Some(marked) = crate::document::with_annotation_markup(xml, &attributes, &notes, &chars) {
            if marked != xml {
                replacements.push((at, xml.len(), marked));
            }
        }
    }
    // Back to front; then the offsets after each replacement move by its change
    replacements.sort_unstable_by_key(|(at, _, _)| *at);
    for (at, len, xml) in replacements.iter().rev() {
        body.xml.replace_range(*at..*at + *len, xml);
    }
    // What the replacements up to each one add, for looking up
    let mut added = Vec::with_capacity(replacements.len());
    let mut total = 0isize;
    for (_, len, xml) in &replacements {
        total += xml.len() as isize - *len as isize;
        added.push(total);
    }
    let shift = |offset: usize| -> usize {
        let before = replacements.partition_point(|(at, _, _)| *at < offset);
        let moved = before.checked_sub(1).map_or(0, |i| added[i]);
        (offset as isize + moved) as usize
    };
    for (offset, _) in body.words.iter_mut() {
        *offset = shift(*offset);
    }
    for offset in body.edited_words.iter_mut() {
        *offset = shift(*offset);
    }
}

/// The abbreviation store's defaults the editor sends (mark → expansion).
fn read_abbreviations(json: Option<&str>) -> Result<HashMap<String, String>, String> {
    match json {
        Some(json) => serde_json::from_str(json).map_err(|e| format!("Failed to parse the abbreviation store: {e}")),
        None => Ok(HashMap::new()),
    }
}

/// The element that starts at `at` in `xml` (a `<w>`), as written.
fn element_at(xml: &str, at: usize) -> Option<&str> {
    let rest = xml.get(at..)?;
    let name_len = rest[1..].find(|c: char| c.is_whitespace() || c == '>' || c == '/')? ;
    let name = &rest[1..1 + name_len];
    let (open, close) = (format!("<{name}"), format!("</{name}>"));
    let mut depth = 0usize;
    let mut i = 0;
    while i < rest.len() {
        let here = &rest[i..];
        if here.starts_with(&close) {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(&rest[..i + close.len()]);
            }
            i += close.len();
        } else if here.starts_with(&open) && here[open.len()..].starts_with(|c: char| c.is_whitespace() || c == '>' || c == '/') {
            let tag_end = here.find('>')?;
            if here[..tag_end].ends_with('/') {
                if depth == 0 {
                    return Some(&rest[..i + tag_end + 1]);
                }
            } else {
                depth += 1;
            }
            i += tag_end + 1;
        } else {
            i += here.chars().next().map_or(1, char::len_utf8);
        }
    }
    None
}

impl ImportedSource {
    /// Compiles `dsl` without guidance or answers, as the first compile
    /// after loading does (for tests that go through saving).
    #[cfg(test)]
    pub(crate) fn compile_plainly(&mut self, dsl: &str) -> Result<CompileOutput, String> {
        let mut compiler = Compiler::new().with_config(self.config.clone());
        compile_imported_source(self, &mut compiler, dsl, None, &HashMap::new(), HashMap::new(), &[], None)
    }

    /// The XML of word `word` (as the previews number them) as the last
    /// compile wrote it.
    pub fn word_xml(&self, word: usize) -> Result<String, String> {
        let at = *self.last_word_offsets.get(word).ok_or("There is no such word")?;
        element_at(&self.last_body, at).map(str::to_string).ok_or_else(|| "The word's XML can't be read".into())
    }

    /// Sets the XML of word `word` (as the previews number them) to `xml`:
    /// it becomes the word as the file has it, and the shorthand reads as it
    /// (the editor puts the returned shorthand in place of the word's).
    /// The word must be in the file (not added in the shorthand) and `xml`
    /// one well-formed word of the same element.
    pub fn set_word_xml(&mut self, word: usize, xml: &str) -> Result<WordXmlChange, String> {
        let &(token, k) = self.words.as_ref().and_then(|w| w.get(word)).ok_or("There is no such word")?;
        let not_in_file = "This word was added in the shorthand; its XML can be set once the project is saved and opened again";
        let segment = usize::try_from(token)
            .ok()
            .filter(|_| k == 0)
            .and_then(|t| self.patcher.segment_of_token(t))
            .ok_or(not_in_file)?;
        let line_edition = self.document.profile.as_ref().and_then(|p| p.line_edition.clone());
        let dsl = crate::document::word_dsl(xml, line_edition.as_deref())?;
        let tree = self.tree.as_mut().ok_or("The file's XML can't be edited word by word")?;
        tree.replace_word(segment, xml)?;
        if let Some(crate::importer::tei::segments::Segment::Word { dsl_content, original_xml, attributes, .. }) = self.document.segments.get_mut(segment) {
            dsl_content.clone_from(&dsl);
            original_xml.clear();
            attributes.clear();
        }
        self.patcher.set_original(segment, &dsl, &self.document.segments, self.document.dsl_rules);
        let (from, to) = self.patcher.span_of_token(token).ok_or("The word isn't in the shorthand")?;
        let element = tree.segment_element(segment);
        let attribute = |name: &str| element.and_then(|e| e.attribute(name)).map(str::to_string);
        Ok(WordXmlChange { from, to, dsl, lemma: attribute("lemma"), msa: attribute("me:msa").or_else(|| attribute("msa")) })
    }

    /// The profile, for saving (`profile.json`).
    pub fn profile(&self) -> Option<&crate::document::profile::Profile> {
        self.document.profile.as_ref()
    }

    /// The last compile's records still open (no answer in `decisions`),
    /// as saved with `output`, the file that compile wrote: each by its
    /// word's place and fingerprint there. Records without a word (not a
    /// `<w>`) aren't saved.
    pub fn open_records(&self, output: &str, decisions: &HashMap<usize, Decision>) -> Result<Vec<SavedRecord>, String> {
        use crate::document::anchor::{body_words, fingerprint};
        let open: Vec<&WordRecord> = self.last_records.iter().filter(|r| !decisions.contains_key(&r.key)).collect();
        if open.is_empty() {
            return Ok(Vec::new());
        }
        let tree = crate::document::Tree::parse(output)?;
        let words = body_words(&tree);
        Ok(open
            .into_iter()
            .filter_map(|r| {
                let n = r.word?;
                Some(SavedRecord {
                    word: n,
                    fingerprint: fingerprint(&tree, *words.get(n)?),
                    suggestion: r.suggestion,
                    was: r.was.clone(),
                    xml: r.xml.clone(),
                    lemma: r.lemma.clone(),
                    msa: r.msa.clone(),
                    stale_levels: r.stale_levels.clone(),
                })
            })
            .collect())
    }

    /// Takes the records saved with the file this session was opened from
    /// (`.teis` 2.0), finding each word by its place and fingerprint.
    /// Returns how many words weren't found (the file was changed outside
    /// the app); their records are dropped.
    pub fn restore_records(&mut self, saved: Vec<SavedRecord>) -> usize {
        use crate::document::anchor::{body_words, find_word};
        let Some(tree) = &self.tree else { return saved.len() };
        let words = body_words(tree.tree());
        let mut lost = 0;
        for record in saved {
            let found = find_word(tree.tree(), &words, record.word, &record.fingerprint)
                .and_then(|n| tree.segment_of(words[n]))
                .and_then(|segment| Some((segment, self.patcher.token_of(segment)? as u64)));
            match found {
                Some((segment, token)) => self.saved.push((segment, token, record)),
                None => lost += 1,
            }
        }
        lost
    }


    /// Reads the file into its tree and prepares the patcher, so the first
    /// edit doesn't wait for either (a large manuscript takes a second).
    pub fn new(document: ImportedDocument, original_body_xml: String, preamble: String, postamble: String) -> Self {
        Self::with_tree(document, None, original_body_xml, preamble, postamble)
    }

    /// [`new`](Self::new), with the file already parsed into `tree` if it was.
    pub fn with_tree(
        mut document: ImportedDocument,
        tree: Option<crate::document::Tree>,
        original_body_xml: String,
        preamble: String,
        postamble: String,
    ) -> Self {
        let timer = perf::timer();
        let tree = document.spans.as_deref().and_then(|spans| {
            match tree {
                Some(tree) => TreeSource::on(tree, preamble.len(), postamble.len(), &document.segments, spans),
                None => TreeSource::new(&preamble, &original_body_xml, &postamble, &document.segments, spans),
            }
                .inspect_err(|e| {
                    log::warn!("The imported file couldn't be mapped onto its segments ({e}); edits rebuild its body")
                })
                .ok()
        });
        let config = compiler_config_for(&document, Originals::new(tree.as_ref()));
        let patcher = Patcher::new(&document.segments, document.dsl_rules);
        // The tree has the file's XML; the segments' copies of it go
        if let Some(tree) = &tree {
            document.drop_copies(|id| tree.segment_xml(id), |id| tree.segment_attributes(id));
        }
        timer.done("imported.load", [("tree", tree.is_some().into()), ("segments", document.segments.len().into())]);
        Self { document, original_body_xml, preamble, postamble, patcher, tree, config, words: None, parking: Parking::default(), saved: Vec::new(), last_records: Vec::new(), last_body: String::new(), last_word_offsets: Vec::new() }
    }
}

/// Makes the staged document `handle` (see [`ImportedSession::stage`]) the
/// one [`compile_imported`] compiles into, or with none, clears it.
///
/// # Errors
///
/// If no document with that handle is staged (another was staged since).
#[tauri::command(async, rename_all = "camelCase")]
pub async fn set_imported_document(session: State<'_, ImportedSession>, handle: Option<u64>) -> Result<(), String> {
    session.load(handle)
}

/// Sets which levels new words of the current imported document are
/// written with, and whether in a `<choice>`, as the editor confirmed them
/// after importing. The profile is saved with the project.
///
/// # Errors
///
/// If no imported document is loaded.
#[tauri::command(async, rename_all = "camelCase")]
pub async fn set_imported_levels(
    session: State<'_, ImportedSession>,
    levels: Vec<crate::document::profile::Level>,
    choice: bool,
) -> Result<crate::document::profile::Profile, String> {
    session.set_levels(levels, choice)
}

/// The XML of word `word` (numbered as the previews count) of the current
/// imported document, as the last compile wrote it (the word panel's XML tab).
///
/// # Errors
///
/// If no imported document is loaded or there is no such word.
#[tauri::command(async)]
pub async fn imported_word_xml(session: State<'_, ImportedSession>, word: usize) -> Result<String, String> {
    session.with_current(|source| source.word_xml(word))?.ok_or("No imported document is loaded")?
}

/// Sets the XML of word `word` of the current imported document (the word
/// panel's XML tab); see [`ImportedSource::set_word_xml`]. The editor then
/// puts the returned shorthand in place of the word's and compiles.
///
/// # Errors
///
/// If the XML isn't one well-formed word, or the word isn't in the file.
#[tauri::command(async)]
pub async fn set_imported_word_xml(session: State<'_, ImportedSession>, word: usize, xml: String) -> Result<WordXmlChange, String> {
    let mut slots = session.0.lock().map_err(|_| "Imported document state is poisoned")?;
    let source = slots.current.as_mut().ok_or("No imported document is loaded")?;
    source.set_word_xml(word, &xml)
}

/// The XML of word `word` of the new document, as the last compile wrote it
/// (the word panel's XML tab).
///
/// # Errors
///
/// If nothing was compiled yet or there is no such word.
#[tauri::command(async)]
pub async fn native_word_xml(session: State<'_, NativeSession>, word: usize) -> Result<String, String> {
    session.word_xml(word)
}

/// Sets the XML of word `word` of the new document (the word panel's XML
/// tab); see [`NativeSession::set_word_xml`].
///
/// # Errors
///
/// If the XML isn't one well-formed `<w>`, or there is no such word.
#[tauri::command(async)]
pub async fn set_native_word_xml(session: State<'_, NativeSession>, word: usize, xml: String) -> Result<WordXmlChange, String> {
    session.set_word_xml(word, &xml)
}

/// The normalized form the shorthand gives word `word` (numbered as the
/// previews count) of the current imported document, as its edit would
/// derive it: what the word panel resets a curated normalized form to.
/// `None` for a word without a normalized level.
///
/// # Errors
///
/// If no imported document is loaded, the word isn't one, or the entity and
/// normalizer data don't parse.
#[tauri::command(async, rename_all = "camelCase")]
#[allow(clippy::too_many_arguments)]
pub async fn imported_derived_normalized(
    session: State<'_, ImportedSession>,
    word: usize,
    entities_json: Option<String>,
    normalizer_json: Option<String>,
    entity_mappings_json: Option<String>,
    custom_mappings: Option<HashMap<String, String>>,
    abbreviations_json: Option<String>,
) -> Result<Option<String>, String> {
    let session = session.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut registry = EntityRegistry::new();
        if let Some(json) = entities_json {
            registry.load_from_str(&json)?;
        }
        let mut dictionary = match normalizer_json {
            Some(json) => Some(LevelDictionary::load(&json)?),
            None => None,
        };
        if let Some(ref mut dict) = dictionary {
            if let Some(ref json) = entity_mappings_json {
                dict.load_entity_mappings(json)?;
            }
            if let Some(custom) = custom_mappings {
                dict.add_entity_mappings(custom);
            }
        }
        let abbreviations = read_abbreviations(abbreviations_json.as_deref())?;
        session
            .with_current(|source| -> Result<Option<String>, String> {
                let &(token, _) = source.words.as_ref().and_then(|w| w.get(word)).ok_or("There is no such word")?;
                let dsl = source.patcher.token_text(token).ok_or("The word isn't in the shorthand")?.to_string();
                let mut compiler = Compiler::new().with_entities(&registry).with_config(source.config.clone()).with_abbreviations(&abbreviations);
                if let Some(ref dict) = dictionary {
                    compiler = compiler.with_dictionary(dict);
                }
                Ok(crate::importer::tei::patching::derived_normalized(&mut compiler, &dsl))
            })?
            .ok_or("No imported document is loaded")?
    })
    .await
    .map_err(|e| format!("Deriving the normalized form failed: {e}"))?
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
    decisions: Option<HashMap<usize, Decision>>,
    annotated_words: Option<Vec<u32>>,
    annotations_json: Option<String>,
    abbreviations_json: Option<String>,
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
        let source = guard.current.as_mut().ok_or("No imported document is loaded")?;
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
        let config = source.config.clone();

        let mut compiler = Compiler::new()
            .with_entities(&registry)
            .with_config(config);

        if let Some(ref dict) = dictionary {
            compiler = compiler.with_dictionary(dict);
        }
        let abbreviations = read_abbreviations(abbreviations_json.as_deref())?;
        compiler = compiler.with_abbreviations(&abbreviations);
        // A file whose abbreviations are TEI's <choice><abbr><expan>
        let choice = source.document.profile.as_ref().is_some_and(|p| {
            p.abbreviations == crate::document::profile::Abbreviations::ChoiceAbbrExpan
        });
        compiler = compiler.with_choice_abbreviations(choice);
        // A file whose punctuation is in <me:punct>
        let me_punct = source.document.profile.as_ref().is_some_and(|p| {
            p.punctuation == crate::document::profile::Punctuation::MePunct
        });
        compiler = compiler.with_menota_punctuation(me_punct);

        let decisions = decisions.unwrap_or_default();
        let annotated = annotated_words.unwrap_or_default();
        // Notes and other annotations, written into their words
        let annotations: Option<AnnotationSet> = match annotations_json {
            Some(json) => Some(serde_json::from_str(&json).map_err(|e| format!("Failed to parse annotations: {e}"))?),
            None => None,
        };
        let output =
            compile_imported_source(source, &mut compiler, &edited_dsl, guidance.as_ref(), &decisions, lemmas, &annotated, annotations.as_ref())?;
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
/// previous compile) into the words. `annotated` are the words (numbered
/// likewise) with annotations other than lemmas, which replaced words report.
#[allow(clippy::too_many_arguments)]
pub(crate) fn compile_imported_source(
    source: &mut ImportedSource,
    compiler: &mut Compiler,
    edited_dsl: &str,
    guidance: Option<&Guidance>,
    decisions: &HashMap<usize, Decision>,
    lemmas: HashMap<u32, LemmaMapping>,
    annotated: &[u32],
    annotations: Option<&AnnotationSet>,
) -> Result<CompileOutput, String> {
    // Compute patches between original segments and edited DSL, using the
    // editor's changes to keep track of which word is which
    let mut timer = perf::timer();
    let (mut patches, state) = source.patcher.compute(edited_dsl, guidance)?;
    timer.lap("compile.compute_patches", [("patches", patches.len().into())]);

    // Replaced and deleted words, with the patcher's answers as suggestions;
    // the editor's answers win
    let originals = Originals::new(source.tree.as_ref());
    let mut records = source.patcher.records(
        &source.document.segments,
        originals,
        &patches,
        &state,
        decisions,
        !annotated.is_empty(),
        None,
    );
    apply_decisions(&source.document.segments, originals, &mut patches, decisions);
    timer.lap("compile.records", [("records", records.len().into())]);

    // Apply patches to the document's tree
    let body = render_body(
        &source.document,
        // A word's XML set in the word panel: the body is no longer the file's
        (!source.tree.as_ref().is_some_and(TreeSource::is_changed)).then_some(source.original_body_xml.as_str()),
        source.tree.as_ref(),
        &patches,
        compiler,
    )?;
    timer.lap("compile.render_body", []);

    // Which word is which: lemmas follow their words (no guidance: the
    // document was just loaded, numbered as it is now)
    let words = source.patcher.word_keys(&source.document, &body, &state);
    timer.lap("compile.word_keys", [("words", words.len().into())]);
    let previous = source.words.as_ref().filter(|_| guidance.is_some());
    let renumbered = previous.and_then(|previous| renumbering(previous, &words));
    let (parked, returned) = match previous {
        Some(previous) => source.parking.update(previous, &words, renumbered.as_ref()),
        None => Default::default(),
    };
    let mut lemmas = match previous {
        Some(previous) => {
            let now = numbers_by_key(&words);
            lemmas
                .into_iter()
                .filter_map(|(n, lemma)| Some((*now.get(previous.get(n as usize)?)?, lemma)))
                .collect()
        }
        None => lemmas,
    };
    // A different word doesn't get the old word's lemma back: the editor
    // still holds it for this word number (read from the file), and only
    // another lemma is the editor's choice for the new word
    // Edited words with a level that couldn't follow the edit are asked
    // about too (the normalized form may be out of date)
    if !body.stale.is_empty() {
        let stale: HashMap<usize, Vec<String>> = body.stale.iter().cloned().collect();
        let missing: HashSet<usize> = stale.keys().filter(|k| !records.iter().any(|r| r.key == **k)).copied().collect();
        if !missing.is_empty() {
            let more = source.patcher.records(
                &source.document.segments,
                originals,
                &patches,
                &state,
                decisions,
                true,
                Some(&missing),
            );
            records.extend(more);
            records.sort_by_key(|r| r.from);
        }
        for record in &mut records {
            if let Some(levels) = stale.get(&record.key) {
                record.stale_levels = levels.clone();
            }
        }
    }
    // Records still open when the project was saved: their words are the
    // file's (as saved) now
    for (key, token, saved) in &source.saved {
        if let Some(record) = records.iter_mut().find(|r| r.key == *key) {
            record.was.clone_from(&saved.was);
            record.xml.clone_from(&saved.xml);
            record.lemma.clone_from(&saved.lemma);
            record.msa.clone_from(&saved.msa);
            for level in &saved.stale_levels {
                if !record.stale_levels.contains(level) {
                    record.stale_levels.push(level.clone());
                }
            }
            continue;
        }
        // A deleted word isn't asked about
        let Some(j) = state.ids.iter().position(|id| id == token) else { continue };
        let utf16 = |offset: usize| state.dsl.get(..offset).map_or(0, |text| text.encode_utf16().count());
        records.push(WordRecord {
            key: *key,
            suggestion: saved.suggestion,
            decision: decisions.get(key).copied(),
            word: None,
            token: Some(*token),
            from: utf16(state.spans[j].start),
            to: utf16(state.spans[j].end),
            was: saved.was.clone(),
            xml: saved.xml.clone(),
            lemma: saved.lemma.clone(),
            msa: saved.msa.clone(),
            annotated: false,
            stale_levels: saved.stale_levels.clone(),
        });
    }
    if !source.saved.is_empty() {
        records.sort_by_key(|r| r.from);
    }
    WordRecord::number(&mut records, &words);
    if !annotated.is_empty() {
        let annotated: HashSet<usize> = annotated
            .iter()
            .filter_map(|&n| match &renumbered {
                Some(map) => map.get(n as usize).copied().flatten(),
                None => Some(n),
            })
            .map(|n| n as usize)
            .collect();
        for record in &mut records {
            record.annotated = record.word.is_some_and(|n| annotated.contains(&n));
        }
        records.retain(|r| r.lemma.is_some() || r.msa.is_some() || r.annotated || !r.stale_levels.is_empty());
    }
    for record in records.iter().filter(|r| r.outcome() == Decision::Different) {
        let Some(n) = record.word else { continue };
        let old = |lemma: &LemmaMapping| {
            lemma.lemma == record.lemma.as_deref().unwrap_or("") && lemma.msa == record.msa.as_deref().unwrap_or("")
        };
        if lemmas.get(&(n as u32)).is_some_and(old) {
            lemmas.remove(&(n as u32));
        }
    }
    timer.lap("compile.renumber", []);
    // Annotations (other than lemmas) into their words, as a new document
    // has them: the exported file is the edition
    let mut body = body;
    if let Some(set) = annotations {
        write_annotations(&mut body, set, &words, previous, &source.config);
        timer.lap("compile.write_annotations", [("annotations", set.annotations.len().into())]);
    }
    let (body_xml, word_offsets) = write_lemmas_placed(&body, &lemmas, source.document.is_menota);
    timer.lap("compile.write_lemmas", [("lemmas", lemmas.len().into())]);

    // Only a successful compile becomes the basis for the next one
    source.patcher.commit(state);
    source.words = Some(words);
    source.last_records.clone_from(&records);
    source.last_body.clone_from(&body_xml);
    source.last_word_offsets = word_offsets;

    // Combine with preamble (everything before body) and postamble (everything after)
    Ok(CompileOutput {
        revision: source.patcher.revision(),
        renumbering: renumbered,
        parked,
        returned,
        edited_words: body.edited_word_indices(),
        records,
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
    /// Words of the previous compile that are gone: what the editor stores
    /// by word number for them is kept aside under their ticket
    pub parked: Vec<ParkedWord>,
    /// Words parked before that are back (moved), with their numbers now
    pub returned: Vec<ReturnedWord>,
    /// Words (by number, in document order) edited in the DSL whose lemma, msa
    /// and normalized form were kept from the original and should be reviewed
    pub edited_words: Vec<u32>,
    /// Words of the original replaced or deleted in the DSL (imported documents)
    pub records: Vec<WordRecord>,
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub parked: Vec<ParkedWord>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub returned: Vec<ReturnedWord>,
    pub edited_words: Vec<u32>,
    pub records: Vec<WordRecord>,
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
        let CompileOutput { xml, revision, renumbering, parked, returned, edited_words, records } = output;
        let reply = CompileReply {
            xml: if delta.is_some() { None } else { Some(xml.clone()) },
            delta,
            revision,
            renumbering,
            parked,
            returned,
            edited_words,
            records,
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
        let output = |xml: &str, revision| CompileOutput { xml: xml.into(), revision, renumbering: None, parked: vec![], returned: vec![], edited_words: vec![], records: vec![] };
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
        let mut compiler = Compiler::new().with_config(source.config.clone());
        let lemmas = lemmas
            .iter()
            .map(|(n, lemma, msa, norm)| {
                let mapping = LemmaMapping { lemma: lemma.to_string(), msa: msa.to_string(), normalized: norm.map(str::to_string) };
                (*n, mapping)
            })
            .collect();
        compile_imported_source(source, &mut compiler, dsl, guidance.as_ref(), &HashMap::new(), lemmas, &[], None).unwrap()
    }

    /// Compiles `dsl` as an edit of `from` (guided), with `decisions`.
    fn compile_decided(source: &mut ImportedSource, from: &str, dsl: &str, decisions: &[(usize, Decision)]) -> CompileOutput {
        compile_guided(source, from, dsl, decisions, &[])
    }

    /// Compiles `dsl` as an edit of `from` (guided), with `decisions` and the
    /// `annotated` words (numbered by the previous compile).
    fn compile_guided(
        source: &mut ImportedSource,
        from: &str,
        dsl: &str,
        decisions: &[(usize, Decision)],
        annotated: &[u32],
    ) -> CompileOutput {
        let prefix = from.chars().zip(dsl.chars()).take_while(|(a, b)| a == b).count();
        let suffix = from[prefix..].chars().rev().zip(dsl[prefix..].chars().rev()).take_while(|(a, b)| a == b).count();
        let utf16 = |s: &str, chars: usize| s.chars().take(chars).map(char::len_utf16).sum::<usize>();
        let (from_len, dsl_len) = (from.chars().count(), dsl.chars().count());
        let change = crate::identity::TextChange {
            from_a: utf16(from, prefix),
            to_a: utf16(from, from_len - suffix),
            from_b: utf16(dsl, prefix),
            to_b: utf16(dsl, dsl_len - suffix),
        };
        let guidance = Guidance { base_revision: source.patcher.revision(), changes: vec![change] };
        let mut compiler = Compiler::new().with_config(source.config.clone());
        let decisions: HashMap<usize, Decision> = decisions.iter().copied().collect();
        compile_imported_source(source, &mut compiler, dsl, Some(&guidance), &decisions, HashMap::new(), annotated, None).unwrap()
    }

    #[test]
    fn test_a_word_cut_and_pasted_in_two_compiles_is_parked_and_comes_back() {
        let xml = r#"<TEI xmlns:me="http://www.menota.org/ns/1.0"><text><body><p><w lemma="á"><me:dipl>a</me:dipl></w> <w lemma="bé"><me:dipl>b<foreign rend="red">e</foreign></me:dipl></w> <w lemma="sé"><me:dipl>c</me:dipl></w></p></body></text></TEI>"#;
        let (mut source, dsl) = open(xml);
        assert_eq!(dsl, ".p a be c");
        compile(&mut source, &dsl, None, &[]);
        let cut = compile_guided(&mut source, &dsl, ".p a c", &[], &[]);
        assert_eq!(cut.renumbering, Some(vec![Some(0), None, Some(1)]));
        assert_eq!(cut.parked.len(), 1);
        assert_eq!(cut.parked[0].word, 1);
        let pasted = compile_guided(&mut source, ".p a c", ".p a c be", &[], &[]);
        assert_eq!(pasted.returned, vec![ReturnedWord { ticket: cut.parked[0].ticket.clone(), word: 2 }]);
        assert!(pasted.xml.contains(r#"<w lemma="bé"><me:dipl>b<foreign rend="red">e</foreign></me:dipl></w></p>"#), "{}", pasted.xml);
        // Back is back: nothing is parked twice
        let again = compile_guided(&mut source, ".p a c be", ".p a c", &[], &[]);
        assert_eq!(again.parked.len(), 1);
    }

    /// Saves the current document of `session` as a project and opens it
    /// again (staged and loaded), as the app does; returns the reopened
    /// shorthand and how many places of it were rewritten.
    fn save_and_reopen(session: &ImportedSession, dsl: &str, xml: &str, decisions: &str) -> (String, usize, StagedDocument) {
        let dir = std::env::temp_dir().join(format!("saga-scribe-session-{}-{}", std::process::id(), xml.len()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("p.teis");
        let fields = crate::commands::file::ProjectFields {
            source: dsl.to_string(),
            output: xml.to_string(),
            confirmations_json: "{}".into(),
            template_id: "menota".into(),
            metadata_json: None,
            annotations_json: None,
            abbreviations_json: None,
        };
        crate::commands::file::save_project_at(session, &path, fields, Some(decisions.to_string())).unwrap();
        let mut project = crate::commands::file::read_project(&path.to_string_lossy()).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(project.manifest.version, "2.1");
        let (reopened, rewritten) = crate::commands::file::stage_project(&mut project, session).unwrap();
        let reopened = reopened.unwrap();
        session.load(Some(reopened.handle)).unwrap();
        (project.source, rewritten, reopened)
    }

    /// The way the app goes: import (staged), load, edit, save from the
    /// session (the file as it now is, `.teis` 2.0), open again, and the
    /// edit and the answer for the replaced word are in the file.
    #[test]
    fn test_imported_project_is_saved_from_the_session_and_opened_again() {
        let compile_in = |session: &ImportedSession, from: &str, dsl: &str, decisions: &[(usize, Decision)]| {
            session.with_current_mut(|source| compile_decided(source, from, dsl, decisions)).unwrap().unwrap()
        };
        let session = ImportedSession::default();
        let reply = crate::commands::import::stage(crate::importer::tei::parse(TWO_OKS).unwrap(), &session).unwrap();
        let words = reply.words.as_deref().unwrap();
        assert_eq!((words[0].lemma.as_deref(), words[0].msa.as_deref()), (Some("ok"), Some("xCC")));
        session.load(Some(reply.imported.unwrap().handle)).unwrap();
        let dsl = reply.dsl;
        compile_in(&session, &dsl, &dsl, &[]);
        let edited = dsl.replacen("ok", "aok", 1);
        let suggested = compile_in(&session, &dsl, &edited, &[]);
        let key = suggested.records[0].key;
        let before = compile_in(&session, &edited, &edited, &[(key, Decision::Different)]);
        assert!(!before.xml.contains(r#"lemma="ok""#), "{}", before.xml);

        let (source, rewritten, reopened) = save_and_reopen(&session, &edited, &before.xml, &format!(r#"{{"{key}":"different"}}"#));
        assert_eq!((rewritten, source.as_str()), (0, edited.as_str()));
        // Answered: in the file, nothing left to ask
        assert!(reopened.decisions.is_empty());
        let after = session
            .with_current_mut(|source| {
                let mut compiler = Compiler::new().with_config(source.config.clone());
                compile_imported_source(source, &mut compiler, &edited, None, &HashMap::new(), HashMap::new(), &[], None).unwrap()
            })
            .unwrap()
            .unwrap();
        assert_eq!(after.xml, before.xml);
        assert!(after.records.is_empty(), "{:?}", after.records);
    }

    /// A replaced word not yet decided when the project is saved is still
    /// asked about when it is opened again, found by its place and
    /// fingerprint in the saved file.
    #[test]
    fn test_an_open_record_is_saved_and_comes_back() {
        let session = ImportedSession::default();
        let reply = crate::commands::import::stage(crate::importer::tei::parse(TWO_OKS).unwrap(), &session).unwrap();
        session.load(Some(reply.imported.unwrap().handle)).unwrap();
        let dsl = reply.dsl;
        let compile_in = |session: &ImportedSession, from: &str, dsl: &str| {
            session.with_current_mut(|source| compile_decided(source, from, dsl, &[])).unwrap().unwrap()
        };
        compile_in(&session, &dsl, &dsl);
        let edited = dsl.replacen("ok", "aok", 1);
        let open = compile_in(&session, &dsl, &edited);
        let [record] = open.records.as_slice() else { panic!("{:?}", open.records) };
        assert_eq!((record.was.as_str(), record.decision), ("ok", None));

        let (source, _, reopened) = save_and_reopen(&session, &edited, &open.xml, "{}");
        assert!(reopened.decisions.is_empty());
        let after = session
            .with_current_mut(|s| {
                let mut compiler = Compiler::new().with_config(s.config.clone());
                compile_imported_source(s, &mut compiler, &source, None, &HashMap::new(), HashMap::new(), &[], None).unwrap()
            })
            .unwrap()
            .unwrap();
        assert_eq!(after.xml, open.xml);
        let [back] = after.records.as_slice() else { panic!("{:?}", after.records) };
        assert_eq!((back.was.as_str(), back.lemma.as_deref(), back.word, back.suggestion), ("ok", Some("ok"), Some(0), record.suggestion));
        assert_eq!(back.xml, record.xml);
    }

    #[test]
    fn test_staged_documents_load_by_handle() {
        let session = ImportedSession::default();
        let (first, _) = open(TWO_OKS);
        let first = session.stage(first).unwrap();
        let (second, _) = open(TWO_OKS);
        let second = session.stage(second).unwrap();
        assert!(first.is_menota && first.decisions.is_empty());
        // Staging another replaces the first: it can't be loaded any more
        assert!(session.load(Some(first.handle)).is_err());
        session.load(Some(second.handle)).unwrap();
        assert_eq!(session.with_current(|_| ()).unwrap(), Some(()));
        // Loaded once; clearing leaves none
        assert!(session.load(Some(second.handle)).is_err());
        session.load(None).unwrap();
        assert_eq!(session.with_current(|_| ()).unwrap(), None);
    }

    #[test]
    fn test_session_reads_words_from_the_tree_not_copies() {
        let (mut source, dsl) = open(TWO_OKS);
        assert!(source.tree.is_some());
        assert!(source.config.word_wrap && source.config.multi_level);
        for segment in &source.document.segments {
            if let crate::importer::tei::segments::Segment::Word { original_xml, attributes, .. } = segment {
                assert!(original_xml.is_empty() && attributes.is_empty(), "{segment:?}");
            }
        }
        compile(&mut source, &dsl, None, &[]);
        // An edited word keeps its lemma (read from the tree) ...
        let edited = dsl.replacen("ok", "okk", 1);
        let out = compile_decided(&mut source, &dsl, &edited, &[]);
        assert!(out.xml.contains(r#"<w lemma="ok" me:msa="xCC"><choice><me:facs>okk</me:facs>"#), "{}", out.xml);
        let [record] = out.records.as_slice() else { panic!("{:?}", out.records) };
        assert_eq!((record.lemma.as_deref(), record.msa.as_deref()), (Some("ok"), Some("xCC")));
        // The record keeps the old word's whole XML, as the file has it
        assert_eq!(
            record.xml,
            r#"<w lemma="ok" me:msa="xCC"><choice><me:facs>ok</me:facs><me:dipl>ok</me:dipl><me:norm>ok</me:norm></choice></w>"#
        );
        // ... and the next word is still the file's, untouched
        assert!(out.xml.contains("<me:norm>maðr</me:norm>"), "{}", out.xml);
    }

    #[test]
    fn test_a_curated_normalized_form_that_cant_follow_an_edit_is_marked() {
        // "maþr" has the normalized form "maðr", which the shorthand doesn't
        // give: a reading, kept when the word is edited, and marked
        let (mut source, dsl) = open(TWO_OKS);
        compile(&mut source, &dsl, None, &[]);
        let edited = dsl.replacen("maþr", "maþrr", 1);
        let out = compile_decided(&mut source, &dsl, &edited, &[]);
        let [record] = out.records.as_slice() else { panic!("{:?}", out.records) };
        assert_eq!((record.stale_levels.as_slice(), record.lemma.as_deref()), (&["norm".to_string()][..], None));
        assert!(out.xml.contains("<me:norm>maðr</me:norm>"), "{}", out.xml);
        // Kept: answered, no longer open
        let key = record.key;
        let out = compile_decided(&mut source, &edited, &edited, &[(key, Decision::Same)]);
        assert_eq!(out.records[0].decision, Some(Decision::Same));
        assert!(out.xml.contains("<me:norm>maðr</me:norm>"), "{}", out.xml);
    }

    #[test]
    fn test_the_derived_normalized_form_is_what_the_shorthand_gives() {
        let (mut source, dsl) = open(TWO_OKS);
        compile(&mut source, &dsl, None, &[]);
        let &(token, _) = source.words.as_ref().unwrap().get(1).unwrap();
        let word = source.patcher.token_text(token).unwrap().to_string();
        assert_eq!(word, "maþr");
        let mut compiler = Compiler::new().with_config(source.config.clone());
        // The file has the curated "maðr"; the shorthand alone gives "maþr"
        assert_eq!(crate::importer::tei::patching::derived_normalized(&mut compiler, &word).as_deref(), Some("maþr"));
    }

    #[test]
    fn test_replaced_word_with_annotations_is_a_record() {
        // "maþr" with the normalized form the shorthand gives (derived, so
        // it follows edits and isn't asked about)
        let xml = TWO_OKS.replace("<me:norm>maðr</me:norm>", "<me:norm>maþr</me:norm>");
        let (mut source, dsl) = open(&xml);
        compile(&mut source, &dsl, None, &[]);
        // "maþr" has no lemma: replaced, it is a record only if annotated
        let edited = dsl.replacen("maþr", "maþrr", 1);
        assert!(compile_decided(&mut source, &dsl, &edited, &[]).records.is_empty());
        let again = edited.replacen("maþrr", "maþr", 1);
        compile_decided(&mut source, &edited, &again, &[]);
        let out = compile_guided(&mut source, &again, &edited, &[], &[1]);
        let [record] = out.records.as_slice() else { panic!("{:?}", out.records) };
        assert_eq!((record.word, record.annotated, record.lemma.as_deref()), (Some(1), true, None));
        // Annotations elsewhere don't make it one; a word with a lemma is
        // one either way, marked as annotated only if it is
        let both = edited.replacen("ok", "okk", 1);
        let out = compile_guided(&mut source, &edited, &both, &[], &[1]);
        assert_eq!(out.records.len(), 2, "{:?}", out.records);
        assert!(!out.records[0].annotated && out.records[1].annotated);
        let out = compile_guided(&mut source, &both, &both, &[], &[0]);
        let [record] = out.records.as_slice() else { panic!("{:?}", out.records) };
        assert!(record.annotated && record.lemma.is_some());
    }

    #[test]
    fn test_corrected_word_is_a_record_suggested_as_the_same_word() {
        let (mut source, dsl) = open(TWO_OKS);
        compile(&mut source, &dsl, None, &[]);
        let edited = dsl.replacen("ok", "okk", 1);
        let out = compile_decided(&mut source, &dsl, &edited, &[]);
        let [record] = out.records.as_slice() else { panic!("{:?}", out.records) };
        assert_eq!((record.suggestion, record.decision, record.word), (Decision::Same, None, Some(0)));
        assert_eq!(&edited[record.from..record.to], "okk");
        assert_eq!((record.was.as_str(), record.lemma.as_deref(), record.msa.as_deref()), ("ok", Some("ok"), Some("xCC")));
        assert!(out.xml.contains(r#"<w lemma="ok" me:msa="xCC">"#), "{}", out.xml);
        // Answered as another word, it loses its lemma, analysis and normalized form
        let out = compile_decided(&mut source, &edited, &edited, &[(record.key, Decision::Different)]);
        assert_eq!(out.records[0].decision, Some(Decision::Different));
        assert!(!out.xml.contains("lemma=\"ok\"") && out.xml.contains("<me:facs>okk</me:facs>"), "{}", out.xml);
    }

    #[test]
    fn test_retyped_other_word_is_suggested_as_different_and_can_be_kept() {
        let (mut source, dsl) = open(TWO_OKS);
        compile(&mut source, &dsl, None, &[]);
        let edited = dsl.replacen("ok", "þá", 1);
        let out = compile_decided(&mut source, &dsl, &edited, &[]);
        let [record] = out.records.as_slice() else { panic!("{:?}", out.records) };
        assert_eq!(record.suggestion, Decision::Different);
        assert!(!out.xml.contains("lemma=\"ok\""), "{}", out.xml);
        let out = compile_decided(&mut source, &edited, &edited, &[(record.key, Decision::Same)]);
        assert!(out.xml.contains(r#"<w lemma="ok" me:msa="xCC">"#) && out.xml.contains("þá"), "{}", out.xml);
        // A word without a lemma has nothing to lose: no record
        let edited2 = edited.replacen("maþr", "maðr", 1);
        let out = compile_decided(&mut source, &edited, &edited2, &[(record.key, Decision::Same)]);
        assert_eq!(out.records.len(), 1, "{:?}", out.records);
    }

    #[test]
    fn test_deleted_word_is_not_asked_about_and_comes_back_as_it_was() {
        let (mut source, dsl) = open(TWO_OKS);
        compile(&mut source, &dsl, None, &[]);
        let edited = dsl.replacen(" ok", "", 1);
        let out = compile_decided(&mut source, &dsl, &edited, &[]);
        assert!(out.records.is_empty(), "{:?}", out.records);
        assert!(!out.xml.contains("lemma=\"ok\""), "{}", out.xml);
        let out = compile_decided(&mut source, &edited, &dsl, &[]);
        assert!(out.records.is_empty(), "{:?}", out.records);
        assert_eq!(out.xml, TWO_OKS);
    }

    #[test]
    fn test_different_word_does_not_get_the_old_lemma_back_from_the_editor() {
        // The app sends the file's own lemmas with every compile
        let (mut source, dsl) = open(TWO_OKS);
        compile(&mut source, &dsl, None, &[(0, "ok", "xCC")]);
        let edited = dsl.replacen("ok", "þá", 1);
        let mut compiler = Compiler::new().with_config(source.config.clone());
        let at = dsl.find("ok").unwrap();
        let guidance = Guidance {
            base_revision: source.patcher.revision(),
            changes: vec![TextChange { from_a: at, to_a: at + 2, from_b: at, to_b: at + 2 }],
        };
        let old = HashMap::from([(0, LemmaMapping { lemma: "ok".into(), msa: "xCC".into(), normalized: None })]);
        let out = compile_imported_source(&mut source, &mut compiler, &edited, Some(&guidance), &HashMap::new(), old, &[], None).unwrap();
        assert_eq!((out.records[0].suggestion, out.records[0].word), (Decision::Different, Some(0)));
        assert!(!out.xml.contains("lemma=\"ok\""), "{}", out.xml);
        // A lemma chosen for the new word is written
        let new = HashMap::from([(0, LemmaMapping { lemma: "þá".into(), msa: "xAV".into(), normalized: None })]);
        let guidance = Guidance { base_revision: source.patcher.revision(), changes: vec![] };
        let out = compile_imported_source(&mut source, &mut compiler, &edited, Some(&guidance), &HashMap::new(), new, &[], None).unwrap();
        assert!(out.xml.contains(r#"<w lemma="þá" me:msa="xAV">"#), "{}", out.xml);
    }

    #[test]
    fn test_replaced_word_record_covers_the_whole_new_word() {
        // HolmPerg line 18: "bok" (lemma bók) retyped as "bækr", letter by letter
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/HolmPerg-34-4to-MLL.xml");
        let Ok(xml) = std::fs::read_to_string(path) else { return };
        let (mut source, dsl) = open(&xml);
        compile(&mut source, &dsl, None, &[]);
        let at = dsl.find("//18  bok").unwrap() + "//18  ".len();
        let mut previous = dsl.clone();
        for typed in ["bk", "b", "bæ", "bæk", "bækr"] {
            let edited = format!("{}{typed}{}", &dsl[..at], &dsl[at + 3..]);
            let out = compile_decided(&mut source, &previous, &edited, &[]);
            let [record] = out.records.as_slice() else { panic!("{:?}", out.records) };
            let units: Vec<u16> = edited.encode_utf16().collect();
            assert_eq!(String::from_utf16_lossy(&units[record.from..record.to]), typed);
            assert_eq!((record.was.as_str(), record.lemma.as_deref()), ("bok", Some("bók")));
            previous = edited;
        }
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
            base_revision: source.patcher.revision(),
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
            base_revision: source.patcher.revision(),
            changes: vec![TextChange { from_a: from, to_a: from + units(old), from_b: from, to_b: from + units(new) }],
        };
        compile(source, &edited, Some(guidance), lemmas)
    }

    #[test]
    fn test_another_word_in_place_of_one_is_a_new_word() {
        let (mut source, dsl) = open(TWO_OKS);
        compile(&mut source, &dsl, None, &[(0, "ok", "xCC")]);
        // "ok" (lemma ok) replaced with "geck": the word's data doesn't
        // carry over (its transcription markup would follow the text)
        let out = replace_word(&mut source, &dsl, "ok", "geck", &[(0, "ok", "xCC")]);
        assert!(out.xml.contains("<w><choice><me:facs>geck</me:facs>"), "{}", out.xml);
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


#[cfg(test)]
mod word_xml_tests {
    use super::*;

    const FILE: &str = r#"<TEI xmlns="http://www.tei-c.org/ns/1.0" xmlns:me="http://www.menota.org/ns/1.0"><text><body>
<p><w lemma="ok"><me:dipl>ok</me:dipl></w> <!-- <w>not a word</w> --> <w lemma="maðr"><me:dipl>ma<ex>n</ex>n</me:dipl></w> <w><me:dipl>vas</me:dipl></w></p>
</body></text></TEI>"#;

    fn open() -> (ImportedSource, String) {
        let import = crate::importer::tei::parse(FILE).unwrap();
        let source = ImportedSource::new(
            import.imported_document.unwrap(),
            import.original_body_xml.unwrap(),
            import.original_preamble.unwrap_or_default(),
            import.original_postamble.unwrap_or_default(),
        );
        (source, import.dsl)
    }

    #[test]
    fn a_words_xml_is_set_and_the_shorthand_follows() {
        let (mut source, dsl) = open();
        let first = source.compile_plainly(&dsl).unwrap();
        assert_eq!(source.word_xml(1).unwrap(), r#"<w lemma="maðr"><me:dipl>ma<ex>n</ex>n</me:dipl></w>"#);
        // A typo in the file fixed and a lemma changed in the XML tab
        let change = source
            .set_word_xml(1, r#"<w lemma="maðr" me:msa="xNC"><me:dipl>ma<ex>n</ex>nr</me:dipl></w>"#)
            .unwrap();
        assert_eq!((change.dsl.as_str(), change.msa.as_deref()), ("ma{=n}nr", Some("xNC")));
        let dsl16: Vec<u16> = dsl.encode_utf16().collect();
        assert_eq!(String::from_utf16(&dsl16[change.from..change.to]).unwrap(), "ma{=n}n");
        let edited = format!(
            "{}{}{}",
            String::from_utf16(&dsl16[..change.from]).unwrap(),
            change.dsl,
            String::from_utf16(&dsl16[change.to..]).unwrap()
        );
        let mut compiler = Compiler::new().with_config(source.config.clone());
        let guidance = Guidance {
            base_revision: source.patcher.revision(),
            changes: vec![crate::identity::TextChange { from_a: change.from, to_a: change.to, from_b: change.from, to_b: change.from + change.dsl.encode_utf16().count() }],
        };
        let out = compile_imported_source(&mut source, &mut compiler, &edited, Some(&guidance), &HashMap::new(), HashMap::new(), &[], None).unwrap();
        assert!(out.xml.contains(r#"<w lemma="maðr" me:msa="xNC"><me:dipl>ma<ex>n</ex>nr</me:dipl></w>"#), "{}", out.xml);
        // Everything else as it was; the word keeps its number
        assert_eq!(out.xml.replace(r#" me:msa="xNC""#, "").replace("</ex>nr<", "</ex>n<"), first.xml);
        assert_eq!(out.renumbering, None);
        assert_eq!(source.word_xml(1).unwrap(), r#"<w lemma="maðr" me:msa="xNC"><me:dipl>ma<ex>n</ex>nr</me:dipl></w>"#);
    }

    #[test]
    fn only_a_single_word_is_taken() {
        let (mut source, dsl) = open();
        source.compile_plainly(&dsl).unwrap();
        assert!(source.set_word_xml(0, "<w>ok</w> <w>ok</w>").is_err());
        assert!(source.set_word_xml(0, "<w>ok").is_err());
        assert!(source.set_word_xml(0, "<pc>.</pc>").is_err());
        assert!(source.set_word_xml(9, "<w>ok</w>").is_err());
        // Nothing changed
        assert!(source.compile_plainly(&dsl).unwrap().xml.contains("<!-- <w>not a word</w> -->"));
    }
}

#[cfg(test)]
mod native_override_tests {
    use super::*;

    fn compile(session: &NativeSession, dsl: &str, change: Option<(usize, usize, usize, usize)>, notes: bool) -> String {
        let mut annotations = AnnotationSet::new();
        if notes {
            annotations.annotations.push(
                serde_json::from_value(serde_json::json!({
                    "id": "n", "type": "note", "target": {"type": "word", "wordIndex": 1},
                    "value": {"kind": "note", "text": "a note"}
                }))
                .unwrap(),
            );
        }
        let config = CompilerConfig { word_wrap: true, multi_level: true, ..Default::default() };
        let compiler = Compiler::new().with_config(config).with_annotations(&annotations);
        let pending = std::mem::take(&mut *session.1.lock().unwrap());
        let mut state = session.0.lock().unwrap();
        let guidance = change.map(|(from_a, to_a, from_b, to_b)| Guidance {
            base_revision: state.as_ref().map_or(0, |s| s.tracked.revision),
            changes: vec![crate::identity::TextChange { from_a, to_a, from_b, to_b }],
        });
        compile_tracking_words_with(compiler, dsl, guidance.as_ref(), &mut state, pending).unwrap().0
    }

    #[test]
    fn a_new_documents_word_keeps_the_xml_set_for_it() {
        let session = NativeSession::default();
        let body = compile(&session, "ok maþr vas", None, false);
        let compiled = session.word_xml(1).unwrap();
        assert!(body.contains(&compiled) && compiled.contains("<me:dipl>maþr</me:dipl>"), "{compiled}");
        // Markup the shorthand doesn't show, set in the word panel
        let xml = compiled.replace("<me:dipl>maþr</me:dipl>", "<me:dipl>ma<hi rend=\"red\">þ</hi>r</me:dipl>");
        let change = session.set_word_xml(1, &xml).unwrap();
        assert_eq!((change.from, change.to), (3, 7));
        let body = compile(&session, "ok maþr vas", Some((3, 7, 3, 7)), false);
        assert!(body.contains("ma<hi rend=\"red\">þ</hi>r</me:dipl>"), "{body}");
        // Edited in the shorthand: the edit is merged into it, and an
        // annotation added since is written into it too
        let body = compile(&session, "ok maþrr vas", Some((7, 7, 7, 8)), true);
        assert!(body.contains("ma<hi rend=\"red\">þ</hi>rr</me:dipl>"), "{body}");
        assert!(body.contains("a note"), "{body}");
        // Saved and opened again: the first compile takes it back
        let saved = session.saved_overrides();
        assert_eq!(saved.len(), 1);
        let reopened = NativeSession::default();
        reopened.restore(saved);
        let again = compile(&reopened, "ok maþrr vas", None, true);
        assert_eq!(again, body);
    }
}

#[cfg(test)]
mod imported_annotation_tests {
    use super::*;

    #[test]
    fn notes_and_annotations_are_written_into_an_imported_file() {
        let xml = r#"<TEI xmlns="http://www.tei-c.org/ns/1.0" xmlns:me="http://www.menota.org/ns/1.0"><text><body>
<p><w lemma="ok"><me:dipl>ok</me:dipl></w> <w><me:dipl>maþr</me:dipl></w> <w><me:dipl>vas</me:dipl></w></p>
</body></text></TEI>"#;
        let import = crate::importer::tei::parse(xml).unwrap();
        let mut source = ImportedSource::new(
            import.imported_document.unwrap(),
            import.original_body_xml.unwrap(),
            import.original_preamble.unwrap_or_default(),
            import.original_postamble.unwrap_or_default(),
        );
        let annotations: AnnotationSet = serde_json::from_value(serde_json::json!({
            "version": "1.0",
            "annotations": [
                {"id": "n1", "type": "note", "target": {"type": "word", "wordIndex": 1}, "value": {"kind": "note", "text": "a note"}},
                {"id": "s1", "type": "semantic", "target": {"type": "word", "wordIndex": 1}, "value": {"kind": "semantic", "category": "person"}}
            ]
        }))
        .unwrap();
        let mut compiler = Compiler::new().with_config(source.config.clone());
        let out = compile_imported_source(&mut source, &mut compiler, &import.dsl, None, &HashMap::new(), HashMap::new(), &[], Some(&annotations)).unwrap();
        assert!(out.xml.contains(r##"<w ana="#person"><me:dipl>maþr</me:dipl><note>a note</note></w>"##), "{}", out.xml);
        // The other words, and without annotations the file, as they were
        assert!(out.xml.contains(r#"<w lemma="ok"><me:dipl>ok</me:dipl></w> "#), "{}", out.xml);
        let mut compiler = Compiler::new().with_config(source.config.clone());
        let plain = compile_imported_source(&mut source, &mut compiler, &import.dsl, None, &HashMap::new(), HashMap::new(), &[], None).unwrap();
        assert_eq!(plain.xml, xml);
        // The word panel reads the word with them
        let mut compiler = Compiler::new().with_config(source.config.clone());
        compile_imported_source(&mut source, &mut compiler, &import.dsl, None, &HashMap::new(), HashMap::new(), &[], Some(&annotations)).unwrap();
        assert!(source.word_xml(1).unwrap().contains("<note>a note</note>"));
    }
}

#[cfg(test)]
mod abbreviation_store_tests {
    use super::*;

    #[test]
    fn a_short_form_takes_the_stores_expansion() {
        let et = r#"<w><choice><me:facs><am>&et;</am></me:facs><me:dipl><ex>ok</ex></me:dipl><me:norm>ok</me:norm></choice></w>"#;
        let xml = format!(
            r#"<TEI xmlns="http://www.tei-c.org/ns/1.0" xmlns:me="http://www.menota.org/ns/1.0"><text><body><p>{et} <w><choice><me:facs>a</me:facs><me:dipl>a</me:dipl><me:norm>a</me:norm></choice></w> {et}</p></body></text></TEI>"#
        );
        let import = crate::importer::tei::parse(&xml).unwrap();
        // Twice with the same expansion: in the store, and written short
        assert_eq!(import.dsl, ".p {:et:} a {:et:}");
        let document = import.imported_document.unwrap();
        assert_eq!(document.abbreviations.get(":et:"), Some(&vec!["ok".to_string()]));
        let mut source = ImportedSource::new(
            document,
            import.original_body_xml.unwrap(),
            import.original_preamble.unwrap_or_default(),
            import.original_postamble.unwrap_or_default(),
        );
        let store: HashMap<String, String> = [(":et:".to_string(), "ok".to_string())].into();
        let mut compiler = Compiler::new().with_config(source.config.clone()).with_abbreviations(&store);
        let out = compile_imported_source(&mut source, &mut compiler, ".p {:et:} a {:et:} {:et:}", None, &HashMap::new(), HashMap::new(), &[], None).unwrap();
        // The new word has the expansion; the file's words are as they were
        assert!(out.xml.contains("<me:facs><am>&et;</am></me:facs>\n    <me:dipl><ex>ok</ex></me:dipl>"), "{}", out.xml);
        assert!(out.xml.starts_with(&xml[..xml.rfind("</p>").unwrap()]), "{}", out.xml);
    }
}
