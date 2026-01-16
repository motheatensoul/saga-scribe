<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { EditorView, keymap, lineNumbers, highlightActiveLineGutter, type Command, type KeyBinding } from '@codemirror/view';
    import { EditorState } from '@codemirror/state';
    import { defaultKeymap, history, historyKeymap, undo, redo } from '@codemirror/commands';
    import { search, searchKeymap, openSearchPanel, closeSearchPanel, searchPanelOpen } from '@codemirror/search';
    import { foldGutter, foldKeymap } from '@codemirror/language';
    import { editor } from '$lib/stores/editor';
    import { teiDsl, teiDslHighlighting, teiDslFolding } from '$lib/parser/highlighter';
    import { teiLinter } from '$lib/parser/linter';
    import { lintGutter } from "@codemirror/lint";
    import { daisyExtensions } from '$lib/editor/theme';

    let { onchange }: { onchange?: (content: string) => void } = $props();

    let container: HTMLDivElement;
    let view: EditorView;

    const undoCommand = undo as unknown as Command;
    const redoCommand = redo as unknown as Command;

    onMount(() => {
        const bindings = [
            ...defaultKeymap,
            ...historyKeymap,
            ...searchKeymap,
            ...foldKeymap,
        ] as unknown as KeyBinding[];

        const startState = EditorState.create({
            doc: $editor.content,
            extensions: [
                lineNumbers(),
                highlightActiveLineGutter(),
                foldGutter(),
                history(),
                keymap.of(bindings),
                search({ top: true }),
                teiDsl,
                teiDslHighlighting,
                teiDslFolding,
                teiLinter,
                lintGutter(),
                daisyExtensions,
                EditorView.updateListener.of((update) => {
                    if (update.docChanged) {
                        const content = update.state.doc.toString();
                        editor.setContent(content);
                        onchange?.(content);
                    }
                }),
            ],
        });

        view = new EditorView({
            state: startState,
            parent: container,
        });
    });

    onDestroy(() => {
        view?.destroy();
    });

    export function setContent(content: string) {
        if (view) {
            view.dispatch({
                changes: {
                    from: 0,
                    to: view.state.doc.length,
                    insert: content,
                },
            });
        }
    }

    export function insertText(text: string) {
        if (view) {
            const pos = view.state.selection.main.head;
            view.dispatch({
                changes: { from: pos, insert: text },
                selection: { anchor: pos + text.length },
            });
            view.focus();
        }
    }

    export function triggerUndo() {
        if (view) {
            undoCommand(view);
            view.focus();
        }
    }

    export function triggerRedo() {
        if (view) {
            redoCommand(view);
            view.focus();
        }
    }

    export function triggerSearch() {
        if (view) {
            // Check if search panel is already open
            if (searchPanelOpen(view.state)) {
                closeSearchPanel(view);
                view.focus();
            } else {
                openSearchPanel(view);
                // The search panel automatically focuses itself on open
            }
        }
    }
</script>

<div class="w-full h-full overflow-hidden flex-1" bind:this={container}></div>

<style>
    :global(.cm-editor) {
        height: 100%;
    }
</style>
