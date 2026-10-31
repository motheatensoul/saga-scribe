import { linter, type Diagnostic } from "@codemirror/lint";
import { entityStore } from "$lib/stores/entities";
import { get } from "svelte/store";
import { checkDsl } from "$lib/tauri";
import { perfTime } from "$lib/perf";

// Debounce timer
let linterTimeout: ReturnType<typeof setTimeout> | null = null;
const DEBOUNCE_MS = 500;

/**
 * DSL-specific linter for the editor.
 *
 * Reports DSL syntax errors (from the backend parser, which keeps going after
 * an error so every one is shown) and unknown entities.
 * Schema validation (RelaxNG/XSD) is handled separately after compilation
 * and displayed in the XML Preview pane where line numbers are accurate.
 */
export const teiLinter = linter(async (view) => {
    // Clear any existing timeout to restart debounce
    if (linterTimeout) {
        clearTimeout(linterTimeout);
    }

    return new Promise<Diagnostic[]>((resolve) => {
        linterTimeout = setTimeout(async () => {
            const content = view.state.doc.toString();
            const diagnostics: Diagnostic[] = [];

            if (!content.trim()) {
                resolve([]);
                return;
            }

            // Syntax errors, at the construct that failed
            try {
                const found = await perfTime("lint.check_dsl", () => checkDsl(content), { dsl_chars: content.length });
                for (const d of found) {
                    diagnostics.push({ from: d.from, to: d.to, severity: "error", message: d.message });
                }
            } catch (e) {
                console.warn("DSL syntax check failed:", e);
            }

            // Entity Validation
            // Check for entity patterns :name: and verify they exist in loaded entities
            const entStore = get(entityStore);
            if (entStore.loaded) {
                const entityRegex = /:([a-zA-Z][a-zA-Z0-9]*):/g;
                let match;
                while ((match = entityRegex.exec(content)) !== null) {
                    const name = match[1];
                    // Start position in document
                    const from = match.index;
                    const to = from + match[0].length;

                    if (!entStore.entities[name]) {
                        diagnostics.push({
                            from,
                            to,
                            severity: "error",
                            message: `Unknown entity: :${name}:`
                        });
                    }
                }
            }

            resolve(diagnostics);
        }, DEBOUNCE_MS);
    });
});
