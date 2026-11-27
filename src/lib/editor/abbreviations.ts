import { syntaxTree } from "@codemirror/language";
import type { EditorState } from "@codemirror/state";
import { hoverTooltip, type EditorView } from "@codemirror/view";
import type { Diagnostic } from "@codemirror/lint";
import { get } from "svelte/store";
import { abbreviationStore } from "$lib/stores/abbreviations";

/** An abbreviation in the shorthand: `{mark=expansion}`, or `{mark}` (expansion null) */
export interface Abbreviation {
    from: number;
    to: number;
    mark: string;
    expansion: string | null;
}

/** The first `=` of `text` outside brackets and escapes, as the parser finds it */
function topLevelEquals(text: string): number {
    let depth = 0;
    for (let i = 0; i < text.length; i++) {
        const c = text[i];
        if (c === "\\") i++;
        else if (c === "{" || c === "[" || c === "<") depth++;
        else if (c === "}" || c === "]" || c === ">") depth = Math.max(0, depth - 1);
        else if (c === "=" && depth === 0) return i;
    }
    return -1;
}

/** The abbreviations of the shorthand (top level, as the editor's grammar finds them) */
export function abbreviations(state: EditorState, from = 0, to = state.doc.length): Abbreviation[] {
    const found: Abbreviation[] = [];
    syntaxTree(state).iterate({
        from,
        to,
        enter(node) {
            if (node.name !== "Expansion") return;
            const inner = state.doc.sliceString(node.from + 1, node.to - 1);
            const equals = topLevelEquals(inner);
            found.push({
                from: node.from,
                to: node.to,
                mark: (equals < 0 ? inner : inner.slice(0, equals)).trim(),
                expansion: equals < 0 ? null : inner.slice(equals + 1),
            });
            return false;
        },
    });
    return found;
}

/**
 * What the abbreviation store says about the shorthand's abbreviations: a
 * `{mark}` whose mark isn't in it (a warning), and a `{mark=expansion}`
 * whose mark isn't (with an action that adds it).
 */
export function abbreviationDiagnostics(state: EditorState): Diagnostic[] {
    const store = get(abbreviationStore);
    const diagnostics: Diagnostic[] = [];
    for (const a of abbreviations(state)) {
        const known = (store[a.mark] ?? []).length > 0;
        if (a.expansion === null && !known) {
            diagnostics.push({
                from: a.from,
                to: a.to,
                severity: "warning",
                message: `{${a.mark}} has no expansion in the abbreviation store: write it once as {${a.mark}=…} and add it to the store from there`,
            });
        } else if (a.expansion !== null && a.mark !== "" && !known) {
            const expansion = a.expansion;
            diagnostics.push({
                from: a.from,
                to: a.to,
                severity: "info",
                message: `${a.mark} isn't in the abbreviation store yet`,
                actions: [
                    {
                        name: `Add "${expansion}" for ${a.mark} to the store`,
                        apply: () => abbreviationStore.setDefault(a.mark, expansion),
                    },
                ],
            });
        }
    }
    return diagnostics;
}

/** A short form's expansion when the pointer rests on it */
export const abbreviationHints = hoverTooltip((view: EditorView, pos: number) => {
    const hit = abbreviations(view.state, pos, pos).find((a) => a.from <= pos && pos <= a.to);
    if (!hit || hit.expansion !== null) return null;
    const expansions = get(abbreviationStore)[hit.mark] ?? [];
    return {
        pos: hit.from,
        end: hit.to,
        above: true,
        create: () => {
            const dom = document.createElement("div");
            dom.className = "cm-abbreviation-hint";
            dom.textContent = expansions.length
                ? `${hit.mark} = ${expansions[0]}` + (expansions.length > 1 ? ` (also: ${expansions.slice(1).join(", ")})` : "")
                : `${hit.mark}: not in the abbreviation store`;
            return { dom };
        },
    };
});
