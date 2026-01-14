import { compileImported, type Segment } from "$lib/tauri";

export interface ImportedState {
  isImportedMode: boolean;
  segments: Segment[];
  originalBodyXml: string;
  originalPreamble: string;
  originalPostamble: string;
  frontMatter: string;
  backMatter: string;
}

class ImportedStore {
  isImportedMode = $state(false);
  segments = $state<Segment[]>([]);
  originalBodyXml = $state("");
  originalPreamble = $state("");
  originalPostamble = $state("");
  frontMatter = $state("");
  backMatter = $state("");

  reset() {
    this.isImportedMode = false;
    this.segments = [];
    this.originalBodyXml = "";
    this.originalPreamble = "";
    this.originalPostamble = "";
    this.frontMatter = "";
    this.backMatter = "";
  }

  load(data: Partial<ImportedState>) {
    this.isImportedMode = true;
    if (data.segments) this.segments = data.segments;
    if (data.originalBodyXml) this.originalBodyXml = data.originalBodyXml;
    if (data.originalPreamble) this.originalPreamble = data.originalPreamble;
    if (data.originalPostamble) this.originalPostamble = data.originalPostamble;
    if (data.frontMatter) this.frontMatter = data.frontMatter;
    if (data.backMatter) this.backMatter = data.backMatter;
  }

  async compile(editedDsl: string, templateHeader: string, templateFooter: string): Promise<string> {
    if (!this.isImportedMode) {
      throw new Error("Not in imported mode");
    }

    return compileImported(
      editedDsl,
      JSON.stringify(this.segments),
      this.originalBodyXml,
      this.originalPreamble || null,
      this.originalPostamble || null,
      templateHeader,
      templateFooter,
      {
        regenerateHeader: true,
        entitiesJson: undefined,
      }
    );
  }
}

export const importedStore = new ImportedStore();
