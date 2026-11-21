<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { EditorView, keymap, lineNumbers, highlightActiveLineGutter, type Command, type KeyBinding } from '@codemirror/view';
    import { Compartment, EditorState, type ChangeSet } from '@codemirror/state';
    import { defaultKeymap, history, historyKeymap, undo, redo } from '@codemirror/commands';
    import { search, searchKeymap, openSearchPanel, closeSearchPanel, searchPanelOpen } from '@codemirror/search';
    import { foldGutter, foldKeymap } from '@codemirror/language';
    import { editor } from '$lib/stores/editor';
    import { teiDsl, teiDslHighlighting, teiDslFolding } from '$lib/parser/highlighter';
    import { teiLinter } from '$lib/parser/linter';
    import { lintGutter } from "@codemirror/lint";
    import { openRecordRanges, setRecordMarks, wordRecordMarks } from '$lib/editor/records';
    import { importedStore } from '$lib/stores/imported.svelte';
    import { daisyExtensions } from '$lib/editor/theme';
    import { markEdit } from '$lib/editor/typing';
    import { entityGlyphs, entityGlyphState, setEntityGlyphs } from '$lib/editor/entityGlyphs';
    import { entityStore } from '$lib/stores/entities';

    let {
        onchange,
        onchanges,
        showGlyphs = true,
    }: {
        onchange?: (content: string) => void;
        /** Every edit's exact changes, before `onchange` */
        onchanges?: (changes: ChangeSet) => void;
        /** Known entities are shown as their characters (the source where the cursor is) */
        showGlyphs?: boolean;
    } = $props();

    let container: HTMLDivElement;
    let view: EditorView;

    /** Showing entities as characters, switched by `showGlyphs` */
    const glyphMode = new Compartment();

    const undoCommand = undo as unknown as Command;
    const redoCommand = redo as unknown as Command;

    onMount(() => {
        const bindings = [
            ...defaultKeymap,
            ...historyKeymap,
            // CodeMirror binds this on Linux and macOS only; in the editor it redoes
            // on every platform (outside it, the window binds it to lemma undo).
            { key: 'Mod-Shift-z', run: redoCommand, preventDefault: true },
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
                wordRecordMarks,
                entityGlyphState,
                glyphMode.of(showGlyphs ? entityGlyphs : []),
                lintGutter(),
                daisyExtensions,
                EditorView.updateListener.of((update) => {
                    if (update.docChanged) {
                        markEdit();
                        onchanges?.(update.changes);
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

    // Each compile brings the replaced words anew: mark them, if the text is
    // still what was compiled (otherwise the next compile, already on its
    // way, brings them; the marks move with the typing meanwhile)
    $effect(() => {
        const records = importedStore.records;
        const compiled = importedStore.recordsDsl;
        if (!view || view.state.doc.toString() !== compiled) return;
        view.dispatch({ effects: setRecordMarks.of(openRecordRanges(records, compiled.length)) });
    });

    // The characters of the known entities, custom ones included
    $effect(() => {
        const known: Record<string, string> = {};
        for (const [name, entity] of Object.entries($entityStore.entities)) {
            if (entity.char) known[name] = entity.char;
        }
        view?.dispatch({ effects: setEntityGlyphs.of(known) });
    });

    $effect(() => {
        const shown = showGlyphs;
        view?.dispatch({ effects: glyphMode.reconfigure(shown ? entityGlyphs : []) });
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

    /** Replaces `from..to` (UTF-16 offsets) with `text`, as an edit. */
    export function replaceRange(from: number, to: number, text: string) {
        view?.dispatch({ changes: { from, to, insert: text } });
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
