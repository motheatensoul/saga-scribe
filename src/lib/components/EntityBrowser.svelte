<script lang="ts">
    import { entityStore, entityNames, type Entity } from '$lib/stores/entities';
    import { saveEntityMapping, removeEntityMapping } from '$lib/tauri';
    
    //Icons
    import { X as CloseButton } from '@lucide/svelte';

    let { oninsert, onclose }: { oninsert?: (text: string) => void; onclose?: () => void } = $props();

    let searchQuery = $state('');
    let selectedCategory = $state<string | null>(null);
    let selectedEntity = $state<string | null>(null);
    let editingTranslation = $state('');

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

    function handleSelect(name: string) {
        if (selectedEntity === name) {
            selectedEntity = null;
        } else {
            selectedEntity = name;
            // Initialize editing value with custom mapping, base mapping, or entity char
            editingTranslation = $entityStore.customMappings[name] ?? $entityStore.baseMappings[name] ?? $entityStore.entities[name]?.char ?? '';
        }
    }

    async function handleSaveMapping(name: string) {
        if (!editingTranslation.trim()) return;

        try {
            await saveEntityMapping(name, editingTranslation.trim());
            entityStore.setCustomMapping(name, editingTranslation.trim());
        } catch (e) {
            console.error('Failed to save mapping:', e);
        }
    }

    async function handleResetMapping(name: string) {
        try {
            await removeEntityMapping(name);
            entityStore.removeCustomMapping(name);
            // Reset editing field to base mapping or original char
            editingTranslation = $entityStore.baseMappings[name] ?? $entityStore.entities[name]?.char ?? '';
        } catch (e) {
            console.error('Failed to remove mapping:', e);
        }
    }

    function handleKeydown(e: KeyboardEvent, name: string) {
        if (e.key === 'Enter') {
            handleSaveMapping(name);
        }
    }

    function handleEntityKeydown(e: KeyboardEvent, name: string) {
        if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            handleSelect(name);
        }
    }
</script>

<div>
    <div class="flex justify-between items-center mb-4">
        <h2 class="text-lg font-bold">Entity Browser</h2>
        <button class="btn btn-ghost btn-sm btn-circle" onclick={onclose} aria-label="Close">
            <CloseButton size="16" strokeWidth="3" />
        </button>
    </div>

    <div class="flex gap-2 mb-4">
        <input
            type="text"
            placeholder="Search entities..."
            bind:value={searchQuery}
            class="input input-bordered input-sm flex-1"
        />
        <select bind:value={selectedCategory} class="select select-bordered select-sm min-w-32">
            <option value={null}>All categories</option>
            {#each categories as category}
                <option value={category}>{category}</option>
            {/each}
        </select>
    </div>

    {#if !$entityStore.loaded}
        <div class="text-center py-8 opacity-70">Loading entities...</div>
    {:else if $entityStore.error}
        <div class="text-center py-8 text-error">{$entityStore.error}</div>
    {:else}
        <div class="overflow-y-auto max-h-96 -mx-2" role="listbox" aria-label="Entity list">
            {#each filteredEntities as [name, entity]}
                {@const hasCustomMapping = name in $entityStore.customMappings}
                {@const isSelected = selectedEntity === name}
                <div class="border-b border-base-200 last:border-b-0">
                    <div
                        class="grid grid-cols-[2rem_1fr_auto] grid-rows-2 gap-x-2 gap-y-0.5 w-full p-2 text-left rounded-lg hover:bg-base-200 cursor-pointer"
                        role="option"
                        aria-selected={isSelected}
                        tabindex="0"
                        class:bg-base-200={isSelected}
                        onclick={() => handleSelect(name)}
                        onkeydown={(e) => handleEntityKeydown(e, name)}
                        title={entity.description}
                    >
                        <span class="row-span-2 text-2xl flex items-center justify-center" style="font-family: 'Junicode', serif;">{entity.char}</span>
                        <span class="font-mono text-sm text-primary font-medium flex items-center gap-2">
                            :{name}:
                            {#if hasCustomMapping}
                                <span class="badge badge-xs badge-accent">custom</span>
                            {/if}
                        </span>
                        <button
                            class="row-span-2 btn btn-ghost btn-xs"
                            onclick={(e) => { e.stopPropagation(); handleInsert(name); }}
                            title="Insert entity"
                        >
                            +
                        </button>
                        <span class="text-xs opacity-70 truncate">{entity.description}</span>
                    </div>

                    {#if isSelected}
                        <div class="px-2 pb-2 pt-1 bg-base-200 rounded-b-lg">
                            <div class="flex items-center gap-2">
                                <span class="text-xs opacity-70 whitespace-nowrap">Translation:</span>
                                <input
                                    type="text"
                                    bind:value={editingTranslation}
                                    onblur={() => handleSaveMapping(name)}
                                    onkeydown={(e) => handleKeydown(e, name)}
                                    class="input input-bordered input-xs flex-1 font-mono"
                                    placeholder="Base letter..."
                                />
                                {#if hasCustomMapping}
                                    <button
                                        class="btn btn-ghost btn-xs"
                                        onclick={() => handleResetMapping(name)}
                                        title="Reset to default"
                                    >
                                        Reset
                                    </button>
                                {/if}
                            </div>
                            <div class="text-xs opacity-50 mt-1">
                                Default: {$entityStore.baseMappings[name] ?? entity.char} | Char: {entity.char} | Unicode: {entity.unicode}
                            </div>
                        </div>
                    {/if}
                </div>
            {:else}
                <div class="text-center py-8 opacity-70">No entities found</div>
            {/each}
        </div>
    {/if}

    <div class="pt-4 border-t border-base-300 text-xs opacity-70">
        {filteredEntities.length} entities
        {#if Object.keys($entityStore.customMappings).length > 0}
            | {Object.keys($entityStore.customMappings).length} custom mappings
        {/if}
    </div>
</div>
