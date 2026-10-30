import type { EntityMap } from "$lib/stores/entities";

const XML_ENTITIES: Record<string, string> = { amp: "&", lt: "<", gt: ">", quot: '"', apos: "'" };

/**
 * The normalized form (`<me:norm>` text) of word number `wordIndex` (the n-th
 * `<w>`, as the previews count) in compiled TEI, with entity references
 * resolved; `null` if the word has none (a single-level transcription).
 */
export function wordNormalizedForm(xml: string, wordIndex: number, entities: EntityMap): string | null {
    const starts = /<w(?=[\s>/])/g;
    let match: RegExpExecArray | null = null;
    for (let n = 0; n <= wordIndex; n++) {
        match = starts.exec(xml);
        if (!match) return null;
    }
    const start = match!.index;
    const end = xml.indexOf("</w>", start);
    if (end < 0) return null;

    const norm = /<me:norm\b[^>]*>([\s\S]*?)<\/me:norm>/.exec(xml.slice(start, end));
    if (!norm) return null;
    return resolveEntityReferences(norm[1].replace(/<[^>]*>/g, ""), entities)
        .replace(/\s+/g, " ")
        .trim();
}

/** `text` with XML character and entity references replaced by their characters. */
export function resolveEntityReferences(text: string, entities: EntityMap): string {
    return text.replace(/&(#x[0-9a-fA-F]+|#[0-9]+|[\w.-]+);/g, (ref, name: string) => {
        if (name.startsWith("#x")) return String.fromCodePoint(parseInt(name.slice(2), 16));
        if (name.startsWith("#")) return String.fromCodePoint(parseInt(name.slice(1), 10));
        return XML_ENTITIES[name] ?? entities[name]?.char ?? ref;
    });
}
