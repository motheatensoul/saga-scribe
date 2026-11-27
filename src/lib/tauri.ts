import { invoke } from "@tauri-apps/api/core";
import type { Template } from "./stores/template";
import type { Entity, EntityMap } from "./stores/entities";

export interface Settings {
  fontSize: number;
  theme: string;
  autoPreview: boolean;
  previewDelay: number;
  activeTemplateId: string | null;
  activeStylesheetId: string;
  showEntityGlyphs: boolean;
}

export async function loadSettings(): Promise<Settings> {
  return invoke("load_settings");
}

export async function saveSettings(settings: Settings): Promise<void> {
  return invoke("save_settings", { settings });
}

export async function getSystemTheme(): Promise<string> {
  return invoke("get_system_theme");
}

export async function setWindowTheme(theme: string): Promise<void> {
  return invoke("set_window_theme", { theme });
}

export interface StylesheetEntry {
  id: string;
  name: string;
  path: string;
  builtIn: boolean;
}

export async function listStylesheets(): Promise<StylesheetEntry[]> {
  return invoke("list_stylesheets");
}

export async function importStylesheet(
  path: string,
): Promise<StylesheetEntry> {
  return invoke("import_stylesheet", { path });
}

export async function deleteStylesheet(id: string): Promise<void> {
  return invoke("delete_stylesheet", { id });
}

export interface FileContent {
  path: string;
  content: string;
}

export async function openFile(path: string): Promise<FileContent> {
  return invoke("open_file", { path });
}

export async function saveFile(path: string, content: string): Promise<void> {
  return invoke("save_file", { path, content });
}

export async function exportTei(
  path: string,
  teiContent: string,
): Promise<void> {
  return invoke("export_tei", { path, teiContent });
}

export async function exportHtml(
  path: string,
  htmlContent: string,
): Promise<void> {
  return invoke("export_html", { path, htmlContent });
}

export async function loadTextFile(path: string): Promise<string> {
  return invoke("load_text_file", { path });
}

/**
 * An imported document the backend has read and keeps until the editor loads
 * it with {@link setImportedDocument}: the manifest stays in the backend.
 */
export interface StagedDocument {
  handle: number;
  /** Whether the document has MENOTA levels (me:facs/dipl/norm) */
  isMenota: boolean;
  /** DSL rules version the document was imported with */
  dslRules: number;
  /** The editor's answers for replaced words, by the original word's segment id */
  decisions: Record<number, Decision>;
  /** The abbreviation store its shorthand is written with */
  abbreviations?: Record<string, string[]>;
  /** How the file encodes its transcription (inferred when it was imported) */
  profile?: Profile | null;
}

/** A transcription level of a word */
export type Level = "facs" | "dipl" | "norm";

/** How an imported file encodes its transcription (`src-tauri/src/document/profile.rs`) */
export interface Profile {
  /** The levels new words are written with, in order */
  levels: Level[];
  /** Whether a word's levels are in a `<choice>` */
  choice: boolean;
  abbreviations: "am_ex" | "choice_abbr_expan";
  punctuation: "pc" | "me_punct";
  /** The edition `//n` stands for, when the file has several lineations */
  lineEdition?: string | null;
  counts: {
    words: number;
    /** Words, numerals and punctuation */
    tokens: number;
    /** Tokens with each level */
    levels: Partial<Record<Level, number>>;
    wordsWithChoice: number;
    am: number;
    ex: number;
    choiceAbbr: number;
    pc: number;
    mePunct: number;
    /** Line breaks by edition ("" for none) */
    lineEditions: Record<string, number>;
    /** Elements the shorthand has no syntax for, by name */
    hidden: Record<string, number>;
  };
}

/** A word whose XML was set in the word panel: its shorthand now, for `from..to` of the editor (UTF-16) */
export interface WordXmlChange {
  from: number;
  to: number;
  dsl: string;
  lemma?: string | null;
  msa?: string | null;
}

/** The XML of word `word` of the current imported document, as the last compile wrote it. */
export async function importedWordXml(word: number): Promise<string> {
  return invoke("imported_word_xml", { word });
}

/** Sets the XML of word `word` of the current imported document (the word panel's XML tab). */
export async function setImportedWordXml(word: number, xml: string): Promise<WordXmlChange> {
  return invoke("set_imported_word_xml", { word, xml });
}

/** An element of the file as the word panel's Markup tab shows it. */
export interface ElementInfo {
  name: string;
  /** How many elements of the same name come before it in the body */
  ordinal: number;
  /** "around": the word is in it; "inside": in the word; "before": between the word and the token before it */
  place: "around" | "inside" | "before";
  attributes: [string, string][];
  /** Attributes the shorthand writes (a break's `n`) */
  fixed: string[];
}

/**
 * Makes a new document's file: `xml`, compiled once from the template and the
 * shorthand so far, is staged like an imported file and edited as its tree
 * from then on. `dsl` is kept where it reads as the file.
 */
export async function createDocument(xml: string, dsl: string, multiLevel: boolean, wordWrap: boolean): Promise<ImportResult> {
  return invoke("create_document", { xml, dsl, multiLevel, wordWrap });
}

/** Writes the metadata editor's metadata into the current imported document's `<teiHeader>` (the editor then compiles). */
export async function setImportedMetadata(metadataJson: string): Promise<void> {
  return invoke("set_imported_metadata", { metadataJson });
}

/** The elements word `word` of the current imported document is in, has in it and has before it. */
export async function importedWordElements(word: number): Promise<ElementInfo[]> {
  return invoke("imported_word_elements", { word });
}

/** Sets the attributes of an element of the current imported document (the editor then compiles). */
export async function setImportedElementAttributes(
  name: string,
  ordinal: number,
  attributes: [string, string][],
): Promise<void> {
  return invoke("set_imported_element_attributes", { name, ordinal, attributes });
}

/** The XML of word `word` of the new document, as the last compile wrote it. */
export async function nativeWordXml(word: number): Promise<string> {
  return invoke("native_word_xml", { word });
}

/** Sets the XML of word `word` of the new document: compiles put it in place of the compiled word. */
export async function setNativeWordXml(word: number, xml: string): Promise<WordXmlChange> {
  return invoke("set_native_word_xml", { word, xml });
}

/** The normalized form the shorthand gives word `word` of the current imported document (null without a normalized level). */
export async function importedDerivedNormalized(word: number, options: CompileImportedOptions): Promise<string | null> {
  return invoke("imported_derived_normalized", {
    word,
    entitiesJson: options.entitiesJson ?? null,
    normalizerJson: options.normalizerJson ?? null,
    entityMappingsJson: options.entityMappingsJson ?? null,
    customMappings: options.customMappings ?? null,
    abbreviationsJson: options.abbreviationsJson ?? null,
  });
}

/** Sets the levels new words of the current imported document get (after the editor confirmed them). */
export async function setImportedLevels(levels: Level[], choice: boolean): Promise<Profile> {
  return invoke("set_imported_levels", { levels, choice });
}

/** What an imported file says about one of its words */
export interface ImportedWord {
  lemma?: string;
  msa?: string;
  normalized?: string;
  diplomatic?: string;
  facsimile?: string;
}

/**
 * The answer for a word of the original replaced by another word in the DSL:
 * the same word corrected (keeps lemma, analysis, normalized form), or
 * another word in its place (loses them).
 */
export type Decision = "same" | "different";

/** A word of the original that the edited DSL replaces, with the lemma and analysis it would lose. */
export interface WordRecord {
  /** The original word's segment id; decisions are kept by it */
  key: number;
  /** The answer the XML follows until the editor decides */
  suggestion: Decision;
  decision: Decision | null;
  /** The new word's number in the compiled text (as the previews count), if it is a <w> */
  word: number | null;
  /** Where the new word is in the DSL that was compiled (UTF-16) */
  from: number;
  to: number;
  /** The old word's shorthand */
  was: string;
  /** The old word's XML as the file has it */
  xml: string;
  lemma: string | null;
  msa: string | null;
  /** The word has annotations other than its lemma; they stay with it whatever the answer */
  annotated: boolean;
  /** Levels kept as the file has them that may be out of date after the edit ("norm", "dipl") */
  staleLevels?: string[];
}

/** Result from importing a file */
export interface ImportResult {
  /** The DSL content extracted from the body */
  dsl: string;
  /** Metadata extracted from teiHeader (if present) */
  metadata?: import("$lib/types/metadata").Metadata;
  /** The imported document, staged in the backend (imported mode) */
  imported?: StagedDocument;
  /** The file's words, numbered as the rendered text counts them (imported mode) */
  words?: ImportedWord[];
  /** Indicates imported mode for round-trip fidelity */
  isImportedMode?: boolean;
  /** Why the file isn't well-formed XML, if it isn't (it was imported as libxml2 repaired it) */
  notWellFormed?: string;
}

/**
 * Import a file and convert it to DSL format, also extracting metadata.
 * The backend runs this on a separate async task to avoid blocking the UI.
 */
export async function importFile(path: string, lineEdition?: string): Promise<ImportResult> {
  return invoke("import_file", { path, lineEdition });
}

export async function listTemplates(): Promise<Template[]> {
  return invoke("list_templates");
}

export async function getTemplate(id: string): Promise<Template> {
  return invoke("get_template", { id });
}

export async function saveTemplate(template: Template): Promise<void> {
  return invoke("save_template", { template });
}

export async function deleteTemplate(id: string): Promise<void> {
  return invoke("delete_template", { id });
}

export interface CompileOptions {
  wordWrap?: boolean;
  autoLineNumbers?: boolean;
  multiLevel?: boolean;
  wrapPages?: boolean;
  entitiesJson?: string;
  normalizerJson?: string;
  entityMappingsJson?: string;
  customMappings?: Record<string, string>;
  lemmaMappingsJson?: string;
  annotationsJson?: string;
  /** The abbreviation store's defaults (mark → expansion), which `{mark}` takes */
  abbreviationsJson?: string;
}

export async function compileDsl(
  input: string,
  templateHeader: string,
  templateFooter: string,
  options?: CompileOptions,
  guidance?: CompileGuidance,
  xmlBase?: number,
): Promise<CompileResult> {
  return invoke("compile_dsl", {
    input,
    templateHeader,
    templateFooter,
    wordWrap: options?.wordWrap ?? false,
    autoLineNumbers: options?.autoLineNumbers ?? false,
    multiLevel: options?.multiLevel ?? false,
    wrapPages: options?.wrapPages ?? false,
    entitiesJson: options?.entitiesJson ?? null,
    normalizerJson: options?.normalizerJson ?? null,
    entityMappingsJson: options?.entityMappingsJson ?? null,
    customMappings: options?.customMappings ?? null,
    lemmaMappingsJson: options?.lemmaMappingsJson ?? null,
    annotationsJson: options?.annotationsJson ?? null,
    abbreviationsJson: options?.abbreviationsJson ?? null,
    guidance: guidance ?? null,
    xmlBase: xmlBase ?? null,
  });
}

export interface CompileImportedOptions {
  entitiesJson?: string;
  normalizerJson?: string;
  entityMappingsJson?: string;
  customMappings?: Record<string, string>;
  /** Confirmed lemmas by word number (written into the file's words) */
  lemmaMappingsJson?: string;
  /** Words (by number) with annotations other than lemmas, which replaced words report */
  annotatedWords?: number[];
  /** All annotations, whose notes and attributes are written into the file's words */
  annotationsJson?: string;
  /** The abbreviation store's defaults (mark → expansion), which `{mark}` takes */
  abbreviationsJson?: string;
}

/**
 * Makes the staged document with `handle` (from importing or opening it) the
 * one compiles go to, or clears it with `null`.
 */
export async function setImportedDocument(handle: number | null): Promise<void> {
  return invoke("set_imported_document", { handle });
}

/** An editor change: `from_a..to_a` of the previous DSL became `from_b..to_b` (UTF-16 offsets). */
export interface TextChange {
  from_a: number;
  to_a: number;
  from_b: number;
  to_b: number;
}

/** The editor's changes since the compile of revision `baseRevision`. */
export interface CompileGuidance {
  baseRevision: number;
  changes: TextChange[];
}

/** A change to a document: `from..to` (UTF-16 offsets) replaced by `insert`. */
export interface XmlDelta {
  from: number;
  to: number;
  insert: string;
}

/**
 * Result of {@link compileDsl} and {@link compileImported}: the document
 * whole (`xml`), or, if the caller holds the output of revision `xmlBase`,
 * only the change to it (`delta`). `compileSession` puts it back together.
 */
export interface CompileResult {
  xml?: string | null;
  delta?: XmlDelta | null;
  /** Identifies this compile's DSL, for {@link CompileGuidance} */
  revision: number;
  /**
   * Where the previous compile's words went: for each of its word numbers,
   * the number now (null: deleted); absent if no word moved. Annotations
   * are numbered by the previous compile (and sent so) until renumbered.
   */
  renumbering?: (number | null)[] | null;
  /** Words of the previous compile that are gone: their annotations are kept aside under the ticket */
  parked?: { word: number; ticket: string }[];
  /** Words gone in an earlier compile that are back (moved), with their numbers now */
  returned?: { ticket: string; word: number }[];
  /**
   * Words (by number) edited in the DSL whose lemma, analysis and normalized
   * form were kept from the original and should be reviewed
   */
  editedWords: number[];
  /** Words of the original replaced by others in the DSL (imported documents) */
  records: WordRecord[];
}

/** Compiles edited DSL against the document loaded with {@link setImportedDocument}. */
export async function compileImported(
  editedDsl: string,
  options?: CompileImportedOptions,
  guidance?: CompileGuidance,
  xmlBase?: number,
  decisions?: Record<number, Decision>,
): Promise<CompileResult> {
  return invoke("compile_imported", {
    editedDsl,
    entitiesJson: options?.entitiesJson ?? null,
    normalizerJson: options?.normalizerJson ?? null,
    entityMappingsJson: options?.entityMappingsJson ?? null,
    customMappings: options?.customMappings ?? null,
    lemmaMappingsJson: options?.lemmaMappingsJson ?? null,
    guidance: guidance ?? null,
    decisions: decisions ?? null,
    annotatedWords: options?.annotatedWords?.length ? options.annotatedWords : null,
    annotationsJson: options?.annotationsJson ?? null,
    abbreviationsJson: options?.abbreviationsJson ?? null,
    xmlBase: xmlBase ?? null,
  });
}

// Entity functions
export async function loadEntities(path: string): Promise<EntityMap> {
  return invoke("load_entities", { path });
}

export async function getEntity(
  path: string,
  name: string,
): Promise<Entity | null> {
  return invoke("get_entity", { path, name });
}

export async function listEntityNames(path: string): Promise<string[]> {
  return invoke("list_entity_names", { path });
}

// Custom entity mapping functions
export async function loadCustomMappings(): Promise<Record<string, string>> {
  return invoke("load_custom_mappings");
}

export async function saveEntityMapping(
  entity: string,
  translation: string,
): Promise<void> {
  return invoke("save_entity_mapping", { entity, translation });
}

export async function removeEntityMapping(entity: string): Promise<void> {
  return invoke("remove_entity_mapping", { entity });
}

export async function clearCustomMappings(): Promise<void> {
  return invoke("clear_custom_mappings");
}

// Custom entity definition functions
export async function loadCustomEntities(): Promise<EntityMap> {
  return invoke("load_custom_entities");
}

export async function saveCustomEntity(
  name: string,
  unicode: string,
  charValue: string,
  description: string,
  category: string,
): Promise<void> {
  return invoke("save_custom_entity", {
    name,
    unicode,
    charValue,
    description,
    category,
  });
}

export async function removeCustomEntity(name: string): Promise<void> {
  return invoke("remove_custom_entity", { name });
}

export async function clearCustomEntities(): Promise<void> {
  return invoke("clear_custom_entities");
}

export async function validateEntityName(name: string): Promise<boolean> {
  return invoke("validate_entity_name", { name });
}

// ONP Dictionary types and functions

export interface OnpEntry {
  release: string;
  lemma: string;
  lemma_mod: string;
  language: string;
  id: string;
  part_of_speech: string[];
  formats: string[];
  citations: number;
}

export interface OnpSense {
  definition: string;
}

export interface OnpFullEntry {
  context?: string;
  entry_type?: string;
  language: string;
  part_of_speech: string;
  canonical_form: { written_rep: string };
  citations: string;
  canonical_url: string;
  senses: OnpSense[];
}

export interface InflectedForm {
  onp_id: string;
  lemma: string;
  analysis: string; // MENOTA me:msa morphological analysis
  part_of_speech: string;
  facsimile?: string; // Facsimile-level form (glyphs resolved)
  diplomatic?: string; // Diplomatic-level form (base letters, abbr expanded) - lookup key
  normalized?: string; // me:norm level canonical form
}

export interface InflectionStore {
  forms: Record<string, InflectedForm[]>;
}

// Load ONP headwords from file
export async function loadOnpHeadwords(path: string): Promise<number> {
  return invoke("load_onp_headwords", { path });
}

// Look up lemma candidates for a wordform
export async function lookupLemma(wordform: string): Promise<OnpEntry[]> {
  return invoke("lookup_lemma", { wordform });
}

// Search for lemmas by prefix (autocomplete)
export async function searchLemmaPrefix(
  prefix: string,
  limit: number = 20,
): Promise<OnpEntry[]> {
  return invoke("search_lemma_prefix", { prefix, limit });
}

// Get a specific ONP entry by ID
export async function getOnpEntry(id: string): Promise<OnpEntry | null> {
  return invoke("get_onp_entry", { id });
}

// Fetch full entry data from ONP API
export async function fetchOnpFullEntry(id: string): Promise<OnpFullEntry> {
  return invoke("fetch_onp_full_entry", { id });
}

// Load user inflection mappings
export async function loadInflections(): Promise<InflectionStore> {
  return invoke("load_inflections");
}

// Look up inflections for a wordform
export async function lookupInflection(
  wordform: string,
): Promise<InflectedForm[]> {
  return invoke("lookup_inflection", { wordform });
}

// Add an inflection mapping
// wordform should be the diplomatic-level form for consistent lookups
export async function addInflection(
  wordform: string,
  onpId: string,
  lemma: string,
  analysis: string,
  partOfSpeech: string,
  facsimile?: string,
  diplomatic?: string,
  normalized?: string,
): Promise<void> {
  return invoke("add_inflection", {
    wordform,
    onpId,
    lemma,
    analysis,
    partOfSpeech,
    facsimile: facsimile || null,
    diplomatic: diplomatic || null,
    normalized: normalized || null,
  });
}

// Remove an inflection mapping
export async function removeInflection(
  wordform: string,
  onpId: string,
  analysis: string,
): Promise<void> {
  return invoke("remove_inflection", {
    wordform,
    onpId,
    analysis,
  });
}

// Clear all inflection mappings
export async function clearInflections(): Promise<void> {
  return invoke("clear_inflections");
}

// Export inflection dictionary to a file (returns number of entries exported)
export async function exportInflections(path: string): Promise<number> {
  return invoke("export_inflections", { path });
}

// Check if ONP registry is loaded
export async function isOnpLoaded(): Promise<boolean> {
  return invoke("is_onp_loaded");
}

// Get ONP registry stats
export async function getOnpStats(): Promise<[number, number]> {
  return invoke("get_onp_stats");
}

// Project archive types and functions

export interface ProjectManifest {
  version: string;
  template_id: string;
  created: string;
  modified: string;
}

export interface LemmaConfirmation {
  lemma: string;
  msa: string;
  normalized?: string;
}

export interface ProjectData {
  source: string;
  output: string;
  /** Legacy lemma confirmations (for backward compat) */
  confirmations: Record<number, LemmaConfirmation>;
  manifest: ProjectManifest;
  /** Optional manuscript metadata (new in v1.1) */
  metadata?: import("$lib/types/metadata").Metadata;
  /** Full annotation set (new in v1.2) */
  annotations?: import("$lib/types/annotations").AnnotationSet;
  /** The imported document, staged in the backend (imported projects) */
  imported?: StagedDocument;
  /** Annotations dropped because their words weren't found in the saved file any more */
  unplacedAnnotations?: number;
  /** The abbreviation store (2.0) */
  abbreviations?: Record<string, string[]>;
  /** Places of the shorthand rewritten to read as the file is imported now (a project saved with older rules) */
  rewritten?: number;
}

/**
 * Save project archive (.teis). Resolves to why the compiled TEI-XML isn't
 * well-formed, if it isn't (the project is saved all the same).
 */
export async function saveProject(
  path: string,
  source: string,
  output: string,
  confirmationsJson: string,
  templateId: string,
  metadataJson?: string,
  annotationsJson?: string,
  /** The answers for replaced words, for an imported document (the backend has the rest of it) */
  decisionsJson?: string,
  /** The abbreviation store */
  abbreviationsJson?: string,
): Promise<string | null> {
  return invoke("save_project", {
    path,
    source,
    output,
    confirmationsJson,
    templateId,
    metadataJson,
    annotationsJson,
    decisionsJson,
    abbreviationsJson,
  });
}

// Open project archive (.teis)
export async function openProject(path: string): Promise<ProjectData> {
  return invoke("open_project", { path });
}

// XML Validation types and functions

export interface SchemaInfo {
  id: string;
  name: string;
  description: string;
  fileName: string;
}

export interface ValidationError {
  message: string;
  line: number | null;
  column: number | null;
  isWarning: boolean;
}

export interface ValidationResult {
  valid: boolean;
  schemaName: string;
  errors: ValidationError[];
  errorCount: number;
  warningCount: number;
}

// List available validation schemas
export async function listSchemas(): Promise<SchemaInfo[]> {
  return invoke("list_schemas");
}

// Validate XML content against a schema
export async function validateXml(
  xmlContent: string,
  schemaId: string,
): Promise<ValidationResult> {
  return invoke("validate_xml", { xmlContent, schemaId });
}

// Validate XML with a custom schema string
export async function validateXmlWithSchema(
  xmlContent: string,
  schemaContent: string,
  schemaName: string,
): Promise<ValidationResult> {
  return invoke("validate_xml_with_schema", {
    xmlContent,
    schemaContent,
    schemaName,
  });
}

// ============================================================================
// Metadata Commands
// ============================================================================

import type { Metadata } from "$lib/types/metadata";

// Generate TEI header from structured metadata
export async function generateTeiHeader(
  metadataJson: string,
  includeMenotaNs: boolean,
): Promise<string> {
  return invoke("generate_tei_header", { metadataJson, includeMenotaNs });
}

// Generate TEI footer (closing tags)
export async function generateTeiFooter(): Promise<string> {
  return invoke("generate_tei_footer");
}

// Validate metadata JSON structure
export async function validateMetadata(metadataJson: string): Promise<boolean> {
  return invoke("validate_metadata", { metadataJson });
}

// Create empty metadata with default values
export async function createEmptyMetadata(): Promise<string> {
  return invoke("create_empty_metadata");
}

// Re-export Metadata type for convenience
export type { Metadata };

// Direct PDF export (Linux, Windows): the backend lays the document out in a
// hidden window and writes the PDF to `path`. See src-tauri/src/commands/pdf.rs.

/** Error message `exportPdf` rejects with when cancelled. */
export const PDF_EXPORT_CANCELLED = "PDF export cancelled";

export async function pdfExportSupported(): Promise<boolean> {
  return invoke("pdf_export_supported");
}

export async function exportPdf(html: string, path: string): Promise<void> {
  return invoke("export_pdf", { html, path });
}

export async function cancelPdfExport(): Promise<void> {
  return invoke("cancel_pdf_export");
}

/**
 * Shows a system notification if the window doesn't have focus, e.g. when an export
 * finishes in the background. Never fails: a notification that can't be shown is
 * only logged.
 */
export async function notifyInBackground(title: string, body: string): Promise<void> {
  await invoke("notify", { title, body }).catch(() => {});
}

/** Used by the hidden print window only. */
export async function takePrintDocument(): Promise<string> {
  return invoke("take_print_document");
}

/** Used by the hidden print window only. */
export async function printDocumentReady(): Promise<void> {
  return invoke("print_document_ready");
}

/** Used by the hidden print window only. */
export async function printDocumentFailed(message: string): Promise<void> {
  return invoke("print_document_failed", { message });
}

/** A DSL syntax error; offsets are UTF-16 positions in the source, as CodeMirror uses. */
export interface DslDiagnostic {
  from: number;
  to: number;
  message: string;
}

/** Reports every syntax error in DSL source (the parser recovers and keeps going). */
export async function checkDsl(input: string): Promise<DslDiagnostic[]> {
  return invoke("check_dsl", { input });
}
