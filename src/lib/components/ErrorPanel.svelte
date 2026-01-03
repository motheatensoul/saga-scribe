<script lang="ts">
    import { errorStore, errorCounts } from '$lib/stores/errors';

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

<div class="error-panel">
    <div class="header">
        <h2>
            Logs
            {#if $errorCounts.error > 0}
                <span class="badge error">{$errorCounts.error}</span>
            {/if}
            {#if $errorCounts.warning > 0}
                <span class="badge warning">{$errorCounts.warning}</span>
            {/if}
        </h2>
        <div class="actions">
            <button class="clear-btn" onclick={() => errorStore.clear()}>Clear</button>
            <button class="close-btn" onclick={onclose}>×</button>
        </div>
    </div>

    <div class="error-list">
        {#each $errorStore.slice().reverse() as error (error.id)}
            <div class="error-item {error.level}">
                <span class="icon">{getLevelIcon(error.level)}</span>
                <span class="time">{formatTime(error.timestamp)}</span>
                <span class="source">[{error.source}]</span>
                <span class="message">{error.message}</span>
                {#if error.details}
                    <pre class="details">{error.details}</pre>
                {/if}
            </div>
        {:else}
            <div class="empty">No log entries</div>
        {/each}
    </div>
</div>

<style>
    .error-panel {
        background: #1e1e1e;
        color: #d4d4d4;
        border-radius: 8px;
        width: 700px;
        max-height: 70vh;
        display: flex;
        flex-direction: column;
        overflow: hidden;
        font-family: monospace;
        font-size: 0.875rem;
    }

    .header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.75rem 1rem;
        border-bottom: 1px solid #333;
        background: #252526;
    }

    h2 {
        margin: 0;
        font-size: 1rem;
        display: flex;
        align-items: center;
        gap: 0.5rem;
    }

    .badge {
        font-size: 0.75rem;
        padding: 0.125rem 0.375rem;
        border-radius: 4px;
    }

    .badge.error {
        background: #f14c4c;
        color: white;
    }

    .badge.warning {
        background: #cca700;
        color: black;
    }

    .actions {
        display: flex;
        gap: 0.5rem;
    }

    .clear-btn {
        background: #333;
        border: none;
        color: #d4d4d4;
        padding: 0.25rem 0.5rem;
        border-radius: 4px;
        cursor: pointer;
        font-size: 0.75rem;
    }

    .clear-btn:hover {
        background: #444;
    }

    .close-btn {
        background: none;
        border: none;
        font-size: 1.25rem;
        cursor: pointer;
        color: #808080;
    }

    .close-btn:hover {
        color: #d4d4d4;
    }

    .error-list {
        flex: 1;
        overflow-y: auto;
        padding: 0.5rem;
    }

    .error-item {
        display: grid;
        grid-template-columns: 1.5rem 4rem 8rem 1fr;
        gap: 0.5rem;
        padding: 0.375rem 0.5rem;
        border-radius: 4px;
        align-items: start;
    }

    .error-item:hover {
        background: #2a2a2a;
    }

    .error-item.error .icon {
        color: #f14c4c;
    }

    .error-item.warning .icon {
        color: #cca700;
    }

    .error-item.info .icon {
        color: #3794ff;
    }

    .icon {
        text-align: center;
    }

    .time {
        color: #808080;
    }

    .source {
        color: #9cdcfe;
    }

    .message {
        color: #d4d4d4;
    }

    .details {
        grid-column: 1 / -1;
        margin: 0.25rem 0 0 2rem;
        padding: 0.5rem;
        background: #1a1a1a;
        border-radius: 4px;
        overflow-x: auto;
        font-size: 0.75rem;
        color: #808080;
        white-space: pre-wrap;
        word-break: break-all;
    }

    .empty {
        text-align: center;
        color: #808080;
        padding: 2rem;
    }
</style>
