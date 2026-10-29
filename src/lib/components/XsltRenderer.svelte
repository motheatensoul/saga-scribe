<script lang="ts">
    import { untrack } from 'svelte';
    import { createXsltProcessor, loadStylesheetText, runXsltTransform } from '$lib/utils/xslt';
    import { entityStore } from '$lib/stores/entities';
    import { lemmaMappings, getInflections } from '$lib/stores/dictionary';
    import { importedStore } from '$lib/stores/imported.svelte';
    import { isPageLocal, renderPages, splitPages } from '$lib/rendered/xsltPages';

    let {
        content = '',
        xslPath = '/xsl/simple.xsl',
        active = true,
        onwordclick,
    }: {
        content?: string;
        xslPath?: string;
        /** Hidden views skip updates and catch up when shown again */
        active?: boolean;
        onwordclick?: (facsimile: string, diplomatic: string, wordIndex: number, element: HTMLElement) => void;
    } = $props();

    let renderedHtml = $state('');
    let error = $state<string | null>(null);
    let xslProcessor = $state<XSLTProcessor | null>(null);
    /** Whether the stylesheet may be applied one page at a time (see xsltPages.ts) */
    let pageLocal = false;
    /** Rendering page by page: the pages live in `pagesEl`, `renderedHtml` is unused */
    let pageMode = $state(false);
    /** Bumped whenever the rendered pages change, so word styling runs again */
    let pagesVersion = $state(0);
    let pagesEl: HTMLDivElement;
    /** Each rendered page's source document and element, in order */
    let pages: { source: string; element: HTMLElement }[] = [];
    /** Page HTML by source document, for the current stylesheet and entities */
    let pageCache = new Map<string, string>();
    let isTransforming = $state(false);
    let containerEl: HTMLDivElement;
    let enhanceVersion = 0;
    /** Inputs of the current rendering; an unchanged tab switch doesn't re-run the stylesheet */
    let renderedFrom: { processor: XSLTProcessor; content: string; entities: unknown } | null = null;

    const yieldToMain = () => new Promise((resolve) => setTimeout(resolve, 0));

    let loadVersion = 0;

    $effect(() => {
        const currentPath = xslPath;
        const version = ++loadVersion;
        xslProcessor = null;
        renderedHtml = '';
        error = null;
        showPages([], []);
        pageCache = new Map();

        if (!currentPath) {
            error = 'No stylesheet selected';
            return;
        }

        (async () => {
            try {
                const xslText = await loadStylesheetText(currentPath);
                if (version !== loadVersion) return;
                pageLocal = isPageLocal(xslText);
                xslProcessor = createXsltProcessor(xslText);
            } catch (e) {
                if (version !== loadVersion) return;
                error = `Failed to load XSLT: ${e}`;
                console.error(error);
            }
        })();
    });

    // Transform XML when content, processor, or entities change
    $effect(() => {
        const processor = xslProcessor;
        const xmlContent = content;
        // Track entity store changes so we re-transform when entities load
        const entities = $entityStore.entities;
        const entitiesLoaded = $entityStore.loaded;

        if (!processor || !xmlContent.trim()) {
            renderedHtml = '';
            showPages([], []);
            renderedFrom = null;
            isTransforming = false;
            return;
        }

        // Don't transform until entities are loaded (to avoid showing [entityName] placeholders)
        if (!entitiesLoaded) {
            isTransforming = true;
            return;
        }

        // Only while visible, and only when the inputs changed. A page-local
        // stylesheet only re-runs on the pages that changed.
        if (!active) return;
        if (
            renderedFrom?.processor === processor &&
            renderedFrom.content === xmlContent &&
            renderedFrom.entities === entities
        ) {
            return;
        }

        // The previous rendering stays visible until the new one is ready
        isTransforming = true;
        error = null;

        // A newer run (or unmount) aborts this one at its next stage boundary
        const controller = new AbortController();
        (async () => {
            try {
                const paged = pageLocal ? splitPages(xmlContent) : null;
                if (paged) {
                    // Cached pages only fit the same stylesheet and entities
                    const cache =
                        renderedFrom?.processor === processor && renderedFrom.entities === entities
                            ? pageCache
                            : new Map<string, string>();
                    const result = await renderPages(processor, paged, entities, cache, controller.signal);
                    pageCache = result.cache;
                    showPages(paged.pieces, result.html);
                    renderedHtml = '';
                } else {
                    // Whole-document transform: the stylesheet computes its own parameters
                    processor.removeParameter(null, 'lineBased');
                    const html = await runXsltTransform(processor, xmlContent, entities, {
                        bodyOnly: true,
                        signal: controller.signal,
                    });
                    showPages([], []);
                    renderedHtml = html;
                }
                renderedFrom = { processor, content: xmlContent, entities };
                error = null;
                isTransforming = false;
            } catch (e) {
                if (controller.signal.aborted) return;
                error = e instanceof Error ? e.message : `Transform error: ${e}`;
                console.error(error);
                renderedHtml = '';
                showPages([], []);
                isTransforming = false;
            }
        })();

        return () => controller.abort();
    });

    /**
     * Shows rendered pages, touching only those whose source changed: a page
     * with the same source keeps its element (and the browser its layout).
     */
    function showPages(sources: string[], html: string[]) {
        if (!pagesEl) return;
        if (sources.length === pages.length) {
            sources.forEach((source, i) => {
                if (pages[i].source !== source) {
                    pages[i].element.innerHTML = html[i];
                    pages[i].source = source;
                }
            });
        } else {
            // Pages added or removed: reuse unchanged pages' elements
            const unused = new Map<string, HTMLElement[]>();
            for (const page of pages) {
                unused.set(page.source, [...(unused.get(page.source) ?? []), page.element]);
            }
            pages = sources.map((source, i) => {
                let element = unused.get(source)?.shift();
                if (!element) {
                    element = document.createElement('div');
                    element.className = 'xslt-page';
                    element.innerHTML = html[i];
                }
                return { source, element };
            });
            pagesEl.replaceChildren(...pages.map((page) => page.element));
        }
        // Called from effects: updating this state must not make them depend on it
        untrack(() => {
            pageMode = sources.length > 0;
            pagesVersion++;
        });
    }

    // Set up delegated word click handlers
    $effect(() => {
        if (!containerEl || !onwordclick) return;

        const handler = (event: MouseEvent) => {
            const target = (event.target as HTMLElement | null)?.closest<HTMLElement>(
                '[data-word-index]'
            );
            if (!target) return;
            event.preventDefault();
            const wordIndex = parseInt(target.dataset.wordIndex ?? '-1', 10);
            const diplomatic = target.dataset.diplomatic || target.textContent || '';
            const facsimile = target.dataset.facsimile || target.textContent || '';
            onwordclick(facsimile, diplomatic, wordIndex, target);
        };

        containerEl.addEventListener('click', handler);
        return () => containerEl.removeEventListener('click', handler);
    });

    // Add word styling in chunks to avoid blocking
    $effect(() => {
        void pagesVersion;
        if (!containerEl || (!renderedHtml && !pageMode)) return;

        const currentVersion = ++enhanceVersion;
        const wordElements = Array.from(
            containerEl.querySelectorAll<HTMLElement>('[data-word-index]')
        );
        const lemmaMap = $lemmaMappings.mappings;
        const editedWords = importedStore.editedWords;
        const inflectionCache = new Map<string, boolean>();
        const YIELD_EVERY = 500;

        void (async () => {
            // A word broken across lines has a box per line; the later ones
            // (continuations) belong to the same word
            let wordIndex = -1;
            for (let index = 0; index < wordElements.length; index++) {
                if (currentVersion !== enhanceVersion) return;
                const element = wordElements[index];
                if (element.dataset.wordContinuation === undefined || wordIndex < 0) {
                    wordIndex++;
                }
                element.dataset.wordIndex = `${wordIndex}`;
                const diplomatic = element.dataset.diplomatic || element.textContent || '';

                element.classList.remove('is-confirmed', 'has-suggestion', 'is-edited');
                element.classList.toggle('is-edited', editedWords.has(wordIndex));

                if (wordIndex in lemmaMap) {
                    element.classList.add('is-confirmed');
                } else if (diplomatic) {
                    let hasSuggestion = inflectionCache.get(diplomatic);
                    if (hasSuggestion === undefined) {
                        hasSuggestion = $getInflections(diplomatic).length > 0;
                        inflectionCache.set(diplomatic, hasSuggestion);
                    }
                    if (hasSuggestion) {
                        element.classList.add('has-suggestion');
                    }
                }

                if (index % YIELD_EVERY === 0) {
                    await yieldToMain();
                }
            }
        })();
    });
</script>

<div class="xslt-rendered p-4 text-lg leading-loose" bind:this={containerEl}>
    {#if error}
        <div class="alert alert-error">
            <span>{error}</span>
        </div>
    {:else if !xslProcessor}
        <div class="flex items-center justify-center h-32">
            <span class="loading loading-spinner loading-md"></span>
            <span class="ml-2">Loading stylesheet...</span>
        </div>
    {:else if pageMode}
        <!-- The pages are in pagesEl below -->
    {:else if isTransforming && !renderedHtml}
        <div class="flex items-center justify-center h-32">
            <!-- SVG spinner for reliable animation during heavy processing -->
            <svg viewBox="0 0 50 50" width="32" height="32" class="text-primary">
                <circle cx="25" cy="25" r="20" fill="none" stroke="currentColor"
                    stroke-width="4" stroke-linecap="round"
                    stroke-dasharray="90, 150" stroke-dashoffset="0">
                    <animateTransform attributeName="transform" type="rotate"
                        from="0 25 25" to="360 25 25" dur="1s" repeatCount="indefinite"/>
                </circle>
            </svg>
            <span class="ml-2">Transforming XML...</span>
        </div>
    {:else if !renderedHtml}
        <div class="text-base-content/50 italic">(No content to render)</div>
    {:else}
        {@html renderedHtml}
    {/if}
    <!-- Page by page rendering, managed by showPages() -->
    <div bind:this={pagesEl}></div>
</div>

<style>
    .xslt-rendered {
        font-family: 'Junicode', Georgia, serif;
    }

    /* Ensure the prose container inherits Junicode for PUA character support */
    .xslt-rendered :global(.prose) {
        font-family: inherit;
    }

    /* Word styling - mirrors RenderedText.svelte */
    .xslt-rendered :global(.word) {
        display: inline;
        padding: 0.125rem 0.25rem;
        margin: 0 -0.125rem;
        border-radius: 0.25rem;
        border: 1px solid transparent;
        background: transparent;
        cursor: pointer;
        transition: all 0.15s ease;
    }

    .xslt-rendered :global(.word.word-stack),
    .xslt-rendered :global(.punctuation.word-stack) {
        display: inline-flex;
        flex-direction: column;
        align-items: flex-start;
        vertical-align: top;
    }

    .xslt-rendered :global(.word-line) {
        display: block;
        line-height: 1.4;
    }

    .xslt-rendered :global(.word:hover) {
        border-color: var(--color-base-300);
        background-color: color-mix(in oklch, var(--color-base-200) 50%, transparent);
    }

    .xslt-rendered :global(.word.is-confirmed) {
        border-color: var(--color-success);
        background-color: color-mix(in oklch, var(--color-success) 15%, transparent);
    }

    .xslt-rendered :global(.word.has-suggestion) {
        border-color: color-mix(in oklch, var(--color-warning) 30%, transparent);
        opacity: 0.75;
    }

    .xslt-rendered :global(.word.has-suggestion:hover) {
        border-color: var(--color-warning);
        background-color: color-mix(in oklch, var(--color-warning) 15%, transparent);
        opacity: 1;
    }

    /* Edited since import: lemma and normalized form are the original's */
    .xslt-rendered :global(.word.is-edited) {
        text-decoration: underline wavy var(--color-warning);
        text-underline-offset: 0.25em;
    }

    /* Hyphen at the end of a line that breaks a word */
    .xslt-rendered :global(.word-break-mark) {
        opacity: 0.5;
        user-select: none;
    }

    /* Start of a paragraph; paragraphs run inline, as lines decide the layout */
    .xslt-rendered :global(.paragraph-mark) {
        opacity: 0.35;
        margin-right: 0.25rem;
        user-select: none;
    }

    /* Character elements (<c>) - ensure Junicode font for PUA characters */
    .xslt-rendered :global(.char) {
        font-family: 'Junicode', Georgia, serif;
    }

    /* Initial capitals - typically larger */
    .xslt-rendered :global(.char-initial) {
        font-size: 1.5em;
        line-height: 1;
        vertical-align: baseline;
    }

    /* Handle rend attributes with specific colors */
    .xslt-rendered :global(.char[data-rend*="cBlue"]) {
        color: var(--color-info);
    }

    .xslt-rendered :global(.char[data-rend*="cRed"]) {
        color: var(--color-error);
    }

    .xslt-rendered :global(.char[data-rend*="cGreen"]) {
        color: var(--color-success);
    }

    /* Abbreviation markers (<am>) and expansions (<ex>) - ensure Junicode font for PUA characters */
    .xslt-rendered :global(.abbr-marker),
    .xslt-rendered :global(.abbr-expansion) {
        font-family: 'Junicode', Georgia, serif;
    }
</style>
