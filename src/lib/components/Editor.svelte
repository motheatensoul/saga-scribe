<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { EditorView, keymap, lineNumbers, highlightActiveLineGutter } from '@codemirror/view';
    import { EditorState } from '@codemirror/state';
    import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
    import { editor } from '$lib/stores/editor';
    import { teiDslLanguage, teiDslHighlighting } from '$lib/parser/highlighter';

    let { onchange }: { onchange?: (content: string) => void } = $props();

    let container: HTMLDivElement;
    let view: EditorView;

    onMount(() => {
        const startState = EditorState.create({
            doc: $editor.content,
            extensions: [
                lineNumbers(),
                highlightActiveLineGutter(),
                history(),
                keymap.of([...defaultKeymap, ...historyKeymap]),
                teiDslLanguage,
                teiDslHighlighting,
                EditorView.updateListener.of((update) => {
                    if (update.docChanged) {
                        const content = update.state.doc.toString();
                        editor.setContent(content);
                        onchange?.(content);
                    }
                }),
                EditorView.theme({
                    '&': { height: '100%' },
                    '.cm-scroller': { overflow: 'auto' },
                    '.cm-content': { fontFamily: 'monospace' },
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
</script>

<div class="editor-container" bind:this={container}></div>

<style>
    .editor-container {
        width: 100%;
        height: 100%;
        overflow: hidden;
    }

    .editor-container :global(.cm-editor) {
        height: 100%;
    }
</style>
