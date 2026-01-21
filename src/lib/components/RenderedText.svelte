<script lang="ts">
    import { entityStore, type EntityMap } from '$lib/stores/entities';
    import { getInflections, lemmaMappings } from '$lib/stores/dictionary';
    import { annotationsForWord, type AnnotationType } from '$lib/stores/annotations';
    import { validationStore } from '$lib/stores/validation';

    let {
        content = '',
        onwordclick,
    }: {
        content?: string;
        onwordclick?: (facsimile: string, diplomatic: string, wordIndex: number, element: HTMLElement, isSpanExtend?: boolean) => void;
    } = $props();

    interface TextToken {
        type: 'word' | 'punctuation' | 'linebreak' | 'pagebreak' | 'space' | 'text';
        displayText: string;  // What to show (facsimile with resolved entities)
        diplomatic?: string;  // For lemmatization lookup
        wordIndex?: number;   // The word's index in the document (for per-instance lemmatization)
        lineNumber?: string;
        pageNumber?: string;
        hasError?: boolean;
        errorTooltip?: string;
    }

    interface Page {
        pageNumber: string;        // e.g., "1r", "1v", or "intro" for content before first pb
        startTokenIndex: number;   // Absolute token index for quick lookups
        tokens: TextToken[];
        wordCount: number;
    }

    interface ParsedDocument {
        pages: Page[];
        totalTokens: number;
        totalWords: number;
    }

    // Access entity store reactively at top level
    let entities = $derived($entityStore.entities);
    // Access validation errors
    let validationState = $derived($validationStore);

    // Async parsing state to avoid blocking UI
    let document = $state<ParsedDocument | null>(null);
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

        if (currentContent === lastContent) return;
        lastContent = currentContent;

        if (!currentContent.trim()) {
            document = null;
            currentPageIndex = 0;
            return;
        }

        isRendering = true;

        // Use async IIFE for chunked parsing
        (async () => {
            // Yield first to let loading state render
            await yieldToMain();

            const result = await parseXmlAsync(currentContent, currentEntities);
            document = result;
            // Reset to first page when content changes
            currentPageIndex = 0;
            isRendering = false;
        })();
    });

    function sliceTag(xml: string, tagName: string): string | null {
        const lower = xml.toLowerCase();
        const start = lower.indexOf(`<${tagName}`);
        if (start === -1) return null;
        const end = lower.indexOf(`</${tagName}>`, start);
        if (end === -1) return null;
        return xml.slice(start, end + tagName.length + 3);
    }

    // Async version of parseXml that yields to main thread periodically
    // Returns ParsedDocument with tokens grouped by page
    async function parseXmlAsync(xml: string, entityDefs: EntityMap): Promise<ParsedDocument> {
        const emptyDoc: ParsedDocument = { pages: [], totalTokens: 0, totalWords: 0 };
        if (!xml.trim()) return emptyDoc;

        try {
            // Strip XML declaration if present (breaks when wrapped)
            let cleanXml = xml.replace(/<\?xml[^?]*\?>\s*/gi, '');

            // Strip DOCTYPE/entity declarations (not supported when wrapped)
            cleanXml = cleanXml
                .replace(/<!DOCTYPE[\s\S]*?\]>\s*/gi, '')
                .replace(/<!DOCTYPE[^>]*>\s*/gi, '');

            const bodyFragment = sliceTag(cleanXml, 'body') ?? sliceTag(cleanXml, 'text');
            const xmlFragment = bodyFragment ?? cleanXml;

            // Replace entity references with placeholders to avoid XML parsing errors
            const entityPattern = /&([a-zA-Z][a-zA-Z0-9]*);/g;
            const placeholderGlyphs = new Map<string, string>();
            let entityCounter = 0;

            const sanitized = xmlFragment.replace(entityPattern, (match, name) => {
                const placeholder = `__ENTITY_${entityCounter}__`;
                const glyph = entityDefs[name]?.char || `[${name}]`;
                placeholderGlyphs.set(placeholder, glyph);
                entityCounter++;
                return placeholder;
            });

            // Wrap in a root element with namespace declarations
            const wrapped = `<root xmlns:me="http://www.menota.org/ns/1.0">${sanitized}</root>`;
            const parser = new DOMParser();
            const doc = parser.parseFromString(wrapped, 'text/xml');

            const parseError = doc.querySelector('parsererror');
            if (parseError) {
                console.warn('XML parse error:', parseError.textContent);
                return {
                    pages: [{
                        pageNumber: 'error',
                        startTokenIndex: 0,
                        tokens: [{ type: 'text', displayText: xml }],
                        wordCount: 0
                    }],
                    totalTokens: 1,
                    totalWords: 0
                };
            }

            const allTokens: TextToken[] = [];
            const root = doc.documentElement;

            // Find the body content - try multiple approaches for namespaced XML
            let body: Element | null = null;

            // Try querySelector first
            body = root.querySelector('body');

            // If not found, try getElementsByTagName (works better with namespaces)
            if (!body) {
                const bodies = root.getElementsByTagName('body');
                if (bodies.length > 0) {
                    body = bodies[0];
                }
            }

            // Fall back to text element
            if (!body) {
                body = root.querySelector('text') || root.getElementsByTagName('text')[0];
            }

            // Final fallback to root
            const target = body || root;

            // Use a counter object to track word index and yield progress
            const wordCounter = { index: 0 };
            const yieldCounter = { count: 0 };
            const YIELD_EVERY = 500; // Yield every 500 tokens for smooth animation

            await extractTokensAsync(
                target,
                allTokens,
                placeholderGlyphs,
                wordCounter,
                yieldCounter,
                YIELD_EVERY,
            );

            // If no tokens extracted, show a message
            if (allTokens.length === 0) {
                return {
                    pages: [{
                        pageNumber: 'empty',
                        startTokenIndex: 0,
                        tokens: [{ type: 'text', displayText: '(No content in body)' }],
                        wordCount: 0
                    }],
                    totalTokens: 1,
                    totalWords: 0
                };
            }

            // Group tokens by page - split on pagebreak tokens
            const pages: Page[] = [];
            let currentPage: Page = {
                pageNumber: 'intro',
                startTokenIndex: 0,
                tokens: [],
                wordCount: 0
            };

            let absoluteTokenIndex = 0;
            for (const token of allTokens) {
                if (token.type === 'pagebreak') {
                    // If current page has content, save it
                    if (currentPage.tokens.length > 0) {
                        pages.push(currentPage);
                    }
                    // Start a new page
                    currentPage = {
                        pageNumber: token.pageNumber || `page-${pages.length + 1}`,
                        startTokenIndex: absoluteTokenIndex,
                        tokens: [token], // Include the pagebreak token at start of its page
                        wordCount: 0
                    };
                } else {
                    currentPage.tokens.push(token);
                    if (token.type === 'word') {
                        currentPage.wordCount++;
                    }
                }
                absoluteTokenIndex++;
            }

            // Don't forget the last page
            if (currentPage.tokens.length > 0) {
                pages.push(currentPage);
            }

            // Calculate totals
            const totalWords = pages.reduce((sum, p) => sum + p.wordCount, 0);

            return {
                pages,
                totalTokens: allTokens.length,
                totalWords
            };
        } catch (e) {
            console.error('Parse error:', e);
            return {
                pages: [{
                    pageNumber: 'error',
                    startTokenIndex: 0,
                    tokens: [{ type: 'text', displayText: `Parse error: ${e}` }],
                    wordCount: 0
                }],
                totalTokens: 1,
                totalWords: 0
            };
        }
    }

    const placeholderPattern = /__ENTITY_\d+__/g;

    // Resolve entity placeholders to actual glyphs
    function resolveEntitiesToGlyphs(text: string, placeholderGlyphs: Map<string, string>): string {
        if (!text.includes('__ENTITY_')) {
            return text;
        }
        return text.replace(placeholderPattern, (placeholder) => {
            return placeholderGlyphs.get(placeholder) ?? placeholder;
        });
    }

    function normalizeWhitespace(text: string): string {
        return text.replace(/\s+/g, ' ').trim();
    }

    function collectWordParts(
        element: Element,
        placeholderGlyphs: Map<string, string>
    ): Array<{ text: string; lineNumber?: string }> {
        const parts: Array<{ text: string; lineNumber?: string }> = [];
        let buffer = '';

        const flush = (lineNumber?: string) => {
            const normalized = normalizeWhitespace(buffer);
            if (normalized) {
                parts.push({
                    text: resolveEntitiesToGlyphs(normalized, placeholderGlyphs),
                    lineNumber,
                });
            } else if (lineNumber) {
                parts.push({ text: '', lineNumber });
            }
            buffer = '';
        };

        const walk = (node: Node) => {
            for (const child of node.childNodes) {
                if (child.nodeType === Node.TEXT_NODE) {
                    buffer += child.textContent ?? '';
                    continue;
                }

                if (child.nodeType !== Node.ELEMENT_NODE) {
                    continue;
                }

                const el = child as Element;
                if (el.localName === 'lb') {
                    flush(el.getAttribute('n') ?? undefined);
                    continue;
                }

                walk(el);
            }
        };

        walk(element);
        flush();

        return parts;
    }

    // Async version of extractTokens that yields periodically
    async function extractTokensAsync(
        node: Node,
        result: TextToken[],
        placeholderGlyphs: Map<string, string>,
        wordCounter: { index: number },
        yieldCounter: { count: number },
        yieldEvery: number
    ): Promise<void> {
        for (const child of node.childNodes) {
            // Yield to main thread periodically
            yieldCounter.count++;
            if (yieldCounter.count % yieldEvery === 0) {
                await yieldToMain();
            }

            if (child.nodeType === Node.TEXT_NODE) {
                const text = child.textContent || '';
                if (text.trim()) {
                    result.push({
                        type: 'text',
                        displayText: resolveEntitiesToGlyphs(text, placeholderGlyphs),
                    });
                }
            } else if (child.nodeType === Node.ELEMENT_NODE) {
                const el = child as Element;
                const tagName = el.localName;

                switch (tagName) {
                    case 'w': {
                        const facsEl = el.querySelector('me\\:facs, facs');
                        const diplEl = el.querySelector('me\\:dipl, dipl');
                        const wordIndex = wordCounter.index;
                        wordCounter.index++;

                        let parts = facsEl ? collectWordParts(facsEl, placeholderGlyphs) : [];
                        if (!parts.some(part => part.text || part.lineNumber)) {
                            parts = diplEl ? collectWordParts(diplEl, placeholderGlyphs) : [];
                        }
                        if (!parts.some(part => part.text || part.lineNumber)) {
                            parts = collectWordParts(el, placeholderGlyphs);
                        }

                        const diplomatic = normalizeWhitespace(
                            resolveEntitiesToGlyphs(
                                diplEl?.textContent || facsEl?.textContent || el.textContent || '',
                                placeholderGlyphs,
                            ),
                        );
                        const lastTextIndex = parts.reduce(
                            (last, part, index) => (part.text ? index : last),
                            -1,
                        );

                        if (lastTextIndex >= 0) {
                            parts.forEach((part, index) => {
                                if (part.text) {
                                    result.push({
                                        type: 'word',
                                        displayText: part.text,
                                        diplomatic,
                                        wordIndex,
                                    });
                                }
                                if (part.lineNumber) {
                                    result.push({
                                        type: 'linebreak',
                                        displayText: '',
                                        lineNumber: part.lineNumber,
                                    });
                                }
                                if (part.text && index === lastTextIndex) {
                                    result.push({ type: 'space', displayText: ' ' });
                                }
                            });
                        } else {
                            parts.forEach((part) => {
                                if (part.lineNumber) {
                                    result.push({
                                        type: 'linebreak',
                                        displayText: '',
                                        lineNumber: part.lineNumber,
                                    });
                                }
                            });
                        }
                        break;
                    }
                    case 'pc': {
                        const facsEl = el.querySelector('me\\:facs, facs');
                        const facsRaw = facsEl?.textContent || el.textContent || '';
                        const displayText = resolveEntitiesToGlyphs(facsRaw, placeholderGlyphs).trim();
                        if (displayText) {
                            result.push({ type: 'punctuation', displayText });
                            result.push({ type: 'space', displayText: ' ' });
                        }
                        break;
                    }
                    case 'lb': {
                        const n = el.getAttribute('n');
                        result.push({ type: 'linebreak', displayText: '', lineNumber: n || undefined });
                        break;
                    }
                    case 'pb': {
                        const n = el.getAttribute('n');
                        result.push({ type: 'pagebreak', displayText: '', pageNumber: n || undefined });
                        break;
                    }
                    case 'gap': {
                        const quantity = el.getAttribute('quantity') || '?';
                        result.push({ type: 'text', displayText: `[…${quantity}]` });
                        break;
                    }
                    case 'supplied': {
                        const text = normalizeWhitespace(
                            resolveEntitiesToGlyphs(el.textContent || '', placeholderGlyphs),
                        );
                        if (text) {
                            result.push({ type: 'text', displayText: `⟨${text}⟩` });
                        }
                        break;
                    }
                    case 'unclear': {
                        const text = resolveEntitiesToGlyphs(el.textContent || '', placeholderGlyphs);
                        result.push({ type: 'text', displayText: `${text}?` });
                        break;
                    }
                    case 'del': {
                        const text = resolveEntitiesToGlyphs(el.textContent || '', placeholderGlyphs);
                        result.push({ type: 'text', displayText: `⟦${text}⟧` });
                        break;
                    }
                    case 'add': {
                        const text = resolveEntitiesToGlyphs(el.textContent || '', placeholderGlyphs);
                        result.push({ type: 'text', displayText: `\\${text}/` });
                        break;
                    }
                    case 'choice': {
                        const abbr = el.querySelector('abbr')?.textContent || '';
                        const text = resolveEntitiesToGlyphs(abbr, placeholderGlyphs);
                        if (text) {
                            result.push({ type: 'text', displayText: text });
                            result.push({ type: 'space', displayText: ' ' });
                        }
                        break;
                    }
                    case 'head': {
                        await extractTokensAsync(
                            el,
                            result,
                            placeholderGlyphs,
                            wordCounter,
                            yieldCounter,
                            yieldEvery,
                        );
                        result.push({ type: 'linebreak', displayText: '' });
                        break;
                    }
                    default:
                        // Recurse into other elements
                        await extractTokensAsync(el, result, placeholderGlyphs, wordCounter, yieldCounter, yieldEvery);
                }
            }
        }
    }

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
