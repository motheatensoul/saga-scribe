import { linter, type Action, type Diagnostic } from "@codemirror/lint";
import type { EditorView } from "@codemirror/view";
import type { Decision, WordRecord } from "$lib/tauri";

/**
 * Words of an imported file that the shorthand replaces or deletes, and
 * that would lose something (a lemma, an analysis, markup the shorthand
 * doesn't show): shown in the editor with a mark in the gutter until the
 * editor answers whether it's the same word (docs/DOCUMENT_MODEL.md,
 * "Replacing and deleting words"). Until then the file follows the
 * suggested answer.
 *
 * The answers are quick fixes on the mark (also from the keyboard: F8 goes
 * to the next mark, Ctrl+Shift+M lists them all with their actions).
 */

/** What the editor shows of a record, and what answering it does. */
export interface RecordSource {
    /** The records of the last compile */
    records: () => readonly WordRecord[];
    decide: (key: number, decision: Decision) => void;
    acceptSuggestions: (records: WordRecord[]) => void;
}

/** "lemma konungr, xNC gM nS cN" */
function wordData(record: WordRecord): string {
    const parts = [
        record.lemma ? `lemma ${record.lemma}` : "",
        record.msa ?? "",
        record.hidden.length > 0 ? `markup the shorthand doesn't show: ${record.hidden.join(", ")}` : "",
    ];
    return parts.filter(Boolean).join("; ");
}

/** The message of an open record. */
export function recordMessage(record: WordRecord): string {
    const data = wordData(record);
    if (record.kind === "removed") {
        return `Deleted word “${record.was}” (${data}). Restore it, or remove it with its data.`;
    }
    const kept = record.lemma ? "keeps its lemma and analysis" : "keeps its analysis";
    const now =
        record.suggestion === "same"
            ? "Treated as the same word, corrected, until you answer."
            : "Treated as a different word, without that data, until you answer.";
    return `Replaced word “${record.was}” (${data}). The same word, corrected, ${kept}; a different word loses them and its normalized form. ${now}`;
}

/**
 * Where to put a deleted word back: at `at` (the end of the word before it,
 * or the start), set apart from its neighbours.
 */
export function restoreChange(doc: string, at: number, was: string): { from: number; insert: string } {
    at = Math.min(at, doc.length);
    if (at > 0 && !/\s/.test(doc[at - 1])) return { from: at, insert: ` ${was}` };
    return { from: at, insert: at < doc.length && !/\s/.test(doc[at]) ? `${was} ` : was };
}

function actions(record: WordRecord, source: RecordSource, open: WordRecord[]): Action[] {
    const suggested = (decision: Decision) => (record.suggestion === decision ? " (suggested)" : "");
    const list: Action[] =
        record.kind === "removed"
            ? [
                  {
                      name: "Restore",
                      apply: (view: EditorView) => {
                          const change = restoreChange(view.state.doc.toString(), record.from, record.was);
                          view.dispatch({ changes: change });
                      },
                  },
                  { name: "Remove", apply: () => source.decide(record.key, "removed") },
              ]
            : [
                  { name: `Same word${suggested("same")}`, apply: () => source.decide(record.key, "same") },
                  { name: `Different word${suggested("different")}`, apply: () => source.decide(record.key, "different") },
              ];
    // Replaced words, many at once (typos fixed while transcribing)
    const replaced = open.filter((r) => r.kind === "replaced");
    if (record.kind === "replaced" && replaced.length > 1) {
        list.push({
            name: `Accept all ${replaced.length} suggestions`,
            apply: () => source.acceptSuggestions(replaced),
        });
    }
    return list;
}

/** The message of an answered record, which can still be changed. */
function answeredMessage(record: WordRecord): string {
    const data = wordData(record);
    switch (record.decision) {
        case "same":
            return `Replaced word “${record.was}” (${data}), answered as the same word: it keeps its data.`;
        case "different":
            return `Replaced word “${record.was}” (${data}), answered as a different word: its data is removed.`;
        default:
            return `Deleted word “${record.was}” (${data}), removed with its data.`;
    }
}

/** The answers that change an answered record. */
function changes(record: WordRecord, source: RecordSource): Action[] {
    if (record.kind === "removed") {
        return actions(record, source, []).filter((a) => a.name === "Restore");
    }
    const other: Decision = record.decision === "same" ? "different" : "same";
    const name = other === "same" ? "Same word after all" : "Different word after all";
    return [{ name, apply: () => source.decide(record.key, other) }];
}

/**
 * The records as diagnostics on `doc` (positions clamped to it): open ones
 * with a mark, answered ones as hints (a faint underline, no mark) that let
 * the answer be changed.
 */
export function recordDiagnostics(records: readonly WordRecord[], doc: string, source: RecordSource): Diagnostic[] {
    const open = records.filter((r) => r.decision === null);
    return records.map((record) => {
        const from = Math.min(record.from, doc.length);
        const to = Math.min(Math.max(record.to, from), doc.length);
        if (record.decision !== null) {
            return {
                from,
                to,
                severity: "hint",
                source: "Replaced and deleted words",
                message: answeredMessage(record),
                actions: changes(record, source),
            };
        }
        return {
            from,
            to,
            // A corrected word is ordinary; another word or a deleted one
            // loses data unless answered
            severity: record.kind === "replaced" && record.suggestion === "same" ? "info" : "warning",
            source: "Replaced and deleted words",
            message: recordMessage(record),
            actions: actions(record, source, open),
        };
    });
}

/** Gutter marks only for what needs attention: answered records are hints without one. */
export function withoutHints(diagnostics: readonly Diagnostic[]): Diagnostic[] {
    return diagnostics.filter((d) => d.severity !== "hint");
}

/** The lint source showing `source`'s open records; rerun it with `forceLinting` after a compile. */
export function wordRecords(source: RecordSource) {
    return linter((view) => recordDiagnostics(source.records(), view.state.doc.toString(), source), { delay: 0 });
}
