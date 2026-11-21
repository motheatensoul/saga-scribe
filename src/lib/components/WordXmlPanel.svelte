<script lang="ts">
    import {
        importedWordXml,
        nativeWordXml,
        setImportedWordXml,
        setNativeWordXml,
        type WordXmlChange,
    } from "$lib/tauri";

    let {
        wordIndex,
        imported = true,
        onapplied,
    }: {
        wordIndex: number;
        /** An imported file's word (otherwise a new document's) */
        imported?: boolean;
        /** The word's XML was set: put its shorthand in the editor */
        onapplied?: (change: WordXmlChange) => void;
    } = $props();

    let xml = $state("");
    let original = $state("");
    let error = $state<string | null>(null);
    let busy = $state(false);

    $effect(() => {
        const word = wordIndex;
        error = null;
        (imported ? importedWordXml : nativeWordXml)(word)
            .then((text) => {
                xml = text;
                original = text;
            })
            .catch((e) => (error = String(e)));
    });

    /** What the word is made of, read from its XML */
    let parts = $derived.by(() => {
        const doc = new DOMParser().parseFromString(
            `<x xmlns:me="http://www.menota.org/ns/1.0">${original}</x>`,
            "application/xml",
        );
        const word = doc.documentElement.firstElementChild;
        if (!word || doc.querySelector("parsererror")) return null;
        const levels = ["facs", "dipl", "norm"].filter(
            (level) => word.getElementsByTagName(`me:${level}`).length > 0,
        );
        const attributes = [...word.attributes].map((a) => `${a.name}="${a.value}"`);
        const inline = new Set<string>();
        for (const element of word.getElementsByTagName("*")) {
            if (!["choice", "me:facs", "me:dipl", "me:norm"].includes(element.tagName)) inline.add(element.tagName);
        }
        return { levels, attributes, inline: [...inline] };
    });

    const LEVEL_NAMES: Record<string, string> = { facs: "facsimile", dipl: "diplomatic", norm: "normalized" };

    async function apply() {
        busy = true;
        error = null;
        try {
            const change = await (imported ? setImportedWordXml : setNativeWordXml)(wordIndex, xml);
            original = xml;
            onapplied?.(change);
        } catch (e) {
            error = String(e);
        } finally {
            busy = false;
        }
    }
</script>

<div class="flex flex-col gap-3 text-sm">
    <p>
        The word as the file has it, with everything in it: what the shorthand doesn’t show can be
        changed here. Applying it sets the word in the file, and its shorthand in the editor follows.
    </p>
    {#if parts}
        <dl class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1">
            <dt class="font-medium">Levels</dt>
            <dd>{parts.levels.length ? parts.levels.map((l) => LEVEL_NAMES[l]).join(", ") : "none (the text is the word’s)"}</dd>
            <dt class="font-medium">Attributes</dt>
            <dd class="font-mono text-xs break-all">{parts.attributes.join(" ") || "none"}</dd>
            <dt class="font-medium">Markup</dt>
            <dd class="font-mono text-xs">{parts.inline.map((n) => `<${n}>`).join(" ") || "none"}</dd>
        </dl>
    {/if}
    <label for="word-xml" class="font-medium">XML</label>
    <textarea
        id="word-xml"
        class="textarea textarea-bordered font-mono text-xs h-48 w-full"
        spellcheck="false"
        bind:value={xml}
    ></textarea>
    {#if error}
        <p role="alert" class="text-base-content">
            <span class="text-error" aria-hidden="true">●</span> {error}
        </p>
    {/if}
    <div class="flex justify-end gap-2">
        <button type="button" class="btn btn-sm btn-ghost" disabled={xml === original || busy} onclick={() => (xml = original)}>
            Undo changes
        </button>
        <button type="button" class="btn btn-sm btn-primary" disabled={xml === original || busy} onclick={apply}>
            Apply
        </button>
    </div>
</div>
