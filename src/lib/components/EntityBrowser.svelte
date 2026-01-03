<script lang="ts">
    import { entityStore, entityNames, type Entity } from '$lib/stores/entities';

    let { oninsert, onclose }: { oninsert?: (text: string) => void; onclose?: () => void } = $props();

    let searchQuery = $state('');
    let selectedCategory = $state<string | null>(null);

    // Get unique categories
    const categories = $derived.by(() => {
        const cats = new Set<string>();
        for (const entity of Object.values($entityStore.entities)) {
            if (entity.category) cats.add(entity.category);
        }
        return Array.from(cats).sort();
    });

    // Filter entities based on search and category
    const filteredEntities = $derived.by(() => {
        const query = searchQuery.toLowerCase();
        return Object.entries($entityStore.entities)
            .filter(([name, entity]) => {
                const matchesSearch =
                    !query ||
                    name.toLowerCase().includes(query) ||
                    entity.description.toLowerCase().includes(query) ||
                    entity.char.includes(query);
                const matchesCategory = !selectedCategory || entity.category === selectedCategory;
                return matchesSearch && matchesCategory;
            })
            .sort((a, b) => a[0].localeCompare(b[0]));
    });

    function handleInsert(name: string) {
        oninsert?.(`:${name}:`);
    }
</script>

<div class="entity-browser">
    <div class="header">
        <h2>Entity Browser</h2>
        <button class="close-btn" onclick={onclose}>×</button>
    </div>

    <div class="controls">
        <input
            type="text"
            placeholder="Search entities..."
            bind:value={searchQuery}
            class="search-input"
        />
        <select bind:value={selectedCategory} class="category-select">
            <option value={null}>All categories</option>
            {#each categories as category}
                <option value={category}>{category}</option>
            {/each}
        </select>
    </div>

    {#if !$entityStore.loaded}
        <div class="loading">Loading entities...</div>
    {:else if $entityStore.error}
        <div class="error">{$entityStore.error}</div>
    {:else}
        <div class="entity-list">
            {#each filteredEntities as [name, entity]}
                <button class="entity-item" onclick={() => handleInsert(name)} title={entity.description}>
                    <span class="entity-char">{entity.char}</span>
                    <span class="entity-name">:{name}:</span>
                    <span class="entity-desc">{entity.description}</span>
                </button>
            {:else}
                <div class="no-results">No entities found</div>
            {/each}
        </div>
    {/if}

    <div class="footer">
        <span class="count">{filteredEntities.length} entities</span>
    </div>
</div>

<style>
    .entity-browser {
        background: white;
        border-radius: 8px;
        width: 500px;
        height: 70vh;
        max-height: 600px;
        min-height: 300px;
        display: flex;
        flex-direction: column;
        overflow: hidden;
        box-shadow: 0 4px 20px rgba(0, 0, 0, 0.15);
    }

    .header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 1rem;
        border-bottom: 1px solid #e1e4e8;
    }

    h2 {
        margin: 0;
        font-size: 1.25rem;
    }

    .close-btn {
        background: none;
        border: none;
        font-size: 1.5rem;
        cursor: pointer;
        color: #666;
    }

    .controls {
        display: flex;
        gap: 0.5rem;
        padding: 0.75rem 1rem;
        border-bottom: 1px solid #e1e4e8;
    }

    .search-input {
        flex: 1;
        padding: 0.5rem;
        border: 1px solid #d1d5da;
        border-radius: 4px;
        font-size: 0.875rem;
    }

    .category-select {
        padding: 0.5rem;
        border: 1px solid #d1d5da;
        border-radius: 4px;
        font-size: 0.875rem;
        min-width: 120px;
    }

    .entity-list {
        flex: 1;
        overflow-y: auto;
        overflow-x: hidden;
        padding: 0.5rem;
        min-height: 0;
    }

    .entity-item {
        display: grid;
        grid-template-columns: 2rem 1fr;
        grid-template-rows: auto auto;
        gap: 0.25rem 0.5rem;
        width: 100%;
        padding: 0.5rem;
        border: none;
        background: none;
        text-align: left;
        cursor: pointer;
        border-radius: 4px;
    }

    .entity-item:hover {
        background: #f6f8fa;
    }

    .entity-char {
        grid-row: 1 / 3;
        font-size: 1.5rem;
        font-family: 'Junicode', 'Junicode Two Beta', 'MUFI PUA', serif;
        display: flex;
        align-items: center;
        justify-content: center;
    }

    .entity-name {
        font-family: monospace;
        font-size: 0.875rem;
        color: #005cc5;
        font-weight: 500;
    }

    .entity-desc {
        font-size: 0.75rem;
        color: #666;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }

    .loading,
    .error,
    .no-results {
        padding: 2rem;
        text-align: center;
        color: #666;
    }

    .error {
        color: #cb2431;
    }

    .footer {
        padding: 0.5rem 1rem;
        border-top: 1px solid #e1e4e8;
        font-size: 0.75rem;
        color: #666;
    }
</style>
