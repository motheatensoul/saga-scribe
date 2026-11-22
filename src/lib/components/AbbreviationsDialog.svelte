<script lang="ts">
    import { dialogFocus } from "$lib/utils/dialogFocus";
    import { abbreviationStore } from "$lib/stores/abbreviations";
    import { X as CloseButton, Trash2 } from "@lucide/svelte";

    let { isopen = $bindable(false) }: { isopen?: boolean } = $props();

    let filter = $state("");
    let newMark = $state("");
    let newExpansion = $state("");

    let marks = $derived(
        Object.entries($abbreviationStore)
            .filter(([mark, expansions]) => {
                const f = filter.trim().toLowerCase();
                return !f || mark.toLowerCase().includes(f) || expansions.some((e) => e.toLowerCase().includes(f));
            })
            .sort(([a], [b]) => a.localeCompare(b)),
    );

    function add(event: SubmitEvent) {
        event.preventDefault();
        const mark = newMark.trim();
        const expansion = newExpansion.trim();
        if (!mark || !expansion) return;
        abbreviationStore.addExpansion(mark, expansion);
        newMark = "";
        newExpansion = "";
    }

    function close() {
        isopen = false;
    }

    function handleKeydown(e: KeyboardEvent) {
        if (isopen && e.key === "Escape") close();
    }

    function handleBackdropClick(e: MouseEvent) {
        if (e.target === e.currentTarget) close();
    }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if isopen}
    <!-- Clicking outside closes the dialog, like its close button; not needed for keyboard use -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
        onclick={handleBackdropClick}
        role="dialog"
        aria-modal="true"
        aria-labelledby="abbreviations-title"
        tabindex="-1"
        {@attach dialogFocus}
    >
        <div class="bg-base-100 rounded-lg shadow-xl w-full max-w-2xl mx-4 max-h-[90vh] flex flex-col">
            <div class="flex items-center justify-between p-6 border-b border-base-300">
                <h2 id="abbreviations-title" class="text-2xl font-bold">Abbreviation store</h2>
                <button class="btn btn-ghost btn-sm btn-circle" onclick={close} aria-label="Close the abbreviation store">
                    <CloseButton size="16" strokeWidth="3" />
                </button>
            </div>

            <div class="p-6 overflow-y-auto flex-1 space-y-4 text-sm">
                <p>
                    <code class="font-mono">{"{mark}"}</code> in the shorthand stands for the mark's first
                    expansion here. Make another one the default, add expansions or remove them; words
                    that write their expansion out don't change.
                </p>
                <div>
                    <label for="abbreviations-filter" class="font-medium block mb-1">Find a mark or expansion</label>
                    <input
                        id="abbreviations-filter"
                        class="input input-bordered input-sm w-full"
                        type="search"
                        bind:value={filter}
                        data-autofocus
                    />
                </div>

                {#if marks.length === 0}
                    <p class="text-base-content/80">
                        {Object.keys($abbreviationStore).length === 0
                            ? "The store is empty. Importing a TEI file fills it from the file's abbreviations."
                            : "No mark or expansion matches."}
                    </p>
                {:else}
                    <ul class="divide-y divide-base-300">
                        {#each marks as [mark, expansions] (mark)}
                            <li class="py-2">
                                <fieldset>
                                    <legend class="font-mono font-medium">{mark}</legend>
                                    <div class="flex flex-wrap gap-x-4 gap-y-1 mt-1">
                                        {#each expansions as expansion, i (expansion)}
                                            <span class="flex items-center gap-1">
                                                <label class="flex items-center gap-1 cursor-pointer">
                                                    <input
                                                        type="radio"
                                                        class="radio radio-xs"
                                                        name={`default-${mark}`}
                                                        checked={i === 0}
                                                        onchange={() => abbreviationStore.setDefault(mark, expansion)}
                                                    />
                                                    <span class="font-mono">{expansion}</span>
                                                    {#if i === 0}<span class="text-base-content/80">(default)</span>{/if}
                                                </label>
                                                <button
                                                    type="button"
                                                    class="btn btn-ghost btn-xs btn-square"
                                                    aria-label={`Remove ${expansion} from ${mark}`}
                                                    title="Remove"
                                                    onclick={() => abbreviationStore.removeExpansion(mark, expansion)}
                                                >
                                                    <Trash2 size="14" aria-hidden="true" />
                                                </button>
                                            </span>
                                        {/each}
                                    </div>
                                </fieldset>
                            </li>
                        {/each}
                    </ul>
                {/if}

                <form class="flex flex-wrap items-end gap-2 pt-2 border-t border-base-300" onsubmit={add}>
                    <div>
                        <label for="abbreviation-mark" class="font-medium block mb-1">Mark</label>
                        <input id="abbreviation-mark" class="input input-bordered input-sm font-mono" bind:value={newMark} placeholder=":kbar:" />
                    </div>
                    <div>
                        <label for="abbreviation-expansion" class="font-medium block mb-1">Expansion</label>
                        <input id="abbreviation-expansion" class="input input-bordered input-sm font-mono" bind:value={newExpansion} placeholder="konun" />
                    </div>
                    <button class="btn btn-sm btn-primary" type="submit" disabled={!newMark.trim() || !newExpansion.trim()}>Add</button>
                </form>
            </div>
        </div>
    </div>
{/if}
