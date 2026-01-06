import { writable } from 'svelte/store';

export interface Template {
    id: string;
    name: string;
    description: string;
    header: string;
    footer: string;
    wordWrap: boolean;
    autoLineNumbers: boolean;
    multiLevel: boolean;
}

function createTemplateStore() {
    const { subscribe, set, update } = writable<{
        templates: Template[];
        active: Template | null;
    }>({
        templates: [],
        active: null,
    });

    return {
        subscribe,
        setTemplates: (templates: Template[]) =>
            update((state) => ({ ...state, templates })),
        setActive: (template: Template) =>
            update((state) => ({ ...state, active: template })),
        reset: () => set({ templates: [], active: null }),
    };
}

export const templateStore = createTemplateStore();
