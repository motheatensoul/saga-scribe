<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { EditorView } from "@codemirror/view";
    import { EditorState } from "@codemirror/state";
    import { xml } from "@codemirror/lang-xml";
    import { basicSetup } from "@codemirror/basic-setup";
    import { lintGutter, linter, type Diagnostic } from "@codemirror/lint";
    import { validationStore } from "$lib/stores/validation";
    import { scrollPositionStore } from "$lib/stores/scrollPosition.svelte";
    import { daisyExtensions } from "$lib/editor/theme";

    let {
        content = "",
        onwordclick,
    }: {
        content?: string;
        onwordclick?: (
            facsimile: string,
            diplomatic: string,
            wordIndex: number,
            element: HTMLElement,
        ) => void;
    } = $props();

    // Svelte 5 derived state
    let validationState = $derived($validationStore);

    let container: HTMLDivElement;
    let view: EditorView;

    // Define a linter source that reads from the validation store
    const xmlLinter = linter((view) => {
        if (!validationState.lastResult || validationState.lastResult.valid) return [];
        
        const diagnostics: Diagnostic[] = [];
        for (const err of validationState.lastResult.errors) {
            // Error lines are 1-based, CodeMirror lines are 1-based for .line() access
            // But we need to be careful about bounds
            if (err.line && err.line <= view.state.doc.lines) {
                 const lineInfo = view.state.doc.line(err.line);
                 diagnostics.push({
                     from: lineInfo.from,
                     to: lineInfo.to,
                     severity: err.isWarning ? "warning" : "error",
                     message: err.message
                 });
            }
        }
        return diagnostics;
    });

    onMount(() => {
        const startState = EditorState.create({
            doc: content,
            extensions: [
                basicSetup,
                xml(),
                // Read-only mode
                EditorState.readOnly.of(true),
                // Linter integration
                lintGutter(),
                xmlLinter,
                // DaisyUI Theme
                daisyExtensions,
            ],
        });

        view = new EditorView({
            state: startState,
            parent: container,
        });

        // Restore scroll position after CodeMirror is ready
        requestAnimationFrame(() => {
            if (view?.scrollDOM) {
                const saved = scrollPositionStore.getPosition('xml');
                view.scrollDOM.scrollTop = saved.scrollTop;
                view.scrollDOM.scrollLeft = saved.scrollLeft;
            }
        });
    });

    onDestroy(() => {
        // Save scroll position before destroying
        if (view?.scrollDOM) {
            scrollPositionStore.savePosition(
                'xml',
                view.scrollDOM.scrollTop,
                view.scrollDOM.scrollLeft
            );
        }
        view?.destroy();
    });

    // Reactive update for content
    $effect(() => {
        if (view && content !== view.state.doc.toString()) {
            view.dispatch({
                changes: {
                    from: 0,
                    to: view.state.doc.length,
                    insert: content,
                },
            });
        }
    });
    
    // Trigger linting update when validation changes
    $effect(() => {
        if (view && validationState.lastResult) {
            // Dispatch a dummy transaction to trigger linter re-run
            view.dispatch({
                // Just dispatch empty to signal update
            });
        }
    });
</script>

<div class="h-full w-full" bind:this={container}></div>

<style>
    :global(.cm-editor) {
        height: 100%;
    }
</style>
