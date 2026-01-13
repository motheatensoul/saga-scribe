import { invoke } from "@tauri-apps/api/core";
import type { Template } from "./stores/template";
import type { Entity, EntityMap } from "./stores/entities";

export interface Settings {
  fontSize: number;
  theme: string;
  autoPreview: boolean;
  previewDelay: number;
  activeTemplateId: string | null;
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

/** Result from importing a file */
export interface ImportResult {
  /** The DSL content extracted from the body */
  dsl: string;
  /** Metadata extracted from teiHeader (if present) */
  metadata?: import("$lib/types/metadata").Metadata;
}

/**
 * Import a file and convert it to DSL format, also extracting metadata.
 * The backend runs this on a separate async task to avoid blocking the UI.
 */
export async function importFile(path: string): Promise<ImportResult> {
  return invoke("import_file", { path });
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
}

export async function compileDsl(
  input: string,
  templateHeader: string,
  templateFooter: string,
  options?: CompileOptions,
): Promise<string> {
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
  key: string;
  def: string;
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
}

// Save project archive (.teis)
export async function saveProject(
  path: string,
  source: string,
  output: string,
  confirmationsJson: string,
  templateId: string,
  metadataJson?: string,
  annotationsJson?: string,
): Promise<void> {
  return invoke("save_project", {
    path,
    source,
    output,
    confirmationsJson,
    templateId,
    metadataJson,
    annotationsJson,
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
