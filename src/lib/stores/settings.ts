import { writable } from 'svelte/store';

export interface Settings {
    fontSize: number;
    theme: 'light' | 'dark';
    autoPreview: boolean;
    previewDelay: number;
}

const defaultSettings: Settings = {
    fontSize: 14,
    theme: 'light',
    autoPreview: true,
    previewDelay: 300,
};

function createSettingsStore() {
    const { subscribe, set, update } = writable<Settings>(defaultSettings);

    return {
        subscribe,
        update: (partial: Partial<Settings>) =>
            update((state) => ({ ...state, ...partial })),
        reset: () => set(defaultSettings),
    };
}

export const settings = createSettingsStore();
