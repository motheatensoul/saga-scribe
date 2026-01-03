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

function createEntityStore() {
    const { subscribe, set, update } = writable<{
        entities: EntityMap;
        loaded: boolean;
        error: string | null;
    }>({
        entities: {},
        loaded: false,
        error: null,
    });

    return {
        subscribe,
        setEntities: (entities: EntityMap) =>
            set({ entities, loaded: true, error: null }),
        setError: (error: string) =>
            update((state) => ({ ...state, error, loaded: false })),
        reset: () => set({ entities: {}, loaded: false, error: null }),
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
