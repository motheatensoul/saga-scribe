<script lang="ts">
    import RenderedText from './RenderedText.svelte';
    import XsltRenderer from './XsltRenderer.svelte';
    import XmlPreview from './XmlPreview.svelte';

    let {
        content = '',
        xslPath = '/xsl/simple.xsl',
        onwordclick,
    }: {
        content?: string;
        xslPath?: string;
        onwordclick?: (facsimile: string, diplomatic: string, wordIndex: number, element: HTMLElement) => void;
    } = $props();

    let viewMode = $state<'xml' | 'rendered' | 'xslt'>('rendered');

    // Track which views have been visited (for lazy mount + keep alive)
    // Once a view is visited, it stays mounted but hidden when inactive
    let visited = $state<Record<'xml' | 'rendered' | 'xslt', boolean>>({
        rendered: true,  // Default view, always visited
        xslt: false,
        xml: false,
    });

    // Mark view as visited when selected
    $effect(() => {
        if (!visited[viewMode]) {
            visited[viewMode] = true;
        }
    });
</script>

<div class="w-full h-full flex flex-col bg-base-200">
    <div class="flex justify-between items-center px-4 py-2 bg-base-200 border-b border-base-300 font-medium text-sm">
        <span class="text-md font-bold xl:text-lg">
            {#if viewMode === 'xml'}
                TEI-XML Preview
            {:else if viewMode === 'xslt'}
                XSLT Rendered
            {:else}
                Rendered Text
            {/if}
        </span>
        <div class="flex gap-1" role="tablist" aria-label="Preview mode">
            <button
                type="button"
                role="tab"
                aria-selected={viewMode === 'rendered'}
                class="btn btn-xs xl:btn-sm text-xs xl:text-sm"
                class:btn-primary={viewMode === 'rendered'}
                class:btn-ghost={viewMode !== 'rendered'}
                onclick={() => (viewMode = 'rendered')}
            >
                Text
            </button>
            <button
                type="button"
                role="tab"
                aria-selected={viewMode === 'xslt'}
                class="btn btn-xs xl:btn-sm text-xs xl:text-sm"
                class:btn-primary={viewMode === 'xslt'}
                class:btn-ghost={viewMode !== 'xslt'}
                onclick={() => (viewMode = 'xslt')}
                title="Render using XSLT stylesheet"
            >
                XSLT
            </button>
            <button
                type="button"
                role="tab"
                aria-selected={viewMode === 'xml'}
                class="btn btn-xs xl:btn-sm text-xs xl:text-sm"
                class:btn-primary={viewMode === 'xml'}
                class:btn-ghost={viewMode !== 'xml'}
                onclick={() => (viewMode = 'xml')}
            >
                XML
            </button>
        </div>
    </div>
    <div class="flex-1 overflow-hidden h-full relative">
        <!-- Lazy mount + keep alive: only mount when first visited, then keep mounted but hidden -->
        {#if visited.rendered}
            <div class="absolute inset-0 overflow-auto" class:hidden={viewMode !== 'rendered'}>
                <RenderedText {content} {onwordclick} />
            </div>
        {/if}
        {#if visited.xslt}
            <div class="absolute inset-0 overflow-auto" class:hidden={viewMode !== 'xslt'}>
                <XsltRenderer {content} {xslPath} {onwordclick} />
            </div>
        {/if}
        {#if visited.xml}
            <div class="absolute inset-0 overflow-auto" class:hidden={viewMode !== 'xml'}>
                <XmlPreview {content} />
            </div>
        {/if}
    </div>
</div>
