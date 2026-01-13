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
    builtinEntities: EntityMap;
    customEntities: EntityMap;
    entities: EntityMap; // Combined: { ...builtin, ...custom }
    baseMappings: Record<string, string>;  // Default diplomatic mappings from entity-base-letters.json
    customMappings: Record<string, string>; // User overrides
    loaded: boolean;
    error: string | null;
}

function createEntityStore() {
    const { subscribe, set, update } = writable<EntityStoreState>({
        builtinEntities: {},
        customEntities: {},
        entities: {},
        baseMappings: {},
        customMappings: {},
        loaded: false,
        error: null,
    });

    return {
        subscribe,
        setBuiltinEntities: (entities: EntityMap) =>
            update((state) => ({ 
                ...state, 
                builtinEntities: entities,
                entities: { ...entities, ...state.customEntities },
                loaded: true, 
                error: null 
            })),
        setCustomEntities: (customEntities: EntityMap) =>
            update((state) => ({
                ...state,
                customEntities,
                entities: { ...state.builtinEntities, ...customEntities }
            })),
        addCustomEntity: (name: string, entity: Entity) =>
            update((state) => {
                const newCustom = { ...state.customEntities, [name]: entity };
                return {
                    ...state,
                    customEntities: newCustom,
                    entities: { ...state.builtinEntities, ...newCustom }
                };
            }),
        removeCustomEntity: (name: string) =>
            update((state) => {
                const { [name]: _, ...rest } = state.customEntities;
                return {
                    ...state,
                    customEntities: rest,
                    entities: { ...state.builtinEntities, ...rest }
                };
            }),
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
        reset: () => set({ 
            builtinEntities: {}, 
            customEntities: {}, 
            entities: {}, 
            baseMappings: {}, 
            customMappings: {}, 
            loaded: false, 
            error: null 
        }),
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
