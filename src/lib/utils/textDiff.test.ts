import { describe, expect, it } from "vitest";
import { changedRange } from "$lib/utils/textDiff";

const apply = (before: string, after: string) => {
    const change = changedRange(before, after);
    return change ? before.slice(0, change.from) + change.insert + before.slice(change.to) : before;
};

describe("changedRange", () => {
    it("returns null for equal strings", () => {
        expect(changedRange("abc", "abc")).toBeNull();
    });

    it("finds a single-character edit", () => {
        expect(changedRange("geck ok", "gekk ok")).toEqual({ from: 2, to: 3, insert: "k" });
    });

    it("handles pure insertions and deletions", () => {
        expect(changedRange("ab", "aXb")).toEqual({ from: 1, to: 1, insert: "X" });
        expect(changedRange("aXb", "ab")).toEqual({ from: 1, to: 2, insert: "" });
    });

    it("always reconstructs the target", () => {
        const cases: Array<[string, string]> = [
            ["", "new"],
            ["old", ""],
            ["aaaa", "aa"],
            ["abab", "ab"],
            ["þat er orð", "þat var orð"],
            ["𝔞𝔟", "𝔞𝔠𝔟"],
        ];
        for (const [before, after] of cases) {
            expect(apply(before, after)).toBe(after);
        }
    });
});
