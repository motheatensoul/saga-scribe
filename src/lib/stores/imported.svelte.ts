import { compileImported, type ImportedDocument, type Segment } from "$lib/tauri";

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
  segmentsJson = $state<string | null>(null);
  originalBodyXml = $state("");
  originalPreamble = $state("");
  originalPostamble = $state("");
  frontMatter = $state("");
  backMatter = $state("");
  isMenota = $state(false);

  reset() {
    this.isImportedMode = false;
    this.segments = [];
    this.spans = undefined;
    this.segmentsJson = null;
    this.originalBodyXml = "";
    this.originalPreamble = "";
    this.originalPostamble = "";
    this.frontMatter = "";
    this.backMatter = "";
    this.isMenota = false;
  }

  load(data: Partial<ImportedState>) {
    this.isImportedMode = true;
    if (data.segments) this.segments = data.segments;
    this.spans = data.spans ?? undefined;
    if (data.originalBodyXml) this.originalBodyXml = data.originalBodyXml;
    if (data.originalPreamble) this.originalPreamble = data.originalPreamble;
    if (data.originalPostamble) this.originalPostamble = data.originalPostamble;
    if (data.frontMatter) this.frontMatter = data.frontMatter;
    if (data.backMatter) this.backMatter = data.backMatter;
    if (data.isMenota !== undefined) this.isMenota = data.isMenota;
    this.segmentsJson = this.buildManifestJson();
  }

  /** Full manifest JSON, used both for compiling and for saving projects. */
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

    const manifestJson = this.segmentsJson ?? this.buildManifestJson();

    return compileImported(
      editedDsl,
      manifestJson,
      this.originalPreamble,
      this.originalPostamble,
      this.originalBodyXml,
      options,
    );
  }
}

export const importedStore = new ImportedStore();
