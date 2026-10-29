import type { ChangeSet } from "@codemirror/state";
import {
  compileImported,
  setImportedDocument,
  type ImportedDocument,
  type Segment,
} from "$lib/tauri";
import { EditHistory } from "$lib/editor/editHistory";

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

  /** Editor changes since the backend's last compile, so it can tell which word is which */
  private history = new EditHistory();
  /** Revision of the backend's last successful compile */
  private confirmedRevision = 0;
  /** Compiles run one at a time, so each knows its exact changes */
  private compiles: Promise<unknown> = Promise.resolve();

  /** Records an editor transaction's changes (called for every edit). */
  recordChanges(changes: ChangeSet) {
    if (this.isImportedMode) this.history.record(changes);
  }

  /** The backend holds a new document: no compile history yet. */
  private resetHistory() {
    this.history.reset();
    this.confirmedRevision = 0;
  }

  private syncBackend(document: Parameters<typeof setImportedDocument>[0]) {
    this.backendSync = this.backendSync
      .catch(() => {})
      .then(() => setImportedDocument(document));
  }

  reset() {
    this.isImportedMode = false;
    this.editedWords = new Set();
    this.resetHistory();
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
    this.resetHistory();
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

    // The changes up to this document are cut off now, while the editor holds it
    const request = this.history.begin(editedDsl.length);
    const run = async () => {
      // Make sure the backend holds this document (rejects if loading it failed)
      await this.backendSync;
      const changes = this.history.changesFor(request);
      const guidance = changes ? { baseRevision: this.confirmedRevision, changes } : undefined;
      const result = await compileImported(editedDsl, options, guidance);
      // A failed compile leaves its changes for the next request
      this.history.confirm(request);
      this.confirmedRevision = result.revision;
      this.editedWords = new Set(result.editedWords);
      return result.xml;
    };
    const compiled = this.compiles.then(run, run);
    this.compiles = compiled.catch(() => {});
    return compiled;
  }
}

export const importedStore = new ImportedStore();
