import { describe, expect, it } from "vitest";
import { annotatedWords, createLemmaAnnotation, createNoteAnnotation, createSemanticAnnotation } from "./annotations";

describe("annotatedWords", () => {
    it("lists the words with annotations of their own other than lemmas", () => {
        const set = {
            version: "1.0",
            annotations: [
                createLemmaAnnotation(0, "ok", "xCC"),
                createNoteAnnotation({ type: "word", wordIndex: 5 }, "damaged"),
                createSemanticAnnotation({ type: "char", wordIndex: 2, charStart: 0, charEnd: 1 }, "person"),
                createNoteAnnotation({ type: "word", wordIndex: 5 }, "again"),
                // A span is about several words, not one of them
                createNoteAnnotation({ type: "span", startWord: 7, endWord: 9 }, "a phrase"),
            ],
        };
        expect(annotatedWords(set)).toEqual([2, 5]);
    });
});
