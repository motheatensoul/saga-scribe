import {
  compileImported,
  setImportedDocument,
  type CompileImportedOptions,
  type ImportedDocument,
  type Segment,
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
    };
    return JSON.stringify(manifest);
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
      return compileImported(editedDsl, options(), guidance, xmlBase);
    });
    this.editedWords = new Set(result.editedWords);
    return result.xml;
  }
}

export const importedStore = new ImportedStore();
