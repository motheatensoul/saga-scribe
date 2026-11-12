import { describe, expect, it } from "vitest";
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { openRecordRanges, setRecordMarks, wordRecordMarks } from "./records";
import { wordsToCheck } from "$lib/stores/imported.svelte";
import type { WordRecord } from "$lib/tauri";

const replaced: WordRecord = {
    key: 3,
    suggestion: "same",
    decision: null,
    word: 0,
    from: 3,
    to: 7,
    was: "bok",
    lemma: "bók",
    msa: "xNC",
};

describe("replaced words in the editor", () => {
    it("marks only the open ones, clamped to the text", () => {
        const answered = { ...replaced, key: 4, word: 1, decision: "same" as const };
        const past = { ...replaced, key: 5, word: 2, from: 8, to: 20 };
        expect(openRecordRanges([past, replaced, answered], 10)).toEqual([
            { from: 3, to: 7 },
            { from: 8, to: 10 },
        ]);
    });

    it("keeps the mark over the whole word as typing moves it", () => {
        const view = new EditorView({ state: EditorState.create({ doc: ".p bækr maþr", extensions: wordRecordMarks }) });
        view.dispatch({ effects: setRecordMarks.of(openRecordRanges([replaced], view.state.doc.length)) });
        view.dispatch({ changes: { from: 0, insert: "x " } });
        const marked = view.contentDOM.querySelector(".cm-word-record");
        expect(marked?.textContent).toBe("bækr");
        view.destroy();
    });
});

describe("words to check", () => {
    it("are the edited ones and the unanswered replaced ones", () => {
        const answered = { ...replaced, key: 4, word: 5, decision: "same" as const };
        const open = { ...replaced, key: 6, word: 7, suggestion: "different" as const };
        expect([...wordsToCheck([2, 5], [answered, open, { ...open, key: 8, word: null }])].sort()).toEqual([2, 7]);
    });
});
