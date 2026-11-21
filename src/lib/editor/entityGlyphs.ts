import { syntaxTree } from "@codemirror/language";
import { Prec, RangeSetBuilder, StateEffect, StateField, type EditorState, type Extension } from "@codemirror/state";
import {
    Decoration,
    EditorView,
    ViewPlugin,
    WidgetType,
    keymap,
    type DecorationSet,
    type ViewUpdate,
} from "@codemirror/view";

/**
 * Entities shown as their characters (TODO.md, "Entities shown as
 * characters"). A known `:name:` is drawn as its glyph, set in Junicode on a
 * faint background so it looks different from a character typed directly.
 * Where the text cursor (any selection) touches or enters it, its source is
 * shown again. The document itself is never changed: search and copy work on
 * the source. Only the visible ranges are decorated, so typing stays fast.
 * Unknown entities stay as source; the linter warns about them.
 */

/** Sets the characters of the known entities: name → character. */
export const setEntityGlyphs = StateEffect.define<Readonly<Record<string, string>>>();

const glyphs = StateField.define<Readonly<Record<string, string>>>({
    create: () => ({}),
    update(value, tr) {
        for (const effect of tr.effects) {
            if (effect.is(setEntityGlyphs)) value = effect.value;
        }
        return value;
    },
});

class GlyphWidget extends WidgetType {
    constructor(
        readonly glyph: string,
        readonly source: string,
    ) {
        super();
    }

    eq(other: GlyphWidget) {
        return other.glyph === this.glyph && other.source === this.source;
    }

    toDOM() {
        const span = document.createElement("span");
        span.className = "cm-entity-glyph";
        span.textContent = this.glyph;
        span.title = this.source;
        return span;
    }

    ignoreEvent() {
        return false;
    }
}

/** A known entity's range and character. */
interface Glyph {
    from: number;
    to: number;
    glyph: string;
}

/** The known entities overlapping `from`..`to`, in document order. */
function glyphsIn(state: EditorState, from: number, to: number): Glyph[] {
    const known = state.field(glyphs);
    const found: Glyph[] = [];
    syntaxTree(state).iterate({
        from,
        to,
        enter(node) {
            if (node.name !== "Entity") return;
            const glyph = known[state.doc.sliceString(node.from + 1, node.to - 1)];
            if (glyph) found.push({ from: node.from, to: node.to, glyph });
            return false;
        },
    });
    return found;
}

/** The known entity ending (`before`) or starting at `pos`, if any. */
export function glyphAt(state: EditorState, pos: number, before: boolean): Glyph | undefined {
    const from = before ? Math.max(0, pos - 1) : pos;
    return glyphsIn(state, from, Math.min(state.doc.length, from + 1)).find((g) =>
        before ? g.to === pos : g.from === pos,
    );
}

function decorate(view: EditorView): DecorationSet {
    const { state } = view;
    const builder = new RangeSetBuilder<Decoration>();
    const selection = state.selection.ranges;
    let end = -1;
    for (const { from, to } of view.visibleRanges) {
        for (const g of glyphsIn(state, from, to)) {
            // Visible ranges can share an entity
            if (g.from < end) continue;
            // The cursor touching or inside it shows the source
            if (selection.some((r) => r.from <= g.to && r.to >= g.from)) continue;
            const source = state.doc.sliceString(g.from, g.to);
            builder.add(g.from, g.to, Decoration.replace({ widget: new GlyphWidget(g.glyph, source) }));
            end = g.to;
        }
    }
    return builder.finish();
}

const glyphPlugin = ViewPlugin.fromClass(
    class {
        decorations: DecorationSet;

        constructor(view: EditorView) {
            this.decorations = decorate(view);
        }

        update(update: ViewUpdate) {
            if (
                update.docChanged ||
                update.viewportChanged ||
                update.selectionSet ||
                syntaxTree(update.startState) !== syntaxTree(update.state) ||
                update.startState.field(glyphs) !== update.state.field(glyphs)
            ) {
                this.decorations = decorate(update.view);
            }
        }
    },
    { decorations: (plugin) => plugin.decorations },
);

/** Backspace and Delete next to a known entity delete all of it. */
function deleteEntity(before: boolean) {
    return (view: EditorView): boolean => {
        const { state } = view;
        const range = state.selection.main;
        if (state.selection.ranges.length !== 1 || !range.empty) return false;
        const g = glyphAt(state, range.head, before);
        if (!g) return false;
        view.dispatch({
            changes: { from: g.from, to: g.to },
            selection: { anchor: g.from },
            scrollIntoView: true,
            userEvent: before ? "delete.backward" : "delete.forward",
        });
        return true;
    };
}

/** The characters' look: Junicode, sized to the monospace x-height, on a faint background. */
const glyphTheme = EditorView.baseTheme({
    ".cm-entity-glyph": {
        fontFamily: "Junicode, serif",
        fontWeight: "400",
        fontStyle: "normal",
        // Junicode's x-height is small next to monospace fonts (about 0.53 of the size)
        fontSizeAdjust: "0.53",
        backgroundColor: "color-mix(in oklch, currentColor 10%, transparent)",
        borderRadius: "2px",
    },
});

/** The extension that shows entities as characters; the known ones are set with {@link setEntityGlyphs}. */
export const entityGlyphs: Extension = [
    glyphPlugin,
    glyphTheme,
    Prec.high(
        keymap.of([
            { key: "Backspace", run: deleteEntity(true) },
            { key: "Delete", run: deleteEntity(false) },
        ]),
    ),
];

/** Keeps the known entities' characters, whether or not they are shown. */
export const entityGlyphState: Extension = glyphs;
