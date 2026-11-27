<script lang="ts">
    import { importedWordElements, setImportedElementAttributes, type ElementInfo } from "$lib/tauri";

    let {
        wordIndex,
        imported = true,
        onapplied,
    }: {
        wordIndex: number;
        /** An imported file's word (new documents follow) */
        imported?: boolean;
        /** An element's attributes were set: compile */
        onapplied?: () => void;
    } = $props();

    /** Each element with the attributes as edited */
    let elements = $state<{ info: ElementInfo; attributes: [string, string][] }[]>([]);
    let error = $state<string | null>(null);
    let busy = $state(false);

    async function load(word: number) {
        error = null;
        try {
            const found = await importedWordElements(word);
            elements = found.map((info) => ({ info, attributes: info.attributes.map((a) => [...a] as [string, string]) }));
        } catch (e) {
            elements = [];
            error = String(e);
        }
    }

    $effect(() => {
        if (imported) void load(wordIndex);
    });

    const PLACES: Record<ElementInfo["place"], string> = {
        around: "The word is in",
        before: "Before the word",
        inside: "In the word",
    };

    const changed = (k: number) =>
        JSON.stringify(elements[k].attributes) !== JSON.stringify(elements[k].info.attributes);

    async function apply(k: number) {
        const { info, attributes } = elements[k];
        busy = true;
        error = null;
        try {
            await setImportedElementAttributes(info.name, info.ordinal, attributes.filter(([n]) => n.trim() !== ""));
            onapplied?.();
            await load(wordIndex);
        } catch (e) {
            error = String(e);
        } finally {
            busy = false;
        }
    }
</script>

<div class="flex flex-col gap-3 text-sm">
    {#if !imported}
        <p>Setting the attributes of markup comes to new documents with a later version; in an imported file it works now.</p>
    {:else}
        <p>
            The elements of the file around this word, in it, and right before it (line and page breaks,
            notes), with their attributes. Applying a change sets it in the file.
        </p>
        {#if elements.length === 0 && !error}
            <p class="text-base-content/80">No markup.</p>
        {/if}
        {#each elements as element, k (`${element.info.name}-${element.info.ordinal}`)}
            <fieldset class="flex flex-col gap-1 border border-base-300 rounded p-2">
                <legend class="px-1">
                    <span class="text-base-content/80">{PLACES[element.info.place]}</span>
                    <code class="font-mono text-xs">&lt;{element.info.name}&gt;</code>
                </legend>
                {#each element.attributes as [name, value], i (i)}
                    {@const fixed = element.info.fixed.includes(name)}
                    <div class="flex items-center gap-2">
                        <label class="sr-only" for={`markup-${k}-name-${i}`}>Attribute {i + 1} name</label>
                        <input
                            id={`markup-${k}-name-${i}`}
                            class="input input-bordered input-xs font-mono w-32"
                            value={name}
                            readonly={fixed}
                            onchange={(e) => (element.attributes[i] = [e.currentTarget.value, element.attributes[i][1]])}
                        />
                        <label class="sr-only" for={`markup-${k}-value-${i}`}>Value of {name}</label>
                        <input
                            id={`markup-${k}-value-${i}`}
                            class="input input-bordered input-xs font-mono flex-1"
                            value={value}
                            readonly={fixed}
                            title={fixed ? "Set in the shorthand" : undefined}
                            onchange={(e) => (element.attributes[i] = [element.attributes[i][0], e.currentTarget.value])}
                        />
                        <button
                            type="button"
                            class="btn btn-ghost btn-xs"
                            disabled={fixed}
                            aria-label={`Remove the attribute ${name}`}
                            onclick={() => (element.attributes = element.attributes.filter((_, j) => j !== i))}
                        >
                            Remove
                        </button>
                    </div>
                {/each}
                <div class="flex justify-between gap-2">
                    <button
                        type="button"
                        class="btn btn-ghost btn-xs"
                        onclick={() => (element.attributes = [...element.attributes, ["", ""]])}
                    >
                        Add an attribute
                    </button>
                    <div class="flex gap-2">
                        <button
                            type="button"
                            class="btn btn-ghost btn-xs"
                            disabled={!changed(k) || busy}
                            onclick={() => (element.attributes = element.info.attributes.map((a) => [...a] as [string, string]))}
                        >
                            Undo changes
                        </button>
                        <button type="button" class="btn btn-primary btn-xs" disabled={!changed(k) || busy} onclick={() => apply(k)}>
                            Apply
                        </button>
                    </div>
                </div>
            </fieldset>
        {/each}
        {#if error}
            <p role="alert" class="text-base-content">
                <span class="text-error" aria-hidden="true">●</span> {error}
            </p>
        {/if}
    {/if}
</div>
