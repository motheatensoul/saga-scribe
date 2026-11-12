import { StateEffect, StateField, type Extension } from "@codemirror/state";
import { Decoration, EditorView, type DecorationSet } from "@codemirror/view";
import type { WordRecord } from "$lib/tauri";

/**
 * Words of an imported file that the shorthand replaces with another word,
 * and that would lose their lemma and analysis (docs/DOCUMENT_MODEL.md,
 * "Replacing and deleting words"). The editor only marks them while they are
 * open; the question is answered in the word panel, opened from the word in
 * the rendered text. Until then the file follows the suggested answer.
 */

/** Replaces the marked words: ranges of the current document (UTF-16). */
export const setRecordMarks = StateEffect.define<{ from: number; to: number }[]>();

const mark = Decoration.mark({ class: "cm-word-record" });

/** The ranges of the open records, clamped to a document of `length`. */
export function openRecordRanges(records: readonly WordRecord[], length: number): { from: number; to: number }[] {
    return records
        .filter((r) => r.decision === null)
        .map((r) => ({ from: Math.min(r.from, length), to: Math.min(Math.max(r.to, r.from), length) }))
        .filter((r) => r.from < r.to)
        .sort((a, b) => a.from - b.from);
}

const recordMarks = StateField.define<DecorationSet>({
    create: () => Decoration.none,
    update(marks, tr) {
        // Typing moves the marks along until the next compile replaces them
        marks = marks.map(tr.changes);
        for (const effect of tr.effects) {
            if (effect.is(setRecordMarks)) {
                marks = Decoration.set(effect.value.map((r) => mark.range(r.from, r.to)));
            }
        }
        return marks;
    },
    provide: (field) => EditorView.decorations.from(field),
});

/** The marks of open records, set with {@link setRecordMarks} after each compile. */
export const wordRecordMarks: Extension = [
    recordMarks,
    EditorView.baseTheme({
        // The same wavy line as the word in the rendered text
        ".cm-word-record": {
            textDecoration: "underline wavy",
            textDecorationColor: "color-mix(in oklch, currentColor 60%, transparent)",
            textUnderlineOffset: "0.25em",
        },
    }),
];
