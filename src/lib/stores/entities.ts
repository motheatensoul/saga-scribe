import { writable, derived } from 'svelte/store';

export interface Entity {
    unicode: string;
    char: string;
    description: string;
    category: string;
}

export interface EntityMap {
    [name: string]: Entity;
}

interface EntityStoreState {
    entities: EntityMap;
    baseMappings: Record<string, string>;  // Default diplomatic mappings from entity-base-letters.json
    customMappings: Record<string, string>; // User overrides
    loaded: boolean;
    error: string | null;
}

function createEntityStore() {
    const { subscribe, set, update } = writable<EntityStoreState>({
        entities: {},
        baseMappings: {},
        customMappings: {},
        loaded: false,
        error: null,
    });

    return {
        subscribe,
        setEntities: (entities: EntityMap) =>
            update((state) => ({ ...state, entities, loaded: true, error: null })),
        setError: (error: string) =>
            update((state) => ({ ...state, error, loaded: false })),
        setBaseMappings: (mappings: Record<string, string>) =>
            update((state) => ({ ...state, baseMappings: mappings })),
        setCustomMappings: (mappings: Record<string, string>) =>
            update((state) => ({ ...state, customMappings: mappings })),
        setCustomMapping: (entity: string, translation: string) =>
            update((state) => ({
                ...state,
                customMappings: { ...state.customMappings, [entity]: translation },
            })),
        removeCustomMapping: (entity: string) =>
            update((state) => {
                const { [entity]: _, ...rest } = state.customMappings;
                return { ...state, customMappings: rest };
            }),
        reset: () => set({ entities: {}, baseMappings: {}, customMappings: {}, loaded: false, error: null }),
    };
}

export const entityStore = createEntityStore();

// Derived store for entity names (for autocomplete)
export const entityNames = derived(entityStore, ($store) =>
    Object.keys($store.entities).sort()
);

// Check if an entity exists
export function hasEntity(name: string): boolean {
    let exists = false;
    entityStore.subscribe((store) => {
        exists = name in store.entities;
    })();
    return exists;
}

// Get entity character for preview
export function getEntityChar(name: string): string | null {
    let char: string | null = null;
    entityStore.subscribe((store) => {
        char = store.entities[name]?.char ?? null;
    })();
    return char;
}
