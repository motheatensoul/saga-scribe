import { describe, expect, it } from "vitest";
import { nextMarkedWord } from "./wordReveal.svelte";

describe("the next marked word", () => {
    const marked = new Set([7, 2, 12]);

    it("follows the given word, wrapping around", () => {
        expect(nextMarkedWord(marked, -1, 1)).toBe(2);
        expect(nextMarkedWord(marked, 2, 1)).toBe(7);
        expect(nextMarkedWord(marked, 8, 1)).toBe(12);
        expect(nextMarkedWord(marked, 12, 1)).toBe(2);
    });

    it("precedes it backwards, wrapping around", () => {
        expect(nextMarkedWord(marked, -1, -1)).toBe(12);
        expect(nextMarkedWord(marked, 7, -1)).toBe(2);
        expect(nextMarkedWord(marked, 2, -1)).toBe(12);
    });

    it("is none without marks", () => {
        expect(nextMarkedWord([], 3, 1)).toBeNull();
    });
});
