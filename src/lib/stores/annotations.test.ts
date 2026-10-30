import { describe, expect, it } from "vitest";
import { get } from "svelte/store";
import { annotationHistory, annotationStore, lemmaMappings } from "$lib/stores/annotations";

describe("removing a lemma from an imported word", () => {
    it("shows no lemma but sends the removal, and undo brings the lemma back", () => {
        annotationStore.loadLegacyConfirmations({ 3: { lemma: "ok", msa: "xCC" } });
        annotationStore.unconfirmWithHistory(3, true, true);

        expect(get(lemmaMappings).mappings[3]).toBeUndefined();
        expect(annotationStore.getLemmaWrites()[3]).toEqual({ lemma: "", msa: "", normalized: undefined });

        // Undo (as the page does): restore the previous annotation
        const action = annotationHistory.undo();
        expect(action?.type).toBe("update");
        annotationStore.add(action!.previousAnnotation!, false);
        expect(get(lemmaMappings).mappings[3]?.lemma).toBe("ok");
    });

    it("removes the lemma outright on native documents", () => {
        annotationStore.loadLegacyConfirmations({ 3: { lemma: "ok", msa: "xCC" } });
        annotationStore.unconfirmWithHistory(3, true, false);
        expect(annotationStore.getLemmaWrites()[3]).toBeUndefined();
    });
});

describe("a word's normalized form", () => {
    it("can be set without a lemma, and isn't shown as one", () => {
        annotationStore.clear();
        annotationStore.setNormalized(2, "maðr");

        expect(get(lemmaMappings).mappings[2]).toBeUndefined();
        expect(annotationStore.isLemmaConfirmed(2)).toBe(false);
        expect(annotationStore.getLemmaWrites()[2]).toEqual({ lemma: "", msa: "", normalized: "maðr" });

        // Back to the generated one: nothing left to send
        annotationStore.setNormalized(2, undefined);
        expect(annotationStore.getLemmaWrites()[2]).toBeUndefined();
    });

    it("keeps the lemma when changed, and stays when the lemma is removed", () => {
        annotationStore.loadLegacyConfirmations({ 4: { lemma: "maðr", msa: "xNC" } });
        annotationStore.setNormalized(4, "manni");
        expect(annotationStore.getLemmaWrites()[4]).toEqual({ lemma: "maðr", msa: "xNC", normalized: "manni" });

        annotationStore.unconfirmWithHistory(4, true, false);
        expect(get(lemmaMappings).mappings[4]).toBeUndefined();
        expect(annotationStore.getLemmaWrites()[4]).toEqual({ lemma: "", msa: "", normalized: "manni" });
    });
});
