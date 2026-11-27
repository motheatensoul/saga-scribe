import { afterEach, describe, expect, it } from "vitest";
import { EditorState } from "@codemirror/state";
import { ensureSyntaxTree } from "@codemirror/language";
import { get } from "svelte/store";
import { teiDsl } from "$lib/parser/highlighter";
import { abbreviationStore } from "$lib/stores/abbreviations";
import { abbreviationDiagnostics, abbreviations } from "./abbreviations";

function state(doc: string): EditorState {
    const s = EditorState.create({ doc, extensions: [teiDsl] });
    ensureSyntaxTree(s, s.doc.length, 5000);
    return s;
}

afterEach(() => abbreviationStore.clear());

describe("abbreviations and the store", () => {
    it("finds the long and the short forms", () => {
        expect(abbreviations(state("{:kbar:=konun}gs {:et:} hæn{=n}aR"))).toEqual([
            { from: 0, to: 14, mark: ":kbar:", expansion: "konun" },
            { from: 17, to: 23, mark: ":et:", expansion: null },
            { from: 27, to: 31, mark: "", expansion: "n" },
        ]);
    });

    it("warns about a short form the store has no expansion for, and adds a long one", () => {
        abbreviationStore.load({ ":et:": ["ok"] });
        const s = state("{:et:} {:kbar:} {:kbar:=konun}gs {=n}");
        const diagnostics = abbreviationDiagnostics(s);
        expect(diagnostics.map((d) => [d.from, d.severity])).toEqual([
            [7, "warning"],
            [16, "info"],
        ]);
        diagnostics[1].actions![0].apply(null as never, 16, 32);
        expect(get(abbreviationStore)[":kbar:"]).toEqual(["konun"]);
        expect(abbreviationStore.changedByEditor()).toBe(true);
        expect(JSON.parse(abbreviationStore.defaultsJson()!)).toEqual({ ":et:": "ok", ":kbar:": "konun" });
        expect(abbreviationDiagnostics(s)).toEqual([]);
    });
});
