// Validation store for sharing errors between components
import { writable, derived } from "svelte/store";
import type { ValidationResult, SchemaInfo } from "../tauri";
import { listSchemas } from "../tauri";

export interface ValidationState {
    lastResult: ValidationResult | null;
    availableSchemas: SchemaInfo[];
    loaded: boolean;
    error: string | null;
    selectedSchemaId: string;
    isValidating: boolean;
}

function createValidationStore() {
    const { subscribe, set, update } = writable<ValidationState>({
        lastResult: null,
        availableSchemas: [],
        loaded: false,
        error: null,
        selectedSchemaId: "tei-p5",
        isValidating: false,
    });

    return {
        subscribe,
        setResult: (result: ValidationResult | null) => 
            update(state => ({ ...state, lastResult: result, isValidating: false, error: null })),
        setSchemas: (schemas: SchemaInfo[]) =>
            update(state => ({ ...state, availableSchemas: schemas, loaded: true, error: null })),
        setError: (error: string) =>
            update(state => ({ ...state, error, loaded: false, isValidating: false })),
        clear: () => update(state => ({ ...state, lastResult: null, isValidating: false, error: null })),
        loadSchemas: async () => {
            try {
                const schemas = await listSchemas();
                update(state => ({ ...state, availableSchemas: schemas, loaded: true }));
            } catch (e) {
                update(state => ({ ...state, error: String(e) }));
            }
        },
        selectSchema: (schemaId: string) =>
            update((s) => ({ ...s, selectedSchemaId: schemaId })),
        startValidation: () =>
            update((s) => ({ ...s, isValidating: true, error: null })),
        setResultKeepSchemas: (result: ValidationResult) =>
            update((s) => ({
                ...s,
                lastResult: result,
                isValidating: false,
                error: null,
            })),
    };
}

export const validationStore = createValidationStore();

export const validationResult = derived(
  validationStore,
  ($state) => $state.lastResult
);

export const isValidating = derived(
  validationStore,
  ($state) => $state.isValidating
);

export const validationError = derived(
  validationStore,
  ($state) => $state.error
);

export const validationCounts = derived(validationStore, ($state) => ({
  errors: $state.lastResult?.errorCount ?? 0,
  warnings: $state.lastResult?.warningCount ?? 0,
}));

export const availableSchemas = derived(
  validationStore,
  ($state) => $state.availableSchemas
);

export const selectedSchemaId = derived(
  validationStore,
  ($state) => $state.selectedSchemaId
);
