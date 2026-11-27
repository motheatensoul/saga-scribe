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

    /** The word's start tag in `xml`: where it is and its attributes as written */
    function startTag(text: string): { end: number; attributes: [string, string][] } | null {
        const match = /^\s*<[^\s>/]+((?:\s+[^\s=]+\s*=\s*(?:"[^"]*"|'[^']*'))*)\s*(\/?)>/.exec(text);
        if (!match) return null;
        const attributes: [string, string][] = [];
        for (const a of match[1].matchAll(/([^\s=]+)\s*=\s*(?:"([^"]*)"|'([^']*)')/g)) {
            attributes.push([a[1], a[2] ?? a[3] ?? ""]);
        }
        return { end: match[0].length, attributes };
    }

    const escapeAttribute = (value: string) =>
        value.replace(/&(?![a-zA-Z][\w.-]*;|#)/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;");

    /** `xml` with the word's attributes set to `attributes` (in that order) */
    function withAttributes(text: string, attributes: [string, string][]): string {
        const tag = startTag(text);
        if (!tag) return text;
        const name = /^\s*<([^\s>/]+)/.exec(text)?.[1] ?? "w";
        const closing = text.slice(0, tag.end).trimEnd().endsWith("/>") ? "/>" : ">";
        const written = attributes
            .filter(([n]) => n.trim() !== "")
            .map(([n, v]) => ` ${n.trim()}="${escapeAttribute(v)}"`)
            .join("");
        const lead = /^\s*/.exec(text)?.[0] ?? "";
        return `${lead}<${name}${written}${closing}${text.slice(tag.end)}`;
    }

    /** The content of the level element `me:<level>` in `xml` (the first), or null */
    function levelContent(text: string, level: string): string | null {
        const match = new RegExp(`<me:${level}(\\s[^>]*)?>([\\s\\S]*?)</me:${level}>`).exec(text);
        return match ? match[2] : null;
    }

    function withLevelContent(text: string, level: string, content: string): string {
        return text.replace(new RegExp(`(<me:${level}(?:\\s[^>]*)?>)[\\s\\S]*?(</me:${level}>)`), (_, open, close) => `${open}${content}${close}`);
    }

    // The attributes and levels as controls, kept in step with the XML text
    let attributes = $derived(startTag(xml)?.attributes ?? []);
    let levelTexts = $derived(
        ["facs", "dipl", "norm"]
            .map((level) => [level, levelContent(xml, level)] as const)
            .filter((entry): entry is readonly [string, string] => entry[1] !== null),
    );

    function setAttribute(index: number, name: string, value: string) {
        const next = attributes.map((a) => [...a] as [string, string]);
        next[index] = [name, value];
        xml = withAttributes(xml, next);
    }
    function removeAttribute(index: number) {
        xml = withAttributes(xml, attributes.filter((_, i) => i !== index));
    }
    function addAttribute() {
        xml = withAttributes(xml, [...attributes, ["name", ""]]);
    }

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
    <fieldset class="flex flex-col gap-1">
        <legend class="font-medium mb-1">Attributes</legend>
        {#each attributes as [name, value], i (i)}
            <div class="flex items-center gap-2">
                <label class="sr-only" for={`word-attribute-name-${i}`}>Attribute {i + 1} name</label>
                <input
                    id={`word-attribute-name-${i}`}
                    class="input input-bordered input-xs font-mono w-32"
                    value={name}
                    onchange={(e) => setAttribute(i, e.currentTarget.value, value)}
                />
                <label class="sr-only" for={`word-attribute-value-${i}`}>Value of {name}</label>
                <input
                    id={`word-attribute-value-${i}`}
                    class="input input-bordered input-xs font-mono flex-1"
                    value={value}
                    onchange={(e) => setAttribute(i, name, e.currentTarget.value)}
                />
                <button type="button" class="btn btn-ghost btn-xs" aria-label={`Remove the attribute ${name}`} onclick={() => removeAttribute(i)}>
                    Remove
                </button>
            </div>
        {/each}
        <div>
            <button type="button" class="btn btn-ghost btn-xs" onclick={addAttribute}>Add an attribute</button>
        </div>
    </fieldset>
    {#if levelTexts.length > 0}
        <fieldset class="flex flex-col gap-1">
            <legend class="font-medium mb-1">Levels</legend>
            {#each levelTexts as [level, content] (level)}
                <label class="flex items-center gap-2">
                    <span class="w-24">{LEVEL_NAMES[level]}</span>
                    <input
                        class="input input-bordered input-xs font-mono flex-1"
                        value={content}
                        onchange={(e) => (xml = withLevelContent(xml, level, e.currentTarget.value))}
                    />
                </label>
            {/each}
        </fieldset>
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
