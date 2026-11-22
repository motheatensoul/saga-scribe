import { writable, get } from "svelte/store";

/**
 * The abbreviation store: for each abbreviation mark (as shorthand, e.g.
 * `:kbar:`), its expansions, the default first. `{mark}` in the shorthand
 * takes the default. Imported files fill it from their `<am>`/`<ex>`; it is
 * saved with the project (`abbreviations.json`).
 */
export type AbbreviationStore = Record<string, string[]>;

function createAbbreviationStore() {
    const { subscribe, set, update } = writable<AbbreviationStore>({});
    /** Whether the last change was the editor's (an abbreviation added), not a load */
    let byEditor = false;
    return {
        subscribe,
        /** Replaces the store (a document was imported or opened) */
        load: (store: AbbreviationStore | undefined | null) => {
            byEditor = false;
            set({ ...(store ?? {}) });
        },
        clear: () => {
            byEditor = false;
            set({});
        },
        /** Makes `expansion` the default of `mark` (keeping its other expansions) */
        setDefault: (mark: string, expansion: string) => {
            byEditor = true;
            update((store) => ({ ...store, [mark]: [expansion, ...(store[mark] ?? []).filter((e) => e !== expansion)] }));
        },
        changedByEditor: () => byEditor,
        /** Each mark's default expansion, as compiles take it (JSON) */
        defaultsJson: (): string | undefined => {
            const store = get({ subscribe });
            const defaults: Record<string, string> = {};
            for (const [mark, expansions] of Object.entries(store)) {
                if (expansions.length > 0) defaults[mark] = expansions[0];
            }
            return Object.keys(defaults).length > 0 ? JSON.stringify(defaults) : undefined;
        },
    };
}

export const abbreviationStore = createAbbreviationStore();
