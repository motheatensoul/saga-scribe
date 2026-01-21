import { writable } from "svelte/store";
import type { StylesheetEntry } from "$lib/tauri";

export const DEFAULT_STYLESHEET_ID = "default";

function createStylesheetStore() {
    const { subscribe, set, update } = writable<StylesheetEntry[]>([]);

    return {
        subscribe,
        setStylesheets: (stylesheets: StylesheetEntry[]) => set(stylesheets),
        addStylesheet: (stylesheet: StylesheetEntry) =>
            update((items) => [...items, stylesheet]),
        removeStylesheet: (id: string) =>
            update((items) => items.filter((item) => item.id !== id)),
    };
}

export const stylesheetStore = createStylesheetStore();
