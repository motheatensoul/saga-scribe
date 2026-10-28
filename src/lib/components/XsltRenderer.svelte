<script lang="ts">
    import { readTextFile } from '@tauri-apps/plugin-fs';
    import { escapeHtml, sanitizeXsltHtml, transformResultToHtml } from '$lib/utils/xsltOutput';
    import { entityStore } from '$lib/stores/entities';
    import { lemmaMappings, getInflections } from '$lib/stores/dictionary';

    let {
        content = '',
        xslPath = '/xsl/simple.xsl',
        onwordclick,
    }: {
        content?: string;
        xslPath?: string;
        onwordclick?: (facsimile: string, diplomatic: string, wordIndex: number, element: HTMLElement) => void;
    } = $props();

    let renderedHtml = $state('');
    let error = $state<string | null>(null);
    let xslProcessor = $state<XSLTProcessor | null>(null);
    let isTransforming = $state(false);
    let containerEl: HTMLDivElement;
    let enhanceVersion = 0;

    const yieldToMain = () => new Promise((resolve) => setTimeout(resolve, 0));

    function sliceTag(xml: string, tagName: string): string | null {
        const lower = xml.toLowerCase();
        const start = lower.indexOf(`<${tagName}`);
        if (start === -1) return null;
        const end = lower.indexOf(`</${tagName}>`, start);
        if (end === -1) return null;
        return xml.slice(start, end + tagName.length + 3);
    }

    let loadVersion = 0;

    async function loadStylesheetText(path: string): Promise<string> {
        if (path.startsWith('/xsl/')) {
            const response = await fetch(path);
            if (!response.ok) {
                throw new Error(`Failed to load stylesheet: ${response.statusText}`);
            }
            return response.text();
        }

        return readTextFile(path);
    }

    $effect(() => {
        const currentPath = xslPath;
        const version = ++loadVersion;
        xslProcessor = null;
        renderedHtml = '';
        error = null;

        if (!currentPath) {
            error = 'No stylesheet selected';
            return;
        }

        (async () => {
            try {
                const xslText = await loadStylesheetText(currentPath);
                if (version !== loadVersion) return;

                const parser = new DOMParser();
                const xslDoc = parser.parseFromString(xslText, 'application/xml');

                const parseError = xslDoc.querySelector('parsererror');
                if (parseError) {
                    throw new Error(`XSL parse error: ${parseError.textContent}`);
                }

                const processor = new XSLTProcessor();
                processor.importStylesheet(xslDoc);
                if (version !== loadVersion) return;
                xslProcessor = processor;
            } catch (e) {
                if (version !== loadVersion) return;
                error = `Failed to load XSLT: ${e}`;
                console.error(error);
            }
        })();
    });

    // Transform XML when content, processor, or entities change
    let transformVersion = 0;

    $effect(() => {
        const processor = xslProcessor;
        const xmlContent = content;
        // Track entity store changes so we re-transform when entities load
        const entities = $entityStore.entities;
        const entitiesLoaded = $entityStore.loaded;

        if (!processor || !xmlContent.trim()) {
            renderedHtml = '';
            isTransforming = false;
            return;
        }

        // Don't transform until entities are loaded (to avoid showing [entityName] placeholders)
        if (!entitiesLoaded) {
            isTransforming = true;
            return;
        }

        const version = ++transformVersion;
        isTransforming = true;
        renderedHtml = '';
        error = null;

        // Defer transformation to allow loading spinner to render
        (async () => {
            await yieldToMain();
            if (version !== transformVersion) return;

            try {
                // Prepare the XML content
                let prepared = xmlContent;

                // Strip DOCTYPE/entity declarations (not supported by DOMParser)
                prepared = prepared
                    .replace(/<!DOCTYPE[\s\S]*?\]>\s*/gi, '')
                    .replace(/<!DOCTYPE[^>]*>\s*/gi, '');

                const bodyFragment = sliceTag(prepared, 'body') ?? sliceTag(prepared, 'text');
                const xmlFragment = bodyFragment
                    ? `<TEI xmlns="http://www.tei-c.org/ns/1.0" xmlns:me="http://www.menota.org/ns/1.0">${bodyFragment}</TEI>`
                    : prepared;

                // Replace named entity references with placeholders, then resolve after XSLT
                // Use ASCII-safe placeholders that won't be stripped by XML/XSLT processing
                const entityPattern = /&([a-zA-Z][a-zA-Z0-9]*);/g;
                const placeholderMap = new Map<string, string>();
                let entityCounter = 0;

                prepared = xmlFragment.replace(entityPattern, (match, name) => {
                    // Skip standard XML entities - these must remain as entities
                    if (['lt', 'gt', 'amp', 'quot', 'apos'].includes(name)) {
                        return match;
                    }
                    const entity = entities[name];
                    // Use format like XENT0X, XENT1X - letters+numbers only
                    const placeholder = `XENT${entityCounter}X`;
                    const glyph = entity?.char || `[${name}]`;
                    placeholderMap.set(placeholder, glyph);
                    entityCounter++;
                    return placeholder;
                });

                if (version !== transformVersion) return;
                await yieldToMain();

                const parser = new DOMParser();
                const xmlDoc = parser.parseFromString(prepared, 'application/xml');

                const parseError = xmlDoc.querySelector('parsererror');
                if (parseError) {
                    if (version !== transformVersion) return;
                    error = `XML parse error: ${parseError.textContent}`;
                    renderedHtml = '';
                    isTransforming = false;
                    return;
                }

                if (version !== transformVersion) return;
                await yieldToMain();

                // Apply XSLT transformation
                const resultDoc = processor.transformToDocument(xmlDoc);

                if (version !== transformVersion) return;

                if (!resultDoc || !resultDoc.documentElement) {
                    error = 'XSLT transformation produced no output';
                    renderedHtml = '';
                    isTransforming = false;
                    return;
                }

                // Get the HTML content
                let html = transformResultToHtml(resultDoc);

                // Resolve placeholders to actual glyphs
                // Placeholders are formatted as XENT{number}X
                const placeholderRegex = /XENT(\d+)X/g;
                html = html.replace(placeholderRegex, (match) => {
                    const glyph = placeholderMap.get(match);
                    return glyph !== undefined ? escapeHtml(glyph) : match;
                });

                if (version !== transformVersion) return;
                // XML and stylesheet may be third-party: never render executable content
                renderedHtml = sanitizeXsltHtml(html);
                error = null;
                isTransforming = false;
            } catch (e) {
                if (version !== transformVersion) return;
                error = `Transform error: ${e}`;
                console.error(error);
                renderedHtml = '';
                isTransforming = false;
            }
        })();
    });

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
        if (!containerEl || !renderedHtml) return;

        const currentVersion = ++enhanceVersion;
        const wordElements = Array.from(
            containerEl.querySelectorAll<HTMLElement>('[data-word-index]')
        );
        const lemmaMap = $lemmaMappings.mappings;
        const inflectionCache = new Map<string, boolean>();
        const YIELD_EVERY = 500;

        void (async () => {
            for (let index = 0; index < wordElements.length; index++) {
                if (currentVersion !== enhanceVersion) return;
                const element = wordElements[index];
                const wordIndex = index;
                element.dataset.wordIndex = `${wordIndex}`;
                const diplomatic = element.dataset.diplomatic || element.textContent || '';

                element.classList.remove('is-confirmed', 'has-suggestion');

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
    {:else if isTransforming}
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
