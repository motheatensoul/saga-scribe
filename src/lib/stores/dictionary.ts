import { writable, derived } from 'svelte/store';
import type { OnpEntry, InflectedForm } from '$lib/tauri';

// Re-export annotation store and related items for backward compatibility
export {
    annotationStore,
    annotationStore as sessionLemmaStore,  // Backward compat alias
    annotationHistory,
    annotationHistory as lemmatizationHistory,  // Backward compat alias
    canUndoAnnotation as canUndo,
    canRedoAnnotation as canRedo,
    isWordConfirmed,
    lemmaMappings,
    annotationCounts,
    totalAnnotations,
} from './annotations';

// Re-export types
export type { SessionLemmaMapping } from './annotations';

interface DictionaryStoreState {
    loaded: boolean;
    loading: boolean;
    headwordCount: number;
    error: string | null;
}

function createDictionaryStore() {
    const { subscribe, set, update } = writable<DictionaryStoreState>({
        loaded: false,
        loading: false,
        headwordCount: 0,
        error: null,
    });

    return {
        subscribe,
        setLoading: () =>
            update((state) => ({ ...state, loading: true, error: null })),
        setLoaded: (count: number) =>
            update((state) => ({
                ...state,
                loaded: true,
                loading: false,
                headwordCount: count,
                error: null,
            })),
        setError: (error: string) =>
            update((state) => ({
                ...state,
                loaded: false,
                loading: false,
                error,
            })),
        reset: () =>
            set({
                loaded: false,
                loading: false,
                headwordCount: 0,
                error: null,
            }),
    };
}

export const dictionaryStore = createDictionaryStore();

// Store for current lemma search results
interface LemmaSearchState {
    query: string;
    results: OnpEntry[];
    loading: boolean;
}

function createLemmaSearchStore() {
    const { subscribe, set, update } = writable<LemmaSearchState>({
        query: '',
        results: [],
        loading: false,
    });

    return {
        subscribe,
        setQuery: (query: string) =>
            update((state) => ({ ...state, query })),
        setResults: (results: OnpEntry[]) =>
            update((state) => ({ ...state, results, loading: false })),
        setLoading: () =>
            update((state) => ({ ...state, loading: true })),
        clear: () =>
            set({ query: '', results: [], loading: false }),
    };
}

export const lemmaSearchStore = createLemmaSearchStore();

// Store for user inflection mappings
interface InflectionStoreState {
    mappings: Record<string, InflectedForm[]>;
    loaded: boolean;
}

function createInflectionMappingStore() {
    const { subscribe, set, update } = writable<InflectionStoreState>({
        mappings: {},
        loaded: false,
    });

    return {
        subscribe,
        setMappings: (mappings: Record<string, InflectedForm[]>) =>
            update((state) => ({ ...state, mappings, loaded: true })),
        addMapping: (wordform: string, inflection: InflectedForm) =>
            update((state) => {
                const existing = state.mappings[wordform.toLowerCase()] || [];
                const alreadyExists = existing.some(
                    (f) => f.onp_id === inflection.onp_id && f.analysis === inflection.analysis
                );
                if (alreadyExists) return state;
                return {
                    ...state,
                    mappings: {
                        ...state.mappings,
                        [wordform.toLowerCase()]: [...existing, inflection],
                    },
                };
            }),
        removeMapping: (wordform: string, onpId: string, analysis: string) =>
            update((state) => {
                const existing = state.mappings[wordform.toLowerCase()] || [];
                const filtered = existing.filter(
                    (f) => !(f.onp_id === onpId && f.analysis === analysis)
                );
                const newMappings = { ...state.mappings };
                if (filtered.length === 0) {
                    delete newMappings[wordform.toLowerCase()];
                } else {
                    newMappings[wordform.toLowerCase()] = filtered;
                }
                return { ...state, mappings: newMappings };
            }),
        clear: () =>
            set({ mappings: {}, loaded: false }),
    };
}

export const inflectionStore = createInflectionMappingStore();

// Derived store: check if a wordform has known mappings
export const hasInflection = derived(inflectionStore, ($store) => {
    return (wordform: string): boolean => {
        return wordform.toLowerCase() in $store.mappings;
    };
});

// Derived store: get mappings for a wordform
export const getInflections = derived(inflectionStore, ($store) => {
    return (wordform: string): InflectedForm[] => {
        return $store.mappings[wordform.toLowerCase()] || [];
    };
});

// Session-based lemmatization is now handled by the annotation store
// See ./annotations.ts for the new implementation
