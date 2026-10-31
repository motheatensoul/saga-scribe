import { describe, expect, it } from "vitest";
import { wordNormalizedForm, wordsWithDiplomaticForm } from "./wordNorm";

const word = (norm: string) =>
    `<w lemma="x"><choice><me:facs>f</me:facs><me:dipl>d</me:dipl><me:norm>${norm}</me:norm></choice></w>`;

describe("wordNormalizedForm", () => {
    const entities = { aelig: { unicode: "U+00E6", char: "æ", description: "", category: "" } };

    it("finds the n-th word's normalized form", () => {
        const xml = `<body><p>${word("ok")} <pc>,</pc> ${word("maðr")}</p></body>`;
        expect(wordNormalizedForm(xml, 0, entities)).toBe("ok");
        expect(wordNormalizedForm(xml, 1, entities)).toBe("maðr");
        expect(wordNormalizedForm(xml, 2, entities)).toBeNull();
    });

    it("drops tags and resolves entity references", () => {
        const xml = word(`h<lb n="2"/>&aelig;tta &amp; &#xF1;`);
        expect(wordNormalizedForm(xml, 0, entities)).toBe("hætta & ñ");
    });

    it("is null for a word without a normalized level", () => {
        expect(wordNormalizedForm("<w>ok</w>", 0, entities)).toBeNull();
    });
});

describe("wordsWithDiplomaticForm", () => {
    const tei = (body: string) =>
        `<TEI xmlns="http://www.tei-c.org/ns/1.0" xmlns:me="http://www.menota.org/ns/1.0"><text><body>${body}</body></text></TEI>`;
    const menota = (dipl: string) => `<w><choice><me:facs>${dipl}</me:facs><me:dipl>${dipl}</me:dipl><me:norm>n</me:norm></choice></w>`;

    it("finds words by their diplomatic form", () => {
        expect(wordsWithDiplomaticForm(tei(menota("ok") + menota("konungr") + menota("ok")), "ok")).toEqual([0, 2]);
    });

    it("reads a single-level word's text", () => {
        expect(wordsWithDiplomaticForm(tei("<w>ok</w> <w>sva</w><head><w> ok </w></head>"), "ok")).toEqual([0, 2]);
    });

    it("finds nothing in XML that doesn't parse", () => {
        expect(wordsWithDiplomaticForm("<w>ok", "ok")).toEqual([]);
    });
});
