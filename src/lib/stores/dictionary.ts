import { writable, derived } from 'svelte/store';
import type { OnpEntry, InflectedForm } from '$lib/tauri';
import { lemmatizationHistory } from './lemmatizationHistory';

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

// Session-based lemmatization store (keyed by word INDEX, not persisted)
// This tracks which specific word instances have been manually confirmed
export interface SessionLemmaMapping {
    lemma: string;
    msa: string;
    normalized?: string;
}

interface SessionLemmaStoreState {
    // Map from word index to confirmed lemma mapping
    mappings: Record<number, SessionLemmaMapping>;
}

function createSessionLemmaStore() {
    const { subscribe, set, update } = writable<SessionLemmaStoreState>({
        mappings: {},
    });

    return {
        subscribe,
        // Confirm a lemmatization for a specific word index
        confirm: (wordIndex: number, mapping: SessionLemmaMapping) =>
            update((state) => ({
                ...state,
                mappings: {
                    ...state.mappings,
                    [wordIndex]: mapping,
                },
            })),
        // Remove confirmation for a word index
        unconfirm: (wordIndex: number) =>
            update((state) => {
                const newMappings = { ...state.mappings };
                delete newMappings[wordIndex];
                return { ...state, mappings: newMappings };
            }),
        // Check if a word index has been confirmed
        isConfirmed: (wordIndex: number): boolean => {
            let result = false;
            subscribe((state) => {
                result = wordIndex in state.mappings;
            })();
            return result;
        },
        // Get mapping for a word index
        getMapping: (wordIndex: number): SessionLemmaMapping | undefined => {
            let result: SessionLemmaMapping | undefined;
            subscribe((state) => {
                result = state.mappings[wordIndex];
            })();
            return result;
        },
        // Clear all session mappings (e.g., when opening a new file)
        clear: () => set({ mappings: {} }),

        // History-aware confirm: records action for undo/redo
        confirmWithHistory: (wordIndex: number, mapping: SessionLemmaMapping) => {
            let previousMapping: SessionLemmaMapping | null = null;

            // Get previous state before updating
            update((state) => {
                previousMapping = state.mappings[wordIndex] || null;
                return {
                    ...state,
                    mappings: {
                        ...state.mappings,
                        [wordIndex]: mapping,
                    },
                };
            });

            // Push to history
            lemmatizationHistory.pushAction({
                type: 'confirm',
                wordIndex,
                mapping,
                previousMapping,
            });
        },

        // History-aware unconfirm: records action for undo/redo
        unconfirmWithHistory: (wordIndex: number) => {
            let previousMapping: SessionLemmaMapping | null = null;

            update((state) => {
                previousMapping = state.mappings[wordIndex] || null;
                if (!previousMapping) return state;

                const newMappings = { ...state.mappings };
                delete newMappings[wordIndex];
                return { ...state, mappings: newMappings };
            });

            if (previousMapping) {
                lemmatizationHistory.pushAction({
                    type: 'unconfirm',
                    wordIndex,
                    mapping: null,
                    previousMapping,
                });
            }
        },
    };
}

export const sessionLemmaStore = createSessionLemmaStore();

// Derived store: check if a word index has a confirmed lemmatization
export const isWordConfirmed = derived(sessionLemmaStore, ($store) => {
    return (wordIndex: number): boolean => {
        return wordIndex in $store.mappings;
    };
});
