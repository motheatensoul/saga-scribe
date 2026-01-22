<script lang="ts">
    import { entityStore, entityNames, type Entity } from '$lib/stores/entities';
    import { errorStore } from '$lib/stores/errors';
    import {
        saveEntityMapping,
        removeEntityMapping,
        saveCustomEntity,
        removeCustomEntity,
        validateEntityName
    } from '$lib/tauri';
    
    //Icons
    import { X as CloseButton, Plus, Trash2, Pencil } from '@lucide/svelte';

    let { oninsert, onclose }: { oninsert?: (text: string) => void; onclose?: () => void } = $props();

    let searchQuery = $state('');
    let selectedCategory = $state<string | null>(null);
    let selectedEntity = $state<string | null>(null);
    let editingTranslation = $state('');

    // Creation/edit form state
    let showCreateForm = $state(false);
    let editingEntityName = $state<string | null>(null); // null = create mode, string = edit mode
    let newEntityName = $state('');
    let newEntityChar = $state('');
    let newEntityUnicode = $state('');
    let newEntityDesc = $state('');
    let newEntityCat = $state('Custom');
    let creationError = $state<string | null>(null);
    let isCreating = $state(false);

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

    async function handleCreateEntity() {
        creationError = null;

        // Basic validation
        if (!newEntityName || !newEntityChar) {
            creationError = "Name and character are required";
            return;
        }

        // Match backend validation: first char must be letter, rest alphanumeric or underscore
        if (!/^[a-zA-Z][a-zA-Z0-9_]*$/.test(newEntityName)) {
            creationError = "Name must start with a letter and contain only letters, numbers, and underscores";
            return;
        }

        isCreating = true;

        try {
            // Check if name already exists (unless we're editing that same entity)
            const nameExists = newEntityName in $entityStore.entities;
            const isEditingSameName = editingEntityName === newEntityName;

            if (nameExists && !isEditingSameName) {
                creationError = "An entity with this name already exists";
                isCreating = false;
                return;
            }

            // Create entity object
            const entity: Entity = {
                char: newEntityChar,
                unicode: newEntityUnicode || `U+CUSTOM`, // Fallback if not provided
                description: newEntityDesc || "Custom entity",
                category: newEntityCat
            };

            // If editing with a different name, remove the old one first
            if (editingEntityName && editingEntityName !== newEntityName) {
                await removeCustomEntity(editingEntityName);
                entityStore.removeCustomEntity(editingEntityName);
            }

            await saveCustomEntity(
                newEntityName,
                entity.unicode,
                entity.char,
                entity.description,
                entity.category
            );

            // Update store
            entityStore.addCustomEntity(newEntityName, entity);

            // Save name before resetting for search filter
            const savedName = newEntityName;

            // Reset form
            showCreateForm = false;
            editingEntityName = null;
            newEntityName = '';
            newEntityChar = '';
            newEntityUnicode = '';
            newEntityDesc = '';
            newEntityCat = 'Custom';

            // Select the new entity
            selectedCategory = null; // Ensure it's visible
            searchQuery = savedName; // Filter to it
        } catch (e) {
            creationError = String(e);
        } finally {
            isCreating = false;
        }
    }

    async function handleDeleteEntity(name: string) {
        if (!confirm(`Are you sure you want to delete the custom entity :${name}:?`)) return;

        try {
            await removeCustomEntity(name);
            entityStore.removeCustomEntity(name);
            if (selectedEntity === name) selectedEntity = null;
        } catch (e) {
            errorStore.error('Entity Browser', `Failed to delete entity: ${e}`);
        }
    }

    function handleEditEntity(name: string) {
        const entity = $entityStore.customEntities[name];
        if (!entity) return;

        // Pre-fill form with existing values
        editingEntityName = name;
        newEntityName = name;
        newEntityChar = entity.char;
        newEntityUnicode = entity.unicode;
        newEntityDesc = entity.description;
        newEntityCat = entity.category;
        creationError = null;
        showCreateForm = true;
    }

    function handleCancelForm() {
        showCreateForm = false;
        editingEntityName = null;
        newEntityName = '';
        newEntityChar = '';
        newEntityUnicode = '';
        newEntityDesc = '';
        newEntityCat = 'Custom';
        creationError = null;
    }
</script>

<div>
    <div class="flex justify-between items-center mb-4">
        <h2 class="text-lg font-bold">Entity Browser</h2>
        <div class="flex gap-2">
            <button
                class="btn btn-sm btn-ghost gap-2"
                class:btn-active={showCreateForm && !editingEntityName}
                onclick={() => {
                    if (showCreateForm) {
                        handleCancelForm();
                    } else {
                        editingEntityName = null;
                        showCreateForm = true;
                    }
                }}
            >
                <Plus size="16" />
                New Entity
            </button>
            <button class="btn btn-ghost btn-sm btn-circle" onclick={onclose} aria-label="Close">
                <CloseButton size="16" strokeWidth="3" />
            </button>
        </div>
    </div>

    {#if showCreateForm}
        <div class="bg-base-200 p-4 rounded-lg mb-4 grid gap-3">
            <h3 class="font-bold text-sm">{editingEntityName ? 'Edit' : 'Create'} Custom Entity</h3>
            
            <div class="grid grid-cols-2 gap-3">
                <div class="form-control">
                    <label class="label label-text text-xs py-1" for="new-name">Name (without colons)</label>
                    <input 
                        id="new-name"
                        type="text" 
                        bind:value={newEntityName} 
                        class="input input-sm input-bordered font-mono" 
                        placeholder="e.g. mychar"
                    />
                </div>
                <div class="form-control">
                    <label class="label label-text text-xs py-1" for="new-char">Character (Glyph)</label>
                    <input 
                        id="new-char"
                        type="text" 
                        bind:value={newEntityChar} 
                        class="input input-sm input-bordered font-junicode text-lg" 
                        placeholder="e.g. ꝥ"
                    />
                </div>
            </div>

            <div class="grid grid-cols-2 gap-3">
                <div class="form-control">
                    <label class="label label-text text-xs py-1" for="new-uni">Unicode (Optional)</label>
                    <input 
                        id="new-uni"
                        type="text" 
                        bind:value={newEntityUnicode} 
                        class="input input-sm input-bordered" 
                        placeholder="e.g. U+E000"
                    />
                </div>
                <div class="form-control">
                    <label class="label label-text text-xs py-1" for="new-cat">Category</label>
                    <input 
                        id="new-cat"
                        type="text" 
                        bind:value={newEntityCat} 
                        class="input input-sm input-bordered" 
                        list="categories-list"
                    />
                    <datalist id="categories-list">
                        {#each categories as cat}
                            <option value={cat}></option>
                        {/each}
                    </datalist>
                </div>
            </div>

            <div class="form-control">
                <label class="label label-text text-xs py-1" for="new-desc">Description</label>
                <input 
                    id="new-desc"
                    type="text" 
                    bind:value={newEntityDesc} 
                    class="input input-sm input-bordered" 
                    placeholder="Brief description"
                />
            </div>

            {#if creationError}
                <div class="text-error text-xs">{creationError}</div>
            {/if}

            <div class="flex justify-end gap-2 mt-2">
                <button
                    class="btn btn-xs"
                    onclick={handleCancelForm}
                    disabled={isCreating}
                >
                    Cancel
                </button>
                <button
                    class="btn btn-xs btn-primary"
                    onclick={handleCreateEntity}
                    disabled={isCreating || !newEntityName || !newEntityChar}
                >
                    {#if isCreating}Saving...{:else}{editingEntityName ? 'Save Changes' : 'Create Entity'}{/if}
                </button>
            </div>
        </div>
    {/if}

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
                        class="grid grid-cols-[2rem_1fr_auto] grid-rows-2 gap-x-2 gap-y-0.5 w-full p-2 text-left rounded-lg hover:bg-base-200 cursor-pointer group"
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
                            {#if name in $entityStore.customEntities}
                                <span class="badge badge-xs badge-secondary">user</span>
                            {/if}
                            {#if hasCustomMapping}
                                <span class="badge badge-xs badge-accent">mapped</span>
                            {/if}
                        </span>
                        <div class="row-span-2 flex items-center gap-1">
                            {#if name in $entityStore.customEntities}
                                <button
                                    class="btn btn-ghost btn-xs opacity-0 group-hover:opacity-100 transition-opacity"
                                    onclick={(e) => { e.stopPropagation(); handleEditEntity(name); }}
                                    title="Edit custom entity"
                                >
                                    <Pencil size="14" />
                                </button>
                                <button
                                    class="btn btn-ghost btn-xs text-error opacity-0 group-hover:opacity-100 transition-opacity"
                                    onclick={(e) => { e.stopPropagation(); handleDeleteEntity(name); }}
                                    title="Delete custom entity"
                                >
                                    <Trash2 size="14" />
                                </button>
                            {/if}
                            <button
                                class="btn btn-ghost btn-xs"
                                onclick={(e) => { e.stopPropagation(); handleInsert(name); }}
                                title="Insert entity"
                            >
                                <Plus size="16" />
                            </button>
                        </div>
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
