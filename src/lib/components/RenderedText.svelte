<script lang="ts">
    import { entityStore, type EntityMap } from '$lib/stores/entities';
    import { getInflections, sessionLemmaStore } from '$lib/stores/dictionary';
    import { validationStore } from '$lib/stores/validation';

    let {
        content = '',
        onwordclick,
    }: {
        content?: string;
        onwordclick?: (facsimile: string, diplomatic: string, wordIndex: number, element: HTMLElement) => void;
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

    // Access entity store reactively at top level
    let entities = $derived($entityStore.entities);
    // Access validation errors
    let validationState = $derived($validationStore);

    // Async parsing state to avoid blocking UI
    let tokens = $state<TextToken[]>([]);
    let isRendering = $state(false);
    let lastContent = $state('');
    let isTruncated = $state(false);
    let totalTokenCount = $state(0);

    // Helper to yield to browser for smooth animations
    const yieldToMain = () => new Promise(resolve => setTimeout(resolve, 0));

    // React to content changes and parse asynchronously
    $effect(() => {
        const currentContent = content;
        const currentEntities = entities;

        if (currentContent === lastContent) return;
        lastContent = currentContent;

        if (!currentContent.trim()) {
            tokens = [];
            return;
        }

        isRendering = true;

        // Use async IIFE for chunked parsing
        (async () => {
            // Yield first to let loading state render
            await yieldToMain();

            const result = await parseXmlAsync(currentContent, currentEntities);

            // Limit tokens to prevent browser hang - large documents need virtualization
            const MAX_TOKENS = 5000;
            totalTokenCount = result.length;
            if (result.length > MAX_TOKENS) {
                tokens = result.slice(0, MAX_TOKENS);
                isTruncated = true;
            } else {
                tokens = result;
                isTruncated = false;
            }
            isRendering = false;
        })();
    });

    // Async version of parseXml that yields to main thread periodically
    async function parseXmlAsync(xml: string, entityDefs: EntityMap): Promise<TextToken[]> {
        if (!xml.trim()) return [];

        try {
            // Strip XML declaration if present (breaks when wrapped)
            let cleanXml = xml.replace(/<\?xml[^?]*\?>\s*/gi, '');

            // Replace entity references with placeholders to avoid XML parsing errors
            const entityPattern = /&([a-zA-Z][a-zA-Z0-9]*);/g;
            const entityMap = new Map<string, string>();
            let entityCounter = 0;

            const sanitized = cleanXml.replace(entityPattern, (match, name) => {
                const placeholder = `__ENTITY_${entityCounter}__`;
                entityMap.set(placeholder, name);
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
                return [{ type: 'text', displayText: xml }];
            }

            const result: TextToken[] = [];
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

            await extractTokensAsync(target, result, entityMap, entityDefs, wordCounter, yieldCounter, YIELD_EVERY);

            // If no tokens extracted, show a message
            if (result.length === 0) {
                return [{ type: 'text', displayText: '(No content in body)' }];
            }

            return result;
        } catch (e) {
            console.error('Parse error:', e);
            return [{ type: 'text', displayText: `Parse error: ${e}` }];
        }
    }

    function parseXml(xml: string, entityDefs: EntityMap): TextToken[] {
        if (!xml.trim()) return [];

        try {
            // Strip XML declaration if present (breaks when wrapped)
            let cleanXml = xml.replace(/<\?xml[^?]*\?>\s*/gi, '');

            // Replace entity references with placeholders to avoid XML parsing errors
            const entityPattern = /&([a-zA-Z][a-zA-Z0-9]*);/g;
            const entityMap = new Map<string, string>();
            let entityCounter = 0;

            const sanitized = cleanXml.replace(entityPattern, (match, name) => {
                const placeholder = `__ENTITY_${entityCounter}__`;
                entityMap.set(placeholder, name);
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
                return [{ type: 'text', displayText: xml }];
            }

            const result: TextToken[] = [];
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

            // Use a counter object to track word index across recursive calls
            const wordCounter = { index: 0 };
            extractTokens(target, result, entityMap, entityDefs, wordCounter);

            // If no tokens extracted, show a message
            if (result.length === 0) {
                return [{ type: 'text', displayText: '(No content in body)' }];
            }

            return result;
        } catch (e) {
            console.error('Parse error:', e);
            return [{ type: 'text', displayText: `Parse error: ${e}` }];
        }
    }

    // Resolve entity placeholders to actual glyphs
    function resolveEntitiesToGlyphs(text: string, entityMap: Map<string, string>, entityDefs: EntityMap): string {
        let result = text;
        for (const [placeholder, entityName] of entityMap) {
            // Look up the entity in our entity definitions
            const entity = entityDefs[entityName];
            const glyph = entity?.char || `[${entityName}]`;
            result = result.replaceAll(placeholder, glyph);
        }
        return result;
    }

    function extractTokens(node: Node, result: TextToken[], entityMap: Map<string, string>, entityDefs: EntityMap, wordCounter: { index: number }): void {
        for (const child of node.childNodes) {
            if (child.nodeType === Node.TEXT_NODE) {
                const text = child.textContent || '';
                if (text.trim()) {
                    // Plain text outside of elements - resolve entities for display
                    result.push({
                        type: 'text',
                        displayText: resolveEntitiesToGlyphs(text, entityMap, entityDefs),
                    });
                }
            } else if (child.nodeType === Node.ELEMENT_NODE) {
                const el = child as Element;
                const tagName = el.localName;

                switch (tagName) {
                    case 'w': {
                        // Word element - get facsimile for display, diplomatic for lookup
                        const facsEl = el.querySelector('me\\:facs, facs');
                        const diplEl = el.querySelector('me\\:dipl, dipl');

                        // Get raw content (with placeholders)
                        const facsRaw = facsEl?.textContent || el.textContent || '';
                        const diplRaw = diplEl?.textContent || facsRaw;

                        // Facsimile: resolve to glyphs for display
                        const displayText = resolveEntitiesToGlyphs(facsRaw, entityMap, entityDefs).trim();

                        // Diplomatic: resolve for lookup
                        const diplomatic = resolveEntitiesToGlyphs(diplRaw, entityMap, entityDefs).trim();

                        // Track word index sequentially (matches compiler's word_index counter)
                        const wordIndex = wordCounter.index;
                        wordCounter.index++;

                        if (displayText) {
                            result.push({
                                type: 'word',
                                displayText,
                                diplomatic,
                                wordIndex,
                            });
                            result.push({ type: 'space', displayText: ' ' });
                        }
                        break;
                    }
                    case 'pc': {
                        // Punctuation - same logic
                        const facsEl = el.querySelector('me\\:facs, facs');
                        const facsRaw = facsEl?.textContent || el.textContent || '';
                        const displayText = resolveEntitiesToGlyphs(facsRaw, entityMap, entityDefs).trim();

                        if (displayText) {
                            result.push({ type: 'punctuation', displayText });
                            result.push({ type: 'space', displayText: ' ' });
                        }
                        break;
                    }
                    case 'lb': {
                        // Line break with optional number
                        const n = el.getAttribute('n');
                        result.push({
                            type: 'linebreak',
                            displayText: '',
                            lineNumber: n || undefined,
                        });
                        break;
                    }
                    case 'pb': {
                        // Page break with folio number
                        const n = el.getAttribute('n');
                        result.push({
                            type: 'pagebreak',
                            displayText: '',
                            pageNumber: n || undefined,
                        });
                        break;
                    }
                    case 'gap': {
                        const quantity = el.getAttribute('quantity') || '?';
                        result.push({ type: 'text', displayText: `[…${quantity}]` });
                        break;
                    }
                    case 'supplied': {
                        const text = resolveEntitiesToGlyphs(el.textContent || '', entityMap, entityDefs);
                        result.push({ type: 'text', displayText: `⟨${text}⟩` });
                        break;
                    }
                    case 'unclear': {
                        const text = resolveEntitiesToGlyphs(el.textContent || '', entityMap, entityDefs);
                        result.push({ type: 'text', displayText: `${text}?` });
                        break;
                    }
                    case 'del': {
                        const text = resolveEntitiesToGlyphs(el.textContent || '', entityMap, entityDefs);
                        result.push({ type: 'text', displayText: `⟦${text}⟧` });
                        break;
                    }
                    case 'add': {
                        const text = resolveEntitiesToGlyphs(el.textContent || '', entityMap, entityDefs);
                        result.push({ type: 'text', displayText: `\\${text}/` });
                        break;
                    }
                    case 'choice': {
                        // Show abbreviation form (facsimile)
                        const abbr = el.querySelector('abbr')?.textContent || '';
                        const text = resolveEntitiesToGlyphs(abbr, entityMap, entityDefs);
                        if (text) {
                            result.push({ type: 'text', displayText: text });
                            result.push({ type: 'space', displayText: ' ' });
                        }
                        break;
                    }
                    default:
                        // Recurse into other elements (div, p, etc.)
                        extractTokens(el, result, entityMap, entityDefs, wordCounter);
                }
            }
        }
    }

    // Async version of extractTokens that yields periodically
    async function extractTokensAsync(
        node: Node,
        result: TextToken[],
        entityMap: Map<string, string>,
        entityDefs: EntityMap,
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
                        displayText: resolveEntitiesToGlyphs(text, entityMap, entityDefs),
                    });
                }
            } else if (child.nodeType === Node.ELEMENT_NODE) {
                const el = child as Element;
                const tagName = el.localName;

                switch (tagName) {
                    case 'w': {
                        const facsEl = el.querySelector('me\\:facs, facs');
                        const diplEl = el.querySelector('me\\:dipl, dipl');
                        const facsRaw = facsEl?.textContent || el.textContent || '';
                        const diplRaw = diplEl?.textContent || facsRaw;
                        const displayText = resolveEntitiesToGlyphs(facsRaw, entityMap, entityDefs).trim();
                        const diplomatic = resolveEntitiesToGlyphs(diplRaw, entityMap, entityDefs).trim();
                        const wordIndex = wordCounter.index;
                        wordCounter.index++;

                        if (displayText) {
                            result.push({ type: 'word', displayText, diplomatic, wordIndex });
                            result.push({ type: 'space', displayText: ' ' });
                        }
                        break;
                    }
                    case 'pc': {
                        const facsEl = el.querySelector('me\\:facs, facs');
                        const facsRaw = facsEl?.textContent || el.textContent || '';
                        const displayText = resolveEntitiesToGlyphs(facsRaw, entityMap, entityDefs).trim();
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
                        const text = resolveEntitiesToGlyphs(el.textContent || '', entityMap, entityDefs);
                        result.push({ type: 'text', displayText: `⟨${text}⟩` });
                        break;
                    }
                    case 'unclear': {
                        const text = resolveEntitiesToGlyphs(el.textContent || '', entityMap, entityDefs);
                        result.push({ type: 'text', displayText: `${text}?` });
                        break;
                    }
                    case 'del': {
                        const text = resolveEntitiesToGlyphs(el.textContent || '', entityMap, entityDefs);
                        result.push({ type: 'text', displayText: `⟦${text}⟧` });
                        break;
                    }
                    case 'add': {
                        const text = resolveEntitiesToGlyphs(el.textContent || '', entityMap, entityDefs);
                        result.push({ type: 'text', displayText: `\\${text}/` });
                        break;
                    }
                    case 'choice': {
                        const abbr = el.querySelector('abbr')?.textContent || '';
                        const text = resolveEntitiesToGlyphs(abbr, entityMap, entityDefs);
                        if (text) {
                            result.push({ type: 'text', displayText: text });
                            result.push({ type: 'space', displayText: ' ' });
                        }
                        break;
                    }
                    default:
                        // Recurse into other elements
                        await extractTokensAsync(el, result, entityMap, entityDefs, wordCounter, yieldCounter, yieldEvery);
                }
            }
        }
    }

    function handleWordClick(token: TextToken, event: MouseEvent) {
        const target = event.currentTarget as HTMLElement;
        // Pass both facsimile (displayText) and diplomatic forms for lemmatization
        const facsimile = token.displayText;
        const diplomatic = token.diplomatic || token.displayText;
        onwordclick?.(facsimile, diplomatic, token.wordIndex ?? -1, target);
    }

    function hasKnownLemma(word: string): boolean {
        const inflections = $getInflections(word);
        return inflections.length > 0;
    }

    // Check if this specific word instance is confirmed in the session
    function isWordConfirmed(wordIndex: number | undefined): boolean {
        if (wordIndex === undefined || wordIndex < 0) return false;
        return wordIndex in $sessionLemmaStore.mappings;
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

<div class="rendered-text p-4 font-serif text-lg leading-loose relative">
    {#if validationState.lastResult && !validationState.lastResult.valid}
         <div class="absolute top-0 right-0 p-2 text-xs text-error opacity-50 hover:opacity-100 transition-opacity">
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
    {:else}
    {#if isTruncated}
        <div class="alert alert-warning mb-4 text-sm">
            <span>Large document: showing first 5,000 tokens of {totalTokenCount.toLocaleString()}. Use XML view for full content.</span>
        </div>
    {/if}
    {#each tokens as token, i}
        {#if token.type === 'word'}
            <button
                type="button"
                class="word-token"
                class:is-confirmed={isWordConfirmed(token.wordIndex)}
                class:has-suggestion={!isWordConfirmed(token.wordIndex) && hasKnownLemma(token.diplomatic || token.displayText)}
                onclick={(e) => handleWordClick(token, e)}
                title={formatLemmaTooltip(token)}
            >
                {token.displayText}
            </button>
        {:else if token.type === 'punctuation'}
            <span class="punctuation">{token.displayText}</span>
        {:else if token.type === 'linebreak'}
            <!-- lb marks START of line, so br comes before (except at start or after pb) -->
            {#if i > 0 && tokens[i - 1]?.type !== 'pagebreak'}
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
</style>
