import { afterEach, describe, expect, it } from "vitest";
import { EditorSelection, EditorState } from "@codemirror/state";
import { EditorView, runScopeHandlers } from "@codemirror/view";
import { ensureSyntaxTree } from "@codemirror/language";
import { teiDsl } from "$lib/parser/highlighter";
import { entityGlyphs, entityGlyphState, glyphAt, setEntityGlyphs } from "./entityGlyphs";

const known = { thorn: "þ", kbar: "ꝁ" };
let view: EditorView | undefined;

function open(doc: string, cursor: number): EditorView {
    const state = EditorState.create({
        doc,
        selection: EditorSelection.cursor(cursor),
        extensions: [teiDsl, entityGlyphState, entityGlyphs],
    });
    ensureSyntaxTree(state, state.doc.length, 5000);
    view = new EditorView({ state, parent: document.body });
    view.dispatch({ effects: setEntityGlyphs.of(known) });
    return view;
}

function shown(v: EditorView): string[] {
    return [...v.dom.querySelectorAll(".cm-entity-glyph")].map((e) => e.textContent ?? "");
}

function press(v: EditorView, key: string) {
    runScopeHandlers(v, new KeyboardEvent("keydown", { key }), "editor");
}

afterEach(() => {
    view?.destroy();
    view = undefined;
});

describe("entities shown as characters", () => {
    it("shows known entities as glyphs, unknown ones and the one at the cursor as source", () => {
        const v = open(":thorn:at :kbar: :nope: x", 24);
        expect(shown(v)).toEqual(["þ", "ꝁ"]);
        // Touching an entity shows its source
        v.dispatch({ selection: { anchor: 16 } });
        expect(shown(v)).toEqual(["þ"]);
        // The document is unchanged: search and copy see the source
        expect(v.state.doc.toString()).toBe(":thorn:at :kbar: :nope: x");
    });

    it("finds the entity before or after a position", () => {
        const v = open("a:thorn:b", 0);
        expect(glyphAt(v.state, 8, true)).toMatchObject({ from: 1, to: 8, glyph: "þ" });
        expect(glyphAt(v.state, 1, false)).toMatchObject({ from: 1, to: 8 });
        expect(glyphAt(v.state, 7, true)).toBeUndefined();
    });

    it("deletes a whole entity with Backspace or Delete", () => {
        const v = open("a:thorn:b:kbar:", 8);
        press(v, "Backspace");
        expect(v.state.doc.toString()).toBe("ab:kbar:");
        expect(v.state.selection.main.head).toBe(1);
        v.dispatch({ selection: { anchor: 2 } });
        press(v, "Delete");
        expect(v.state.doc.toString()).toBe("ab");
    });
});
