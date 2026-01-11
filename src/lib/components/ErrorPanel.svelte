<script lang="ts">
    import { errorStore, errorCounts } from '$lib/stores/errors';
    import { X as CloseButton } from "@lucide/svelte";

    let { onclose }: { onclose?: () => void } = $props();

    function formatTime(date: Date): string {
        return date.toLocaleTimeString();
    }

    function getLevelIcon(level: string): string {
        switch (level) {
            case 'error':
                return '✕';
            case 'warning':
                return '⚠';
            case 'info':
                return 'ℹ';
            default:
                return '•';
        }
    }
</script>

<div class="bg-base-100 text-base-content font-mono text-sm h-full flex flex-col">
    <div class="flex justify-between items-center p-3 border-b border-base-300">
        <h2 class="flex items-center gap-2 font-bold">
            Logs
            {#if $errorCounts.error > 0}
                <span class="badge badge-error badge-sm">{$errorCounts.error}</span>
            {/if}
            {#if $errorCounts.warning > 0}
                <span class="badge badge-warning badge-sm">{$errorCounts.warning}</span>
            {/if}
        </h2>
        <div class="flex gap-2 items-center">
            <button class="btn btn-ghost btn-xs" onclick={() => errorStore.clear()} aria-label="Clear logs">Clear</button>
            <button class="btn btn-ghost btn-sm btn-circle" onclick={onclose} aria-label="Close">
                <CloseButton size={16} />
            </button>
        </div>
    </div>

    <div class="overflow-y-auto flex-1 p-2">
        {#each $errorStore.slice().reverse() as error (error.id)}
            <div class="grid grid-cols-[1.5rem_4rem_8rem_1fr] gap-2 p-1.5 rounded hover:bg-base-200 items-start">
                <span class="text-center" class:text-error={error.level === 'error'} class:text-warning={error.level === 'warning'} class:text-info={error.level === 'info'}>
                    {getLevelIcon(error.level)}
                </span>
                <span class="opacity-50">{formatTime(error.timestamp)}</span>
                <span class="text-primary">[{error.source}]</span>
                <span>{error.message}</span>
                {#if error.details}
                    <pre class="col-span-4 ml-8 mt-1 p-2 bg-base-300 rounded text-xs opacity-70 whitespace-pre-wrap break-all">{error.details}</pre>
                {/if}
            </div>
        {:else}
            <div class="text-center py-8 opacity-50">No log entries</div>
        {/each}
    </div>
</div>
