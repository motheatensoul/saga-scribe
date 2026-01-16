<script lang="ts">
    import { onMount } from 'svelte';
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
    let containerEl: HTMLDivElement;
    let enhanceVersion = 0;

    const placeholderPattern = /__ENTITY_\d+__/g;
    const yieldToMain = () => new Promise((resolve) => setTimeout(resolve, 0));

    function resolveEntitiesToGlyphs(
        text: string,
        placeholderGlyphs: Map<string, string>
    ): string {
        if (!text.includes('__ENTITY_')) {
            return text;
        }
        return text.replace(placeholderPattern, (placeholder) => {
            return placeholderGlyphs.get(placeholder) ?? placeholder;
        });
    }

    function sliceTag(xml: string, tagName: string): string | null {
        const lower = xml.toLowerCase();
        const start = lower.indexOf(`<${tagName}`);
        if (start === -1) return null;
        const end = lower.indexOf(`</${tagName}>`, start);
        if (end === -1) return null;
        return xml.slice(start, end + tagName.length + 3);
    }

    // Load the XSL stylesheet once on mount
    onMount(async () => {
        try {
            const response = await fetch(xslPath);
            if (!response.ok) {
                throw new Error(`Failed to load stylesheet: ${response.statusText}`);
            }
            const xslText = await response.text();

            const parser = new DOMParser();
            const xslDoc = parser.parseFromString(xslText, 'application/xml');

            const parseError = xslDoc.querySelector('parsererror');
            if (parseError) {
                throw new Error(`XSL parse error: ${parseError.textContent}`);
            }

            const processor = new XSLTProcessor();
            processor.importStylesheet(xslDoc);
            xslProcessor = processor;
        } catch (e) {
            error = `Failed to load XSLT: ${e}`;
            console.error(error);
        }
    });

    // Transform XML when content or processor changes
    $effect(() => {
        if (!xslProcessor || !content.trim()) {
            renderedHtml = '';
            return;
        }

        try {
            // Prepare the XML content
            let xmlContent = content;

            // Strip DOCTYPE/entity declarations (not supported by DOMParser)
            xmlContent = xmlContent
                .replace(/<!DOCTYPE[\s\S]*?\]>\s*/gi, '')
                .replace(/<!DOCTYPE[^>]*>\s*/gi, '');

            const bodyFragment = sliceTag(xmlContent, 'body') ?? sliceTag(xmlContent, 'text');
            const xmlFragment = bodyFragment
                ? `<TEI xmlns="http://www.tei-c.org/ns/1.0" xmlns:me="http://www.menota.org/ns/1.0">${bodyFragment}</TEI>`
                : xmlContent;

            // Replace entity references with placeholders to avoid XML parsing errors
            const entityPattern = /&([a-zA-Z][a-zA-Z0-9]*);/g;
            const placeholderGlyphs = new Map<string, string>();
            let entityCounter = 0;
            const entities = $entityStore.entities;

            xmlContent = xmlFragment.replace(entityPattern, (match, name) => {
                // Skip standard XML entities
                if (['lt', 'gt', 'amp', 'quot', 'apos'].includes(name)) {
                    return match;
                }
                const placeholder = `__ENTITY_${entityCounter}__`;
                const glyph = entities[name]?.char || `[${name}]`;
                placeholderGlyphs.set(placeholder, glyph);
                entityCounter++;
                return placeholder;
            });

            const parser = new DOMParser();
            const xmlDoc = parser.parseFromString(xmlContent, 'application/xml');

            const parseError = xmlDoc.querySelector('parsererror');
            if (parseError) {
                error = `XML parse error: ${parseError.textContent}`;
                renderedHtml = '';
                return;
            }

            // Apply XSLT transformation
            const resultDoc = xslProcessor.transformToDocument(xmlDoc);

            if (!resultDoc || !resultDoc.documentElement) {
                error = 'XSLT transformation produced no output';
                renderedHtml = '';
                return;
            }

            // Get the HTML content
            let html = resultDoc.documentElement.outerHTML;

            // Resolve entity placeholders to actual glyphs
            html = resolveEntitiesToGlyphs(html, placeholderGlyphs);

            renderedHtml = html;
            error = null;
        } catch (e) {
            error = `Transform error: ${e}`;
            console.error(error);
            renderedHtml = '';
        }
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
            const facsimile = target.textContent || '';
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

<div class="xslt-rendered p-4 font-serif text-lg leading-loose" bind:this={containerEl}>
    {#if error}
        <div class="alert alert-error">
            <span>{error}</span>
        </div>
    {:else if !xslProcessor}
        <div class="flex items-center justify-center h-32">
            <span class="loading loading-spinner loading-md"></span>
            <span class="ml-2">Loading stylesheet...</span>
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
</style>
