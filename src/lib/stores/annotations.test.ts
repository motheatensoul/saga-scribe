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
