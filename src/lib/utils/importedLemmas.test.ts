import { describe, expect, it } from "vitest";
import type { Segment } from "$lib/tauri";
import { importedLemmas, importedWords, menotaText } from "./importedLemmas";

let id = 0;
const word = (xml: string, attributes: Record<string, string> = {}): Segment => ({
    type: "word",
    id: id++,
    original_xml: xml,
    attributes,
    dsl_content: "",
    has_inline_lb: false,
});
const space: Segment = { type: "whitespace", id: id++, content: " " };

const menota = (facs: string, dipl: string, norm: string) =>
    `<me:facs>${facs}</me:facs><me:dipl>${dipl}</me:dipl><me:norm>${norm}</me:norm>`;

describe("menotaText", () => {
    it("reads the MENOTA level, else the plain element, without tags", () => {
        expect(menotaText("<me:dipl>k<lb/>onungr</me:dipl>", "dipl")).toBe("konungr");
        expect(menotaText("<norm>maðr</norm>", "norm")).toBe("maðr");
        expect(menotaText("<me:norm> </me:norm><norm>x</norm>", "norm")).toBe("x");
        expect(menotaText("<me:dipl>a</me:dipl>", "norm")).toBeUndefined();
    });
});

describe("importedWords", () => {
    it("numbers nested words in document order", () => {
        const segments = [
            word(`<w lemma="a">${menota("a", "a", "á")}</w>`, { lemma: "a" }),
            space,
            word(`<head><w lemma="b" me:msa="xNC">${menota("b", "b", "b")}</w> <w>${menota("c", "c", "c")}</w></head>`),
            word(`<w lemma="d" msa="xAJ">d</w>`, { lemma: "d", msa: "xAJ" }),
        ];
        expect(importedWords(segments)).toEqual([
            { lemma: "a", msa: undefined, facsimile: "a", diplomatic: "a", normalized: "á" },
            { lemma: "b", msa: "xNC", facsimile: "b", diplomatic: "b", normalized: "b" },
            { lemma: undefined, msa: undefined, facsimile: "c", diplomatic: "c", normalized: "c" },
            { lemma: "d", msa: "xAJ", facsimile: undefined, diplomatic: undefined, normalized: undefined },
        ]);
    });
});

describe("importedLemmas", () => {
    const segments = [
        word(`<w lemma="maðr" me:msa="xNC gM nS">${menota("maþr", "maðr", "maðr")}</w>`, { lemma: "maðr", "me:msa": "xNC gM nS" }),
        word(`<w lemma="maðr" me:msa="gM xNC nS">${menota("Maðr", "Maðr", "maðr")}</w>`, { lemma: "maðr", "me:msa": "gM xNC nS" }),
        word(`<w lemma="vera">${menota("var", "var", "")}</w>`, { lemma: "vera" }),
        word(`<supplied><w lemma="ok" me:msa="xCC">${menota("ok", "ok", "ok")}</w></supplied>`),
    ];

    it("confirms the lemmas that have an analysis, by word number", () => {
        expect(importedLemmas(segments, true).confirmations).toEqual({
            0: { lemma: "maðr", msa: "xNC gM nS" },
            1: { lemma: "maðr", msa: "gM xNC nS" },
            3: { lemma: "ok", msa: "xCC" },
        });
    });

    it("teaches each form once, keyed by its lowercased diplomatic form", () => {
        const { inflections } = importedLemmas(segments, true);
        expect(inflections.map(([form, f]) => `${form}: ${f.onp_id} ${f.analysis} ${f.part_of_speech}`)).toEqual([
            "maðr: imported:maðr xNC gM nS xNC",
            "ok: imported:ok xCC xCC",
        ]);
    });

    it("counts missing lemmas and normalized forms in MENOTA files only", () => {
        const missing = [word(`<w>${menota("a", "a", "")}</w>`), word(`<head><w>b</w></head>`)];
        expect(importedLemmas(missing, true)).toMatchObject({ missingLemmas: 2, missingNormalized: 2 });
        expect(importedLemmas(missing, false)).toMatchObject({ missingLemmas: 0, missingNormalized: 0 });
    });
});
