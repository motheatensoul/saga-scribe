import {
  compileImported,
  setImportedDocument,
  type ImportedDocument,
  type Segment,
} from "$lib/tauri";

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
    this.segments = [];
    this.spans = undefined;
    this.originalBodyXml = "";
    this.originalPreamble = "";
    this.originalPostamble = "";
    this.frontMatter = "";
    this.backMatter = "";
    this.isMenota = false;
    this.syncBackend(null);
  }

  load(data: Partial<ImportedState>) {
    this.isImportedMode = true;
    this.editedWords = new Set();
    if (data.segments) this.segments = data.segments;
    this.spans = data.spans ?? undefined;
    if (data.originalBodyXml) this.originalBodyXml = data.originalBodyXml;
    if (data.originalPreamble) this.originalPreamble = data.originalPreamble;
    if (data.originalPostamble) this.originalPostamble = data.originalPostamble;
    if (data.frontMatter) this.frontMatter = data.frontMatter;
    if (data.backMatter) this.backMatter = data.backMatter;
    if (data.isMenota !== undefined) this.isMenota = data.isMenota;
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
      spans: this.spans,
    };
    return JSON.stringify(manifest);
  }

  async compile(
    editedDsl: string,
    options?: {
      entitiesJson?: string;
      normalizerJson?: string;
      entityMappingsJson?: string;
      customMappings?: Record<string, string>;
    },
  ): Promise<string> {
    if (!this.isImportedMode) {
      throw new Error("Not in imported mode");
    }

    // Make sure the backend holds this document (rejects if loading it failed)
    await this.backendSync;
    const result = await compileImported(editedDsl, options);
    this.editedWords = new Set(result.editedWords);
    return result.xml;
  }
}

export const importedStore = new ImportedStore();
