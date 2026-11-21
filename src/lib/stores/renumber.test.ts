import { describe, expect, it } from "vitest";
import { renumberParking, renumberSet, renumberTarget, renumberWord, type Parked, type Renumbering } from "$lib/stores/renumber";
import { createEmptyAnnotationSet, createLemmaAnnotation } from "$lib/types/annotations";

// Previous words 0..3: "x" inserted before 1, word 2 deleted
const MAP: Renumbering = [0, 2, null, 3];

describe("renumbering", () => {
    it("moves words, drops deleted ones, leaves numbers past the words alone", () => {
        expect(renumberWord(MAP, 1)).toBe(2);
        expect(renumberWord(MAP, 2)).toBeUndefined();
        expect(renumberWord(MAP, 9)).toBe(9);
    });

    it("keeps what survives of a span", () => {
        expect(renumberTarget({ type: "span", startWord: 1, endWord: 3 }, MAP)).toEqual({
            type: "span",
            startWord: 2,
            endWord: 3,
        });
        expect(renumberTarget({ type: "span", startWord: 2, endWord: 2 }, MAP)).toBeNull();
        expect(renumberTarget({ type: "char", wordIndex: 3, charStart: 0, charEnd: 1 }, MAP)).toEqual({
            type: "char",
            wordIndex: 3,
            charStart: 0,
            charEnd: 1,
        });
    });

    it("moves lemmas with their IDs and drops those of deleted words", () => {
        const set = createEmptyAnnotationSet();
        set.annotations = [createLemmaAnnotation(1, "bee", "xNC"), createLemmaAnnotation(2, "gone", "xNC")];
        const moved = renumberSet(set, MAP);
        expect(moved.annotations).toHaveLength(1);
        expect(moved.annotations[0].id).toBe("lemma-2");
        expect(moved.annotations[0].target).toEqual({ type: "word", wordIndex: 2 });
    });

    it("keeps a gone word's lemma aside until the word comes back", () => {
        // ".p a be c": cut "be" (word 1), then paste it after "c"
        const parked: Parked = new Map();
        const set = createEmptyAnnotationSet();
        set.annotations = [createLemmaAnnotation(1, "bé", "xNC"), createLemmaAnnotation(2, "sé", "xAV")];
        const cut = renumberParking(set, [0, null, 1], parked, [{ word: 1, ticket: "2:0" }]);
        expect(cut.annotations.map((a) => a.id)).toEqual(["lemma-1"]);
        expect(parked.size).toBe(1);
        const pasted = renumberParking(cut, [0, 1], parked, [], [{ ticket: "2:0", word: 2 }]);
        expect(pasted.annotations.map((a) => [a.id, a.target])).toEqual([
            ["lemma-1", { type: "word", wordIndex: 1 }],
            ["lemma-2", { type: "word", wordIndex: 2 }],
        ]);
        expect(pasted.annotations[1].value).toEqual(set.annotations[0].value);
        expect(parked.size).toBe(0);
    });
});
