import { describe, expect, it } from "vitest";
import { wordNormalizedForm } from "./wordNorm";

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
