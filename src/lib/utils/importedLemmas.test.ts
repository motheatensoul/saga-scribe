import { describe, expect, it } from "vitest";
import type { ImportedWord } from "$lib/tauri";
import { importedLemmas } from "./importedLemmas";

const word = (lemma: string | undefined, msa: string | undefined, facs: string, dipl: string, norm?: string): ImportedWord => ({
    lemma,
    msa,
    facsimile: facs,
    diplomatic: dipl,
    normalized: norm,
});

describe("importedLemmas", () => {
    const words = [
        word("maðr", "xNC gM nS", "maþr", "maðr", "maðr"),
        word("maðr", "gM xNC nS", "Maðr", "Maðr", "maðr"),
        word("vera", undefined, "var", "var"),
        word("ok", "xCC", "ok", "ok", "ok"),
    ];

    it("confirms the lemmas that have an analysis, by word number", () => {
        expect(importedLemmas(words, true).confirmations).toEqual({
            0: { lemma: "maðr", msa: "xNC gM nS" },
            1: { lemma: "maðr", msa: "gM xNC nS" },
            3: { lemma: "ok", msa: "xCC" },
        });
    });

    it("teaches each form once, keyed by its lowercased diplomatic form", () => {
        const { inflections } = importedLemmas(words, true);
        expect(inflections.map(([form, f]) => `${form}: ${f.onp_id} ${f.analysis} ${f.part_of_speech}`)).toEqual([
            "maðr: imported:maðr xNC gM nS xNC",
            "ok: imported:ok xCC xCC",
        ]);
    });

    it("counts missing lemmas and normalized forms in MENOTA files only", () => {
        const missing = [word(undefined, undefined, "a", "a"), { facsimile: "b" }];
        expect(importedLemmas(missing, true)).toMatchObject({ missingLemmas: 2, missingNormalized: 2 });
        expect(importedLemmas(missing, false)).toMatchObject({ missingLemmas: 0, missingNormalized: 0 });
    });
});
