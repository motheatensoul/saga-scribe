import { writable, get } from 'svelte/store';
import { loadSettings as loadSettingsFromBackend, saveSettings as saveSettingsToBackend, type Settings as BackendSettings } from '../tauri';

export interface Settings {
    fontSize: number;
    theme: 'light' | 'dark';
    autoPreview: boolean;
    previewDelay: number;
    activeTemplateId: string | null;
}

const defaultSettings: Settings = {
    fontSize: 14,
    theme: 'light',
    autoPreview: true,
    previewDelay: 300,
    activeTemplateId: null,
};

let saveTimeout: ReturnType<typeof setTimeout> | null = null;

function createSettingsStore() {
    const { subscribe, set, update } = writable<Settings>(defaultSettings);

    const debouncedSave = () => {
        if (saveTimeout) {
            clearTimeout(saveTimeout);
        }
        saveTimeout = setTimeout(async () => {
            const current = get({ subscribe });
            try {
                await saveSettingsToBackend(current as BackendSettings);
            } catch (e) {
                console.error('Failed to save settings:', e);
            }
        }, 500);
    };

    return {
        subscribe,
        load: async () => {
            try {
                const loaded = await loadSettingsFromBackend();
                set({
                    fontSize: loaded.fontSize,
                    theme: (loaded.theme === 'dark' ? 'dark' : 'light') as 'light' | 'dark',
                    autoPreview: loaded.autoPreview,
                    previewDelay: loaded.previewDelay,
                    activeTemplateId: loaded.activeTemplateId,
                });
            } catch (e) {
                console.error('Failed to load settings, using defaults:', e);
                set(defaultSettings);
            }
        },
        update: (partial: Partial<Settings>) => {
            update((state) => ({ ...state, ...partial }));
            debouncedSave();
        },
        reset: () => {
            set(defaultSettings);
            debouncedSave();
        },
    };
}

export const settings = createSettingsStore();
