<script lang="ts">
    import RenderedText from './RenderedText.svelte';

    let {
        content = '',
        onwordclick,
    }: {
        content?: string;
        onwordclick?: (facsimile: string, diplomatic: string, wordIndex: number, element: HTMLElement) => void;
    } = $props();

    let viewMode = $state<'xml' | 'rendered'>('rendered');
</script>

<div class="w-full h-full flex flex-col bg-base-200">
    <div class="flex justify-between items-center px-4 py-2 bg-base-200 border-b border-base-300 font-medium text-sm">
        <span class="text-md font-bold xl:text-lg">
            {viewMode === 'xml' ? 'TEI-XML Preview' : 'Rendered Text'}
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
    <div class="flex-1 overflow-auto">
        {#if viewMode === 'xml'}
            <pre class="m-0 p-4 font-mono text-sm leading-relaxed whitespace-pre-wrap warp-break-word"><code>{content}</code></pre>
        {:else}
            <RenderedText {content} {onwordclick} />
        {/if}
    </div>
</div>
