import { writable, derived } from 'svelte/store';
import type { SessionLemmaMapping } from './dictionary';

export interface LemmaAction {
    type: 'confirm' | 'unconfirm';
    wordIndex: number;
    mapping: SessionLemmaMapping | null;
    previousMapping: SessionLemmaMapping | null;
}

interface LemmatizationHistoryState {
    undoStack: LemmaAction[];
    redoStack: LemmaAction[];
    maxHistory: number;
}

const DEFAULT_MAX_HISTORY = 50;

function createLemmatizationHistoryStore() {
    const { subscribe, set, update } = writable<LemmatizationHistoryState>({
        undoStack: [],
        redoStack: [],
        maxHistory: DEFAULT_MAX_HISTORY,
    });

    return {
        subscribe,

        pushAction: (action: LemmaAction) =>
            update((state) => {
                const newUndoStack = [...state.undoStack, action];
                // Trim to max history
                if (newUndoStack.length > state.maxHistory) {
                    newUndoStack.shift();
                }
                return {
                    ...state,
                    undoStack: newUndoStack,
                    redoStack: [], // Clear redo stack on new action
                };
            }),

        undo: (): LemmaAction | null => {
            let action: LemmaAction | null = null;
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

        redo: (): LemmaAction | null => {
            let action: LemmaAction | null = null;
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
    };
}

export const lemmatizationHistory = createLemmatizationHistoryStore();

// Derived stores for UI state
export const canUndo = derived(lemmatizationHistory, ($history) => $history.undoStack.length > 0);
export const canRedo = derived(lemmatizationHistory, ($history) => $history.redoStack.length > 0);
