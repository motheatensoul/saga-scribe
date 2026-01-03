import { writable, derived } from 'svelte/store';

export interface EditorState {
    content: string;
    filePath: string | null;
    isDirty: boolean;
}

function createEditorStore() {
    const { subscribe, set, update } = writable<EditorState>({
        content: '',
        filePath: null,
        isDirty: false,
    });

    return {
        subscribe,
        setContent: (content: string) =>
            update((state) => ({ ...state, content, isDirty: true })),
        setFile: (filePath: string, content: string) =>
            set({ content, filePath, isDirty: false }),
        markClean: () => update((state) => ({ ...state, isDirty: false })),
        reset: () => set({ content: '', filePath: null, isDirty: false }),
    };
}

export const editor = createEditorStore();

export const fileName = derived(editor, ($editor) => {
    if (!$editor.filePath) return 'Untitled';
    return $editor.filePath.split('/').pop() ?? 'Untitled';
});
