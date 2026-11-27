import {
  compileImported,
  setImportedDocument,
  type CompileImportedOptions,
  type Decision,
  type StagedDocument,
  type WordRecord,
} from "$lib/tauri";
import { compileSession } from "$lib/stores/compileSession";

/**
 * The imported document being edited. The document itself (the file and its
 * segments) is held by the backend; this is what the editor needs of it.
 */
class ImportedStore {
  isImportedMode = $state(false);
  /**
   * Where the document's file came from: imported ("file"), or made from the
   * template for a new document ("new"); both are edited as their XML
   */
  origin = $state<"file" | "new">("file");
  /**
   * Words to check since import: edited ones that kept the original's
   * lemma, analysis and normalized form, and replaced ones not yet answered
   * (an answered word is checked); updated by every compile
   */
  editedWords = $state<ReadonlySet<number>>(new Set());
  /** Words of the original replaced by others in the DSL, from the last compile */
  records = $state<WordRecord[]>([]);
  /** The DSL the records were compiled from (their positions are in it) */
  recordsDsl = $state("");
  /** The records by the new word's number, for the word panel */
  recordByWord = $derived(
    new Map(this.records.filter((r) => r.word !== null).map((r) => [r.word as number, r])),
  );
  /** Replaced words the editor hasn't answered for yet (the XML follows the suggestion) */
  openRecords = $derived(this.records.filter((r) => r.decision === null));
  /** The editor's answers for them, by the original word's key; saved with the manifest */
  decisions = $state<Record<number, Decision>>({});
  /** Called after an answer changes, to compile with it (set by the page) */
  onDecided: (() => void) | null = null;

  /**
   * Backend session updates, chained so they apply in call order: a quick
   * reset() + load() must never let the clear land after the load.
   */
  private backendSync: Promise<void> = Promise.resolve();

  private syncBackend(handle: number | null) {
    this.backendSync = this.backendSync
      .catch(() => {})
      .then(() => setImportedDocument(handle));
  }

  reset() {
    this.isImportedMode = false;
    this.editedWords = new Set();
    this.records = [];
    this.recordsDsl = "";
    this.decisions = {};
    compileSession.reset();
    this.syncBackend(null);
  }

  /** Edits `document`, which the backend staged when it was imported or opened. */
  load(document: StagedDocument, origin: "file" | "new" = "file") {
    this.isImportedMode = true;
    this.origin = origin;
    this.editedWords = new Set();
    this.records = [];
    this.recordsDsl = "";
    this.decisions = { ...document.decisions };
    compileSession.reset();
    this.syncBackend(document.handle);
  }

  /** Answers for the original word `key`, and compiles with the answer. */
  decide(key: number, decision: Decision) {
    this.decisions = { ...this.decisions, [key]: decision };
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
    const records = result.records ?? [];
    this.editedWords = wordsToCheck(result.editedWords, records);
    this.records = records;
    this.recordsDsl = editedDsl;
    return result.xml;
  }
}

/**
 * The words to check: those edited that kept the original's data (`edited`),
 * and replaced ones until answered.
 */
export function wordsToCheck(edited: readonly number[], records: readonly WordRecord[]): Set<number> {
  const words = new Set(edited);
  for (const record of records) {
    if (record.word === null) continue;
    if (record.decision === null) words.add(record.word);
    else words.delete(record.word);
  }
  return words;
}

/**
 * What saving or exporting tells the editor about replaced words still to be
 * decided, or null if there are none.
 */
export function openRecordsNotice(open: number): string | null {
  if (open === 0) return null;
  const words = open === 1 ? "1 replaced word is" : `${open} replaced words are`;
  return `${words} still to be decided; the XML has the suggested answers. Validate lists them, F8 goes to the next.`;
}

export const importedStore = new ImportedStore();
