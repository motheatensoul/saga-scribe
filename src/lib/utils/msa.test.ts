import { describe, expect, it } from "vitest";
import { analysisKey, sameAnalysis } from "$lib/utils/msa";

describe("analyses", () => {
    it("are the same with their tags in any order or spacing", () => {
        expect(sameAnalysis("xNC gM nS cN sI", "xNC nS gM  sI cN")).toBe(true);
        expect(sameAnalysis("xNC gM nS", "xNC gF nS")).toBe(false);
        expect(analysisKey(" cN xNC ")).toBe("cN xNC");
    });
});
