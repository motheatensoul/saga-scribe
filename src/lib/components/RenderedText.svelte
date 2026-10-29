<script lang="ts">
    import { entityStore } from '$lib/stores/entities';
    import { getInflections, lemmaMappings } from '$lib/stores/dictionary';
    import { annotationsForWord, type AnnotationType } from '$lib/stores/annotations';
    import { validationStore } from '$lib/stores/validation';
    import type { ParsedDocument, TextToken } from '$lib/rendered/document';
    import { renderFull, renderUpdate, type RenderedState } from '$lib/rendered/incremental';

    let {
        content = '',
        active = true,
        onwordclick,
    }: {
        content?: string;
        /** Hidden views skip updates and catch up when shown again */
        active?: boolean;
        onwordclick?: (facsimile: string, diplomatic: string, wordIndex: number, element: HTMLElement, isSpanExtend?: boolean) => void;
    } = $props();


    // Access entity store reactively at top level
    let entities = $derived($entityStore.entities);
    // Access validation errors
    let validationState = $derived($validationStore);

    // Async parsing state to avoid blocking UI
    // Raw state: always replaced wholesale, never mutated in place
    let document = $state.raw<ParsedDocument | null>(null);
    /** What `document` was rendered from, so edits can re-parse only affected pages */
    let rendered: RenderedState | null = null;
    let isRendering = $state(false);
    let lastContent = $state('');
    let currentPageIndex = $state(0);

    // Pre-compute confirmed word set for O(1) lookups during render
    let confirmedWordSet = $derived(
        new Set(Object.keys($lemmaMappings.mappings).map(Number))
    );

    // Visible pages: current + 1 adjacent on each side for scroll buffer
    let visiblePages = $derived(() => {
        if (!document || document.pages.length === 0) return [];
        const start = Math.max(0, currentPageIndex - 1);
        const end = Math.min(document.pages.length, currentPageIndex + 2);
        return document.pages.slice(start, end);
    });

    // Helper to yield to browser for smooth animations
    const yieldToMain = () => new Promise(resolve => setTimeout(resolve, 0));

    // React to content changes and parse asynchronously
    $effect(() => {
        const currentContent = content;
        const currentEntities = entities;

        if (!active) return;
        if (currentContent === lastContent) return;
        lastContent = currentContent;

        if (!currentContent.trim()) {
            document = null;
            rendered = null;
            currentPageIndex = 0;
            return;
        }

        // Only show the spinner when there is nothing to show yet; on edits the
        // previous rendering stays on screen (and keeps its scroll position)
        // until the new one is ready.
        if (!document) isRendering = true;

        // Use async IIFE for chunked parsing
        (async () => {
            // Yield first to let loading state render
            await yieldToMain();

            const next = rendered
                ? await renderUpdate(rendered, currentContent, currentEntities)
                : await renderFull(currentContent, currentEntities);
            // A newer edit started its own parse; drop this stale result
            if (currentContent !== lastContent) return;
            rendered = next;
            const result = next.doc;

            // Stay on the page being viewed if it still exists
            const viewedPage = document?.pages[currentPageIndex]?.pageNumber;
            const sameIndex = result.pages.findIndex((p) => p.pageNumber === viewedPage);
            document = result;
            currentPageIndex = sameIndex >= 0 ? sameIndex : 0;
            isRendering = false;
        })();
    });

    function handleWordClick(token: TextToken, event: MouseEvent) {
        const target = event.currentTarget as HTMLElement;
        // Pass both facsimile (displayText) and diplomatic forms for lemmatization
        const facsimile = token.displayText;
        const diplomatic = token.diplomatic || token.displayText;
        // Shift-click extends selection for span annotations
        const isSpanExtend = event.shiftKey;
        onwordclick?.(facsimile, diplomatic, token.wordIndex ?? -1, target, isSpanExtend);
    }

    function hasKnownLemma(word: string): boolean {
        const inflections = $getInflections(word);
        return inflections.length > 0;
    }

    // Check if this specific word instance is confirmed in the session
    // Uses pre-computed confirmedWordSet for O(1) lookup
    function isWordConfirmed(wordIndex: number | undefined): boolean {
        if (wordIndex === undefined || wordIndex < 0) return false;
        return confirmedWordSet.has(wordIndex);
    }

    // Check for different annotation types on a word
    function getWordAnnotationTypes(wordIndex: number | undefined): Set<AnnotationType> {
        if (wordIndex === undefined || wordIndex < 0) return new Set();
        const anns = $annotationsForWord(wordIndex);
        return new Set(anns.map(a => a.type));
    }

    // Check if word has specific annotation type
    function hasAnnotationType(wordIndex: number | undefined, type: AnnotationType): boolean {
        if (wordIndex === undefined || wordIndex < 0) return false;
        return getWordAnnotationTypes(wordIndex).has(type);
    }

    // Navigation functions
    function goToPage(index: number) {
        if (!document) return;
        currentPageIndex = Math.max(0, Math.min(index, document.pages.length - 1));
    }

    function goToPrevPage() {
        goToPage(currentPageIndex - 1);
    }

    function goToNextPage() {
        goToPage(currentPageIndex + 1);
    }

    function formatLemmaTooltip(token: TextToken): string | undefined {
        const word = token.diplomatic || token.displayText;
        const inflections = $getInflections(word);

        if (inflections.length === 0) {
            if (token.diplomatic && token.diplomatic !== token.displayText) {
                return `diplomatic: ${token.diplomatic}`;
            }
            return undefined;
        }

        const parts: string[] = [];
        for (const inf of inflections) {
            parts.push(`${inf.lemma} (${inf.analysis})`);
        }

        let tooltip = parts.join('; ');
        if (token.diplomatic && token.diplomatic !== token.displayText) {
            tooltip = `diplomatic: ${token.diplomatic}\n${tooltip}`;
        }
        return tooltip;
    }
</script>

<div class="rendered-text font-serif text-lg leading-loose relative">
    {#if validationState.lastResult && !validationState.lastResult.valid}
         <div class="absolute top-0 right-0 p-2 text-xs text-error opacity-50 hover:opacity-100 transition-opacity z-10">
            {validationState.lastResult.errors.length} schema errors
         </div>
    {/if}

    {#if isRendering}
        <div class="flex flex-col items-center justify-center py-12">
            <!-- SVG spinner with explicit animation for smooth rendering during async parsing -->
            <svg viewBox="0 0 50 50" width="48" height="48">
                <circle
                    cx="25"
                    cy="25"
                    r="20"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="4"
                    stroke-linecap="round"
                    stroke-dasharray="90, 150"
                    stroke-dashoffset="0"
                    class="text-primary"
                >
                    <animateTransform
                        attributeName="transform"
                        type="rotate"
                        from="0 25 25"
                        to="360 25 25"
                        dur="1s"
                        repeatCount="indefinite"
                    />
                </circle>
            </svg>
            <p class="mt-4 text-base-content/70">Rendering preview...</p>
        </div>
    {:else if document && document.pages.length > 0}
        <!-- Page navigation header -->
        {#if document.pages.length > 1}
            <div class="page-nav sticky top-0 bg-base-100/95 backdrop-blur-sm border-b border-base-300 px-4 py-2 flex items-center justify-between gap-2 z-10">
                <div class="flex items-center gap-1">
                    <button
                        type="button"
                        class="btn btn-ghost btn-xs"
                        onclick={() => goToPage(0)}
                        disabled={currentPageIndex === 0}
                        title="First page"
                    >
                        <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" viewBox="0 0 20 20" fill="currentColor">
                            <path fill-rule="evenodd" d="M15.707 15.707a1 1 0 01-1.414 0l-5-5a1 1 0 010-1.414l5-5a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 010 1.414zm-6 0a1 1 0 01-1.414 0l-5-5a1 1 0 010-1.414l5-5a1 1 0 011.414 1.414L5.414 10l4.293 4.293a1 1 0 010 1.414z" clip-rule="evenodd" />
                        </svg>
                    </button>
                    <button
                        type="button"
                        class="btn btn-ghost btn-xs"
                        onclick={goToPrevPage}
                        disabled={currentPageIndex === 0}
                        title="Previous page"
                    >
                        <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" viewBox="0 0 20 20" fill="currentColor">
                            <path fill-rule="evenodd" d="M12.707 5.293a1 1 0 010 1.414L9.414 10l3.293 3.293a1 1 0 01-1.414 1.414l-4-4a1 1 0 010-1.414l4-4a1 1 0 011.414 0z" clip-rule="evenodd" />
                        </svg>
                    </button>
                </div>

                <div class="flex items-center gap-2">
                    <select
                        class="select select-bordered select-xs"
                        bind:value={currentPageIndex}
                    >
                        {#each document.pages as page, i}
                            <option value={i}>{page.pageNumber}</option>
                        {/each}
                    </select>
                    <span class="text-xs text-base-content/60">
                        of {document.pages.length}
                    </span>
                </div>

                <div class="flex items-center gap-1">
                    <button
                        type="button"
                        class="btn btn-ghost btn-xs"
                        onclick={goToNextPage}
                        disabled={currentPageIndex === document.pages.length - 1}
                        title="Next page"
                    >
                        <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" viewBox="0 0 20 20" fill="currentColor">
                            <path fill-rule="evenodd" d="M7.293 14.707a1 1 0 010-1.414L10.586 10 7.293 6.707a1 1 0 011.414-1.414l4 4a1 1 0 010 1.414l-4 4a1 1 0 01-1.414 0z" clip-rule="evenodd" />
                        </svg>
                    </button>
                    <button
                        type="button"
                        class="btn btn-ghost btn-xs"
                        onclick={() => document && goToPage(document.pages.length - 1)}
                        disabled={!document || currentPageIndex === document.pages.length - 1}
                        title="Last page"
                    >
                        <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" viewBox="0 0 20 20" fill="currentColor">
                            <path fill-rule="evenodd" d="M4.293 15.707a1 1 0 010-1.414L8.586 10 4.293 5.707a1 1 0 011.414-1.414l5 5a1 1 0 010 1.414l-5 5a1 1 0 01-1.414 0zm6 0a1 1 0 010-1.414L14.586 10l-4.293-4.293a1 1 0 011.414-1.414l5 5a1 1 0 010 1.414l-5 5a1 1 0 01-1.414 0z" clip-rule="evenodd" />
                        </svg>
                    </button>
                </div>
            </div>
        {/if}

        <!-- Document stats bar -->
        <div class="px-4 py-1 text-xs text-base-content/50 border-b border-base-200">
            {document.totalWords.toLocaleString()} words / {document.totalTokens.toLocaleString()} tokens
            {#if document.pages.length > 1}
                / {document.pages.length} pages
            {/if}
        </div>

        <!-- Page content - render visible pages only -->
        <div class="page-content p-4">
            {#each visiblePages() as page, pageIdx (page.pageNumber)}
                <div class="page-section" data-page={page.pageNumber}>
                    {#each page.tokens as token, i}
                        {#if token.type === 'word'}
                            <button
                                type="button"
                                class="word-token"
                                class:is-confirmed={isWordConfirmed(token.wordIndex)}
                                class:has-suggestion={!isWordConfirmed(token.wordIndex) && hasKnownLemma(token.diplomatic || token.displayText)}
                                class:has-note={hasAnnotationType(token.wordIndex, 'note')}
                                class:has-semantic={hasAnnotationType(token.wordIndex, 'semantic')}
                                class:has-paleographic={hasAnnotationType(token.wordIndex, 'paleographic')}
                                onclick={(e) => handleWordClick(token, e)}
                                title={formatLemmaTooltip(token)}
                            >
                                {token.displayText}
                            </button>
                        {:else if token.type === 'punctuation'}
                            <span class="punctuation">{token.displayText}</span>
                        {:else if token.type === 'linebreak'}
                            <!-- lb marks START of line, so br comes before (except at start or after pb) -->
                            {#if i > 0 && page.tokens[i - 1]?.type !== 'pagebreak'}
                                <br />
                            {/if}
                            <span class="line-number">{token.lineNumber || ''}</span>
                        {:else if token.type === 'pagebreak'}
                            <div class="pagebreak">
                                <span class="page-indicator">{token.pageNumber || '?'}</span>
                            </div>
                        {:else if token.type === 'space'}
                            {' '}
                        {:else}
                            <span class="plain-text">{token.displayText}</span>
                        {/if}
                    {/each}
                </div>
            {/each}
        </div>
    {:else}
        <div class="p-4 text-base-content/50 text-center">
            No content to display
        </div>
    {/if}
</div>

<style>
    .rendered-text {
        font-family: 'Junicode', Georgia, serif;
    }

    .word-token {
        display: inline;
        padding: 0.125rem 0.25rem;
        margin: 0 -0.125rem;
        border-radius: 0.25rem;
        border: 1px solid transparent;
        background: transparent;
        cursor: pointer;
        transition: all 0.15s ease;
        font: inherit;
        color: inherit;
    }

    .word-token:hover {
        border-color: var(--color-base-300);
        background-color: color-mix(in oklch, var(--color-base-200) 50%, transparent);
    }

    .word-token:focus {
        outline: none;
        border-color: var(--color-primary);
        background-color: color-mix(in oklch, var(--color-primary) 10%, transparent);
    }

    /* Confirmed words - solid success styling */
    .word-token.is-confirmed {
        border-color: var(--color-success);
        background-color: color-mix(in oklch, var(--color-success) 15%, transparent);
    }

    .word-token.is-confirmed:hover {
        border-color: var(--color-success);
        background-color: color-mix(in oklch, var(--color-success) 25%, transparent);
    }

    /* Unconfirmed words with suggestions - faded styling */
    .word-token.has-suggestion {
        border-color: color-mix(in oklch, var(--color-warning) 30%, transparent);
        opacity: 0.75;
    }

    .word-token.has-suggestion:hover {
        border-color: var(--color-warning);
        background-color: color-mix(in oklch, var(--color-warning) 15%, transparent);
        opacity: 1;
    }

    /* Words with notes - accent underline */
    .word-token.has-note {
        border-bottom: 2px dashed var(--color-accent);
    }

    /* Words with semantic annotations - secondary indicator */
    .word-token.has-semantic {
        border-top: 2px solid var(--color-secondary);
    }

    /* Words with paleographic observations - info indicator */
    .word-token.has-paleographic {
        box-shadow: inset 0 0 0 1px var(--color-info);
    }

    /* Combined states - show multiple indicators */
    .word-token.has-note.has-semantic {
        border-bottom: 2px dashed var(--color-accent);
        border-top: 2px solid var(--color-secondary);
    }

    .word-token.has-paleographic.is-confirmed {
        box-shadow: inset 0 0 0 1px var(--color-info);
        border-color: var(--color-success);
    }

    .punctuation {
        color: var(--color-base-content);
    }

    .line-number {
        display: inline-block;
        min-width: 2rem;
        margin-right: 0.5rem;
        font-size: 0.75rem;
        color: var(--color-base-content);
        opacity: 0.4;
        font-family: monospace;
        text-align: right;
    }

    .pagebreak {
        display: block;
        margin: 1.5rem 0;
        text-align: center;
    }

    .page-indicator {
        display: inline-block;
        padding: 0.25rem 0.75rem;
        font-size: 0.875rem;
        color: var(--color-base-content);
        opacity: 0.4;
        font-family: monospace;
        border: 1px dashed currentColor;
        border-radius: 0.25rem;
    }

    .plain-text {
        white-space: pre-wrap;
    }

    .page-nav {
        font-family: system-ui, sans-serif;
    }

    .page-section {
        /* CSS content-visibility for performance - browser can skip rendering off-screen content */
        content-visibility: auto;
        contain-intrinsic-size: auto 500px;
    }

    .page-content {
        /* Ensure smooth scrolling between pages */
        scroll-behavior: smooth;
    }
</style>
