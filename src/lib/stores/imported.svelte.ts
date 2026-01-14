import { compileImported, type Segment } from "$lib/tauri";

export interface ImportedState {
  isImportedMode: boolean;
  segments: Segment[];
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
  originalBodyXml = $state("");
  originalPreamble = $state("");
  originalPostamble = $state("");
  frontMatter = $state("");
  backMatter = $state("");
  isMenota = $state(false);

  reset() {
    this.isImportedMode = false;
    this.segments = [];
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
    if (data.originalBodyXml) this.originalBodyXml = data.originalBodyXml;
    if (data.originalPreamble) this.originalPreamble = data.originalPreamble;
    if (data.originalPostamble) this.originalPostamble = data.originalPostamble;
    if (data.frontMatter) this.frontMatter = data.frontMatter;
    if (data.backMatter) this.backMatter = data.backMatter;
    if (data.isMenota !== undefined) this.isMenota = data.isMenota;
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

    return compileImported(
      editedDsl,
      JSON.stringify(this.segments),
      this.originalPreamble,
      this.originalPostamble,
      options,
    );
  }
}

export const importedStore = new ImportedStore();
