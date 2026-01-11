<script lang="ts">
    import RenderedText from './RenderedText.svelte';
    import XsltRenderer from './XsltRenderer.svelte';
    import XmlPreview from './XmlPreview.svelte';

    let {
        content = '',
        onwordclick,
    }: {
        content?: string;
        onwordclick?: (facsimile: string, diplomatic: string, wordIndex: number, element: HTMLElement) => void;
    } = $props();

    let viewMode = $state<'xml' | 'rendered' | 'xslt'>('rendered');
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
    <div class="flex-1 overflow-auto h-full">
        {#if viewMode === 'xml'}
            <XmlPreview {content} />
        {:else if viewMode === 'xslt'}
            <XsltRenderer {content} {onwordclick} />
        {:else}
            <RenderedText {content} {onwordclick} />
        {/if}
    </div>
</div>
