import { describe, expect, it } from "vitest";
import { parseXmlAsync } from "$lib/rendered/document";
import { syntheticDocument, testEntities } from "$lib/rendered/fixtures";

describe("parseXmlAsync", () => {
    it("splits pages at page breaks", async () => {
        const doc = await parseXmlAsync(syntheticDocument(4), testEntities);
        expect(doc.pages.map((p) => p.pageNumber)).toEqual(["1r", "2r", "3r", "4r"]);
    });

    it("renders words inside <add> once, as words (not all three levels)", async () => {
        const doc = await parseXmlAsync(syntheticDocument(1), testEntities);
        const tokens = doc.pages[0].tokens;
        const open = tokens.findIndex((t) => t.displayText === "\\");
        expect(tokens[open + 1]).toMatchObject({ type: "word", displayText: "a" });
        expect(tokens.filter((t) => t.type === "text").every((t) => !/\n|\s{2,}/.test(t.displayText))).toBe(true);
    });

    it("shows the sic reading of a word-level choice and numbers words in document order", async () => {
        const xml = syntheticDocument(2);
        const doc = await parseXmlAsync(xml, testEntities);
        const words = doc.pages.flatMap((p) => p.tokens).filter((t) => t.type === "word");
        expect(words.some((t) => t.displayText === "hemt")).toBe(true);
        expect(words.some((t) => t.displayText === "beiddi")).toBe(false);
        // The hidden corr reading still takes an index, so indices match <w> elements
        const wElements = (xml.match(/<w[\s>]/g) ?? []).length;
        expect(Math.max(...words.map((t) => t.wordIndex ?? -1)) + 1).toBe(wElements);
    });

    it("resolves entities", async () => {
        const doc = await parseXmlAsync(syntheticDocument(1), testEntities);
        expect(doc.pages[0].tokens.some((t) => t.displayText === "ſꝥ")).toBe(true);
    });
});
