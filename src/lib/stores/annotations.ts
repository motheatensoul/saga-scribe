/**
 * Annotation store - manages all annotations (lemmas, notes, semantic, etc.)
 * This replaces the old sessionLemmaStore with a more flexible system.
 */
import { writable, derived } from "svelte/store";
import type {
    Annotation,
    AnnotationSet,
    AnnotationType,
    AnnotationTarget,
    AnnotationValue,
    AnnotationMetadata,
} from "$lib/types/annotations";
import { renumberAnnotation, renumberSet, type Renumbering } from "./renumber";
import {
    createEmptyAnnotationSet,
    createLemmaAnnotation,
    getLemmaWrites,
    isRemovedLemma,
    getAnnotationsForWord,
    getAnnotationsByType,
    getLemmaMap,
    targetIncludesWord,
    getPrimaryWordIndex,
} from "$lib/types/annotations";

// ============================================================================
// Annotation History (for undo/redo)
// ============================================================================

export interface AnnotationAction {
    type: "add" | "remove" | "update";
    annotation: Annotation;
    previousAnnotation?: Annotation | null;
}

interface AnnotationHistoryState {
    undoStack: AnnotationAction[];
    redoStack: AnnotationAction[];
    maxHistory: number;
}

const DEFAULT_MAX_HISTORY = 50;

function createAnnotationHistoryStore() {
    const { subscribe, set, update } = writable<AnnotationHistoryState>({
        undoStack: [],
        redoStack: [],
        maxHistory: DEFAULT_MAX_HISTORY,
    });

    return {
        subscribe,

        pushAction: (action: AnnotationAction) =>
            update((state) => {
                const newUndoStack = [...state.undoStack, action];
                if (newUndoStack.length > state.maxHistory) {
                    newUndoStack.shift();
                }
                return {
                    ...state,
                    undoStack: newUndoStack,
                    redoStack: [], // Clear redo stack on new action
                };
            }),

        undo: (): AnnotationAction | null => {
            let action: AnnotationAction | null = null;
            update((state) => {
                if (state.undoStack.length === 0) return state;

                const newUndoStack = [...state.undoStack];
                action = newUndoStack.pop() || null;

                if (action) {
                    return {
                        ...state,
                        undoStack: newUndoStack,
                        redoStack: [...state.redoStack, action],
                    };
                }
                return state;
            });
            return action;
        },

        redo: (): AnnotationAction | null => {
            let action: AnnotationAction | null = null;
            update((state) => {
                if (state.redoStack.length === 0) return state;

                const newRedoStack = [...state.redoStack];
                action = newRedoStack.pop() || null;

                if (action) {
                    return {
                        ...state,
                        redoStack: newRedoStack,
                        undoStack: [...state.undoStack, action],
                    };
                }
                return state;
            });
            return action;
        },

        clear: () =>
            set({
                undoStack: [],
                redoStack: [],
                maxHistory: DEFAULT_MAX_HISTORY,
            }),

        setMaxHistory: (max: number) =>
            update((state) => ({
                ...state,
                maxHistory: max,
                undoStack: state.undoStack.slice(-max),
            })),

        /** Moves every action to its words' new numbers; drops those whose word is gone. */
        renumber: (map: Renumbering) =>
            update((state) => {
                const move = (actions: AnnotationAction[]) =>
                    actions.flatMap((action) => {
                        const annotation = renumberAnnotation(action.annotation, map);
                        if (!annotation) return [];
                        const previousAnnotation = action.previousAnnotation
                            ? renumberAnnotation(action.previousAnnotation, map)
                            : action.previousAnnotation;
                        return [{ ...action, annotation, previousAnnotation }];
                    });
                return { ...state, undoStack: move(state.undoStack), redoStack: move(state.redoStack) };
            }),
    };
}

export const annotationHistory = createAnnotationHistoryStore();

export const canUndoAnnotation = derived(
    annotationHistory,
    ($history) => $history.undoStack.length > 0,
);

export const canRedoAnnotation = derived(
    annotationHistory,
    ($history) => $history.redoStack.length > 0,
);

// ============================================================================
// Main Annotation Store
// ============================================================================

interface AnnotationStoreState {
    set: AnnotationSet;
}

function createAnnotationStore() {
    const { subscribe, set, update } = writable<AnnotationStoreState>({
        set: createEmptyAnnotationSet(),
    });

    /**
     * Add an annotation (with optional history tracking)
     */
    function addAnnotation(annotation: Annotation, recordHistory = true): void {
        update((state) => {
            // Check for existing annotation with same ID and replace it
            const existingIndex = state.set.annotations.findIndex(
                (a) => a.id === annotation.id,
            );

            let previousAnnotation: Annotation | null = null;
            const newAnnotations = [...state.set.annotations];

            if (existingIndex >= 0) {
                previousAnnotation = newAnnotations[existingIndex];
                newAnnotations[existingIndex] = annotation;
            } else {
                newAnnotations.push(annotation);
            }

            if (recordHistory) {
                annotationHistory.pushAction({
                    type: existingIndex >= 0 ? "update" : "add",
                    annotation,
                    previousAnnotation,
                });
            }

            return {
                ...state,
                set: {
                    ...state.set,
                    annotations: newAnnotations,
                },
            };
        });
    }

    /**
     * Remove an annotation by ID (with optional history tracking)
     */
    function removeAnnotation(id: string, recordHistory = true): void {
        update((state) => {
            const existingIndex = state.set.annotations.findIndex(
                (a) => a.id === id,
            );
            if (existingIndex < 0) return state;

            const previousAnnotation = state.set.annotations[existingIndex];
            const newAnnotations = state.set.annotations.filter(
                (a) => a.id !== id,
            );

            if (recordHistory) {
                annotationHistory.pushAction({
                    type: "remove",
                    annotation: previousAnnotation,
                    previousAnnotation,
                });
            }

            return {
                ...state,
                set: {
                    ...state.set,
                    annotations: newAnnotations,
                },
            };
        });
    }

    /**
     * Get an annotation by ID
     */
    function getAnnotation(id: string): Annotation | undefined {
        let result: Annotation | undefined;
        subscribe((state) => {
            result = state.set.annotations.find((a) => a.id === id);
        })();
        return result;
    }

    /**
     * Get all annotations for a specific word index
     */
    function getForWord(wordIndex: number): Annotation[] {
        let result: Annotation[] = [];
        subscribe((state) => {
            result = getAnnotationsForWord(state.set, wordIndex);
        })();
        return result;
    }

    /**
     * Get all annotations of a specific type
     */
    function getByType(type: AnnotationType): Annotation[] {
        let result: Annotation[] = [];
        subscribe((state) => {
            result = getAnnotationsByType(state.set, type);
        })();
        return result;
    }

    /**
     * Check if a word has any annotation of a specific type
     */
    function hasAnnotationType(
        wordIndex: number,
        type: AnnotationType,
    ): boolean {
        let result = false;
        subscribe((state) => {
            result = state.set.annotations.some(
                (a) =>
                    a.type === type && targetIncludesWord(a.target, wordIndex),
            );
        })();
        return result;
    }

    /**
     * Load annotations from an AnnotationSet (e.g., from file)
     */
    function loadSet(annotationSet: AnnotationSet): void {
        set({ set: annotationSet });
        annotationHistory.clear();
    }

    /**
     * Get the current annotation set (for serialization)
     */
    function getSet(): AnnotationSet {
        let result: AnnotationSet = createEmptyAnnotationSet();
        subscribe((state) => {
            result = state.set;
        })();
        return result;
    }

    /**
     * Clear all annotations
     */
    function clear(): void {
        set({ set: createEmptyAnnotationSet() });
        annotationHistory.clear();
    }

    // ========================================================================
    // Backward Compatibility: Lemma-specific methods
    // These maintain API compatibility with the old sessionLemmaStore
    // ========================================================================

    /**
     * Confirm a lemma for a word (backward compatible with sessionLemmaStore)
     */
    function confirmLemma(
        wordIndex: number,
        mapping: { lemma: string; msa: string; normalized?: string },
        recordHistory = true,
    ): void {
        const annotation = createLemmaAnnotation(
            wordIndex,
            mapping.lemma,
            mapping.msa,
            mapping.normalized,
        );
        addAnnotation(annotation, recordHistory);
    }

    /**
     * Remove a word's lemma. On an imported file (`keepRemoval`) the removal
     * is kept as an empty lemma, so that it is written into the file. The
     * word's normalized form, if set, stays.
     */
    function unconfirmLemma(wordIndex: number, recordHistory = true, keepRemoval = false): void {
        setLemmaValue(wordIndex, "", "", getLemmaMapping(wordIndex)?.normalized, recordHistory, keepRemoval);
    }

    /**
     * Set a word's normalized form (`undefined`: the generated one, or on an
     * imported file the file's), keeping its lemma, if any.
     */
    function setNormalized(wordIndex: number, normalized: string | undefined, keepRemoval = false): void {
        const current = getLemmaMapping(wordIndex);
        setLemmaValue(wordIndex, current?.lemma ?? "", current?.msa ?? "", normalized, true, keepRemoval);
    }

    /** An empty lemma with no normalized form is kept only as a removal (`keepRemoval`). */
    function setLemmaValue(
        wordIndex: number,
        lemma: string,
        msa: string,
        normalized: string | undefined,
        recordHistory: boolean,
        keepRemoval: boolean,
    ): void {
        if (lemma || normalized || keepRemoval) {
            addAnnotation(createLemmaAnnotation(wordIndex, lemma, msa, normalized), recordHistory);
        } else {
            removeAnnotation(`lemma-${wordIndex}`, recordHistory);
        }
    }

    /** Lemmas to write into an imported file (see `getLemmaWrites`). */
    function getLemmaWritesNow(): ReturnType<typeof getLemmaWrites> {
        let result: ReturnType<typeof getLemmaWrites> = {};
        subscribe((state) => {
            result = getLemmaWrites(state.set);
        })();
        return result;
    }

    /**
     * Check if a word has a confirmed lemma
     */
    function isLemmaConfirmed(wordIndex: number): boolean {
        return !!getLemmaMapping(wordIndex)?.lemma;
    }

    /**
     * Get lemma mapping for a word (backward compatible)
     */
    function getLemmaMapping(
        wordIndex: number,
    ): { lemma: string; msa: string; normalized?: string } | undefined {
        let result:
            | { lemma: string; msa: string; normalized?: string }
            | undefined;
        subscribe((state) => {
            const ann = state.set.annotations.find(
                (a) =>
                    a.type === "lemma" &&
                    a.target.type === "word" &&
                    a.target.wordIndex === wordIndex,
            );
            if (ann && ann.value.kind === "lemma") {
                result = {
                    lemma: ann.value.lemma,
                    msa: ann.value.msa,
                    normalized: ann.value.normalized,
                };
            }
        })();
        return result;
    }

    /**
     * Get all lemma mappings as a record (for backward compatibility with project save)
     */
    function getLemmaMappings(): Record<
        number,
        { lemma: string; msa: string; normalized?: string }
    > {
        let result: Record<
            number,
            { lemma: string; msa: string; normalized?: string }
        > = {};
        subscribe((state) => {
            const map = getLemmaMap(state.set);
            for (const [index, info] of map.entries()) {
                result[index] = info;
            }
        })();
        return result;
    }

    /**
     * Load lemma confirmations from the old format (backward compat)
     */
    function loadLegacyConfirmations(
        confirmations: Record<
            number | string,
            { lemma: string; msa: string; normalized?: string }
        >,
    ): void {
        const annotations: Annotation[] = [];
        for (const [indexStr, mapping] of Object.entries(confirmations)) {
            const index =
                typeof indexStr === "number"
                    ? indexStr
                    : parseInt(indexStr, 10);
            annotations.push(
                createLemmaAnnotation(
                    index,
                    mapping.lemma,
                    mapping.msa,
                    mapping.normalized,
                ),
            );
        }
        set({
            set: {
                version: "1.0",
                annotations,
            },
        });
        annotationHistory.clear();
    }

    return {
        subscribe,

        // General annotation methods
        add: addAnnotation,
        remove: removeAnnotation,
        get: getAnnotation,
        getForWord,
        getByType,
        hasAnnotationType,
        loadSet,
        getSet,
        getLemmaWrites: getLemmaWritesNow,
        clear,
        /** Moves every annotation (and its undo history) to its word's new number. */
        renumber: (map: Renumbering) => {
            update((state) => ({ set: renumberSet(state.set, map) }));
            annotationHistory.renumber(map);
        },

        // Backward-compatible lemma methods
        confirmLemma,
        unconfirmLemma,
        setNormalized,
        isLemmaConfirmed,
        getLemmaMapping,
        getLemmaMappings,
        loadLegacyConfirmations,

        // Legacy aliases for old API
        confirm: (
            wordIndex: number,
            mapping: { lemma: string; msa: string; normalized?: string },
        ) => confirmLemma(wordIndex, mapping, false),
        confirmWithHistory: confirmLemma,
        unconfirm: (wordIndex: number) => unconfirmLemma(wordIndex, false),
        unconfirmWithHistory: unconfirmLemma,
        isConfirmed: isLemmaConfirmed,
        getMapping: getLemmaMapping,
    };
}

export const annotationStore = createAnnotationStore();

// ============================================================================
// Derived Stores (for backward compatibility and convenience)
// ============================================================================

/**
 * Check if a word index has a confirmed lemmatization (backward compat)
 */
export const isWordConfirmed = derived(annotationStore, ($store) => {
    return (wordIndex: number): boolean => {
        return $store.set.annotations.some(
            (a) =>
                a.type === "lemma" &&
                a.target.type === "word" &&
                a.target.wordIndex === wordIndex &&
                !isRemovedLemma(a),
        );
    };
});

/**
 * Get all annotations for a word index
 */
export const annotationsForWord = derived(annotationStore, ($store) => {
    return (wordIndex: number): Annotation[] => {
        return getAnnotationsForWord($store.set, wordIndex);
    };
});

/**
 * The annotation types on each word, by word number: one lookup per word
 * for the rendered text, which asks for every word it shows (filtering all
 * annotations per word took hundreds of milliseconds on a lemmatized
 * manuscript, and froze typing whenever the text was redrawn).
 */
export const annotationTypesByWord = derived(annotationStore, ($store) => {
    const types = new Map<number, Set<AnnotationType>>();
    const add = (word: number, type: AnnotationType) => {
        let set = types.get(word);
        if (!set) types.set(word, (set = new Set()));
        set.add(type);
    };
    for (const annotation of $store.set.annotations) {
        if (isRemovedLemma(annotation)) continue;
        const { target } = annotation;
        if (target.type === "span") {
            for (let word = target.startWord; word <= target.endWord; word++) add(word, annotation.type);
        } else {
            add(target.wordIndex, annotation.type);
        }
    }
    return types;
});

/**
 * Get annotation counts by type
 */
export const annotationCounts = derived(annotationStore, ($store) => {
    const counts: Record<AnnotationType, number> = {
        lemma: 0,
        semantic: 0,
        note: 0,
        paleographic: 0,
        syntax: 0,
        reference: 0,
        custom: 0,
    };
    for (const ann of $store.set.annotations) {
        if (isRemovedLemma(ann)) continue;
        counts[ann.type]++;
    }
    return counts;
});

/**
 * Total annotation count
 */
export const totalAnnotations = derived(
    annotationStore,
    ($store) => $store.set.annotations.filter((a) => !isRemovedLemma(a)).length,
);

/**
 * Backward-compatible: mappings record (legacy format)
 * This is used by components that expect the old sessionLemmaStore format
 */
export const lemmaMappings = derived(annotationStore, ($store) => ({
    // Removed lemmas (see createRemovedLemma) count as none
    mappings: Object.fromEntries(getLemmaMap($store.set)) as Record<
        number,
        { lemma: string; msa: string; normalized?: string }
    >,
}));

// ============================================================================
// Re-export type helpers
// ============================================================================

/**
 * Backward-compatible type alias for lemma mapping
 * @deprecated Use Annotation with type="lemma" instead
 */
export interface SessionLemmaMapping {
    lemma: string;
    msa: string;
    normalized?: string;
}

export type {
    Annotation,
    AnnotationSet,
    AnnotationType,
    AnnotationTarget,
    AnnotationValue,
    AnnotationMetadata,
};

export {
    createEmptyAnnotationSet,
    createLemmaAnnotation,
    getAnnotationsForWord,
    getAnnotationsByType,
    getLemmaMap,
    targetIncludesWord,
    getPrimaryWordIndex,
};
