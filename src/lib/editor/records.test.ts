import { describe, expect, it, vi } from "vitest";
import { recordDiagnostics, recordMessage, restoreChange, withoutHints, type RecordSource } from "./records";
import type { WordRecord } from "$lib/tauri";

const replaced: WordRecord = {
    key: 3,
    kind: "replaced",
    suggestion: "same",
    decision: null,
    from: 3,
    to: 6,
    was: "ok",
    lemma: "ok",
    msa: "xCC",
    hidden: [],
};
const removed: WordRecord = { ...replaced, key: 7, kind: "removed", suggestion: "removed", from: 6, to: 6, was: "hæn", hidden: ["ex"] };

function source(records: WordRecord[]) {
    return {
        records: () => records,
        decide: vi.fn<RecordSource["decide"]>(),
        acceptSuggestions: vi.fn<RecordSource["acceptSuggestions"]>(),
    } satisfies RecordSource;
}

describe("word records in the editor", () => {
    it("shows open records with their answers, answered ones as hints", () => {
        const s = source([replaced, removed, { ...replaced, key: 9, decision: "different" }]);
        const diagnostics = recordDiagnostics(s.records(), ".p okk maþr", s);
        expect(diagnostics.map((d) => [d.from, d.to, d.severity])).toEqual([
            [3, 6, "info"],
            [6, 6, "warning"],
            [3, 6, "hint"],
        ]);
        expect(diagnostics[2].actions?.map((a) => a.name)).toEqual(["Same word after all"]);
        diagnostics[2].actions?.[0].apply(null as never, 3, 6);
        expect(s.decide).toHaveBeenCalledWith(9, "same");
        expect(withoutHints(diagnostics)).toHaveLength(2);
        expect(diagnostics[0].actions?.map((a) => a.name)).toEqual(["Same word (suggested)", "Different word"]);
        expect(diagnostics[1].actions?.map((a) => a.name)).toEqual(["Restore", "Remove"]);
        diagnostics[0].actions?.[1].apply(null as never, 3, 6);
        expect(s.decide).toHaveBeenCalledWith(3, "different");
    });

    it("offers accepting every suggestion when several words are open", () => {
        const other = { ...replaced, key: 4, suggestion: "different" as const };
        const s = source([replaced, other]);
        const [first] = recordDiagnostics(s.records(), ".p okk þá", s);
        const all = first.actions?.find((a) => a.name.startsWith("Accept all"));
        expect(all?.name).toBe("Accept all 2 suggestions");
        all?.apply(null as never, 0, 0);
        expect(s.acceptSuggestions).toHaveBeenCalledWith([replaced, other]);
    });

    it("keeps marks inside the text when it got shorter since the compile", () => {
        const s = source([{ ...replaced, from: 30, to: 34 }]);
        const [d] = recordDiagnostics(s.records(), ".p ok", s);
        expect([d.from, d.to]).toEqual([5, 5]);
    });

    it("says what a word would lose", () => {
        expect(recordMessage(removed)).toBe(
            "Deleted word “hæn” (lemma ok; xCC; markup the shorthand doesn't show: ex). Restore it, or remove it with its data.",
        );
        expect(recordMessage(replaced)).toContain("Treated as the same word, corrected");
    });

    it("puts a deleted word back apart from its neighbours", () => {
        expect(restoreChange(".p ok maþr", 5, "hæn")).toEqual({ from: 5, insert: " hæn" });
        expect(restoreChange("maþr", 0, "hæn")).toEqual({ from: 0, insert: "hæn " });
        expect(restoreChange("", 0, "hæn")).toEqual({ from: 0, insert: "hæn" });
    });
});
