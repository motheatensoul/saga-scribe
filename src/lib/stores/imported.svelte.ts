import {
  compileImported,
  setImportedDocument,
  type CompileImportedOptions,
  type Decision,
  type ImportedDocument,
  type Segment,
  type WordRecord,
} from "$lib/tauri";
import { compileSession } from "$lib/stores/compileSession";

export interface ImportedState {
  isImportedMode: boolean;
  segments: Segment[];
  spans: ImportedDocument["spans"];
  originalBodyXml: string;
  originalPreamble: string;
  originalPostamble: string;
  frontMatter: string;
  backMatter: string;
  isMenota: boolean;
  dslRules: number;
  decisions: Record<number, Decision>;
}

class ImportedStore {
  isImportedMode = $state(false);
  segments = $state<Segment[]>([]);
  spans = $state<ImportedDocument["spans"]>(undefined);
  originalBodyXml = $state("");
  originalPreamble = $state("");
  originalPostamble = $state("");
  frontMatter = $state("");
  backMatter = $state("");
  isMenota = $state(false);
  /** DSL rules version the document was imported with; kept so its DSL still matches */
  dslRules = $state(0);
  /**
   * Words edited since import that kept the original's lemma, analysis and
   * normalized form; updated by every compile
   */
  editedWords = $state<ReadonlySet<number>>(new Set());
  /** Words of the original replaced or deleted in the DSL, from the last compile */
  records = $state<WordRecord[]>([]);
  /** The editor's answers for them, by the original word's key; saved with the manifest */
  decisions = $state<Record<number, Decision>>({});
  /** Called after an answer changes, to compile with it (set by the page) */
  onDecided: (() => void) | null = null;

  /**
   * Backend session updates, chained so they apply in call order: a quick
   * reset() + load() must never let the clear land after the load.
   */
  private backendSync: Promise<void> = Promise.resolve();

  private syncBackend(document: Parameters<typeof setImportedDocument>[0]) {
    this.backendSync = this.backendSync
      .catch(() => {})
      .then(() => setImportedDocument(document));
  }

  reset() {
    this.isImportedMode = false;
    this.editedWords = new Set();
    this.records = [];
    this.decisions = {};
    compileSession.reset();
    this.segments = [];
    this.spans = undefined;
    this.originalBodyXml = "";
    this.originalPreamble = "";
    this.originalPostamble = "";
    this.frontMatter = "";
    this.backMatter = "";
    this.isMenota = false;
    this.dslRules = 0;
    this.syncBackend(null);
  }

  load(data: Partial<ImportedState>) {
    this.isImportedMode = true;
    this.editedWords = new Set();
    this.records = [];
    this.decisions = { ...(data.decisions ?? {}) };
    compileSession.reset();
    if (data.segments) this.segments = data.segments;
    this.spans = data.spans ?? undefined;
    if (data.originalBodyXml) this.originalBodyXml = data.originalBodyXml;
    if (data.originalPreamble) this.originalPreamble = data.originalPreamble;
    if (data.originalPostamble) this.originalPostamble = data.originalPostamble;
    if (data.frontMatter) this.frontMatter = data.frontMatter;
    if (data.backMatter) this.backMatter = data.backMatter;
    if (data.isMenota !== undefined) this.isMenota = data.isMenota;
    this.dslRules = data.dslRules ?? 0;
    this.syncBackend({
      segmentsJson: this.buildManifestJson(),
      originalBodyXml: this.originalBodyXml,
      preamble: this.originalPreamble,
      postamble: this.originalPostamble,
    });
  }

  /** Full manifest JSON, sent to the backend on load and written when saving projects. */
  buildManifestJson(): string {
    const manifest: ImportedDocument = {
      segments: this.segments,
      is_menota: this.isMenota,
      dsl_rules: this.dslRules,
      spans: this.spans,
      decisions: this.decisions,
    };
    return JSON.stringify(manifest);
  }

  /** Answers for the original word `key`, and compiles with the answer. */
  decide(key: number, decision: Decision) {
    this.decisions = { ...this.decisions, [key]: decision };
    this.onDecided?.();
  }

  /** Answers several open records with their suggestions. */
  acceptSuggestions(records: WordRecord[]) {
    if (records.length === 0) return;
    const decisions = { ...this.decisions };
    for (const record of records) decisions[record.key] = record.suggestion;
    this.decisions = decisions;
    this.onDecided?.();
  }

  /**
   * Compiles the edited DSL into the original document (in turn, see
   * compileSession). `options` is called when the compile is made, so that
   * lemmas are numbered as the backend expects.
   */
  async compile(editedDsl: string, options: () => CompileImportedOptions): Promise<string> {
    if (!this.isImportedMode) {
      throw new Error("Not in imported mode");
    }
    const result = await compileSession.run(editedDsl, async (guidance, xmlBase) => {
      // Make sure the backend holds this document (rejects if loading it failed)
      await this.backendSync;
      return compileImported(editedDsl, options(), guidance, xmlBase, this.decisions);
    });
    this.editedWords = new Set(result.editedWords);
    this.records = result.records ?? [];
    return result.xml;
  }
}

export const importedStore = new ImportedStore();
