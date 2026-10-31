import type { InflectedForm, Segment } from "$lib/tauri";
import { analysisKey } from "$lib/utils/msa";

/** What an imported file says about one of its words */
export interface ImportedWord {
    lemma?: string;
    msa?: string;
    normalized?: string;
    diplomatic?: string;
    facsimile?: string;
}

/** The lemmas an imported file brings along */
export interface ImportedLemmas {
    /** Lemma and analysis by word number */
    confirmations: Record<number, { lemma: string; msa: string }>;
    /** Inflected forms for the dictionary: [wordform, form], one per wordform, lemma and analysis */
    inflections: [string, InflectedForm][];
    /** In a MENOTA file: words without a lemma, and without a normalized form */
    missingLemmas: number;
    missingNormalized: number;
}

/** `text` without tags, whitespace collapsed */
function stripXmlTags(text: string): string {
    return text.replace(/<[^>]+>/g, "").replace(/\s+/g, " ").trim();
}

/** Text of the first `<tagName>` (or MENOTA's `<me:tagName>`) in `xml`, if any */
export function menotaText(xml: string, tagName: string): string | undefined {
    for (const name of [`me:${tagName}`, tagName]) {
        const match = xml.match(new RegExp(`<${name}\\b[^>]*>([\\s\\S]*?)<\\/${name}>`, "i"));
        const text = match ? stripXmlTags(match[1]) : "";
        if (text) return text;
    }
    return undefined;
}

/** What `content` (a word's contents) says, at each level */
function levels(content: string): Omit<ImportedWord, "lemma" | "msa"> {
    return {
        normalized: menotaText(content, "norm"),
        diplomatic: menotaText(content, "dipl"),
        facsimile: menotaText(content, "facs"),
    };
}

/** Each `<w>` in `xml` (words nested in a head, supplied, add, ...), in order */
function nestedWords(xml: string): ImportedWord[] {
    const words: ImportedWord[] = [];
    for (const [, attrs = "", content] of xml.matchAll(/<w(?:\s([^>]*))?>([\s\S]*?)<\/w>/g)) {
        const msa = attrs.match(/me:msa="([^"]*)"|msa="([^"]*)"/);
        words.push({
            lemma: attrs.match(/lemma="([^"]*)"/)?.[1],
            msa: msa ? (msa[1] ?? msa[2]) : undefined,
            ...levels(content),
        });
    }
    return words;
}

/**
 * Every word of an imported document, numbered as the rendered text counts
 * them: words nested in other elements count in document order.
 */
export function importedWords(segments: Segment[]): ImportedWord[] {
    const words: ImportedWord[] = [];
    for (const segment of segments) {
        if (segment.type !== "word") continue;
        const xml = segment.original_xml.trimStart();
        if (xml.startsWith("<w")) {
            words.push({
                lemma: segment.attributes.lemma,
                msa: segment.attributes["me:msa"] ?? segment.attributes.msa,
                ...levels(segment.original_xml),
            });
        } else if (xml.startsWith("<")) {
            words.push(...nestedWords(xml));
        }
    }
    return words;
}

/**
 * The lemmas in an imported document: those with an analysis become the
 * words' confirmed lemmas and teach the dictionary their forms. The file's
 * normalized forms aren't the user's, so they aren't confirmed.
 */
export function importedLemmas(segments: Segment[], isMenota: boolean): ImportedLemmas {
    const result: ImportedLemmas = { confirmations: {}, inflections: [], missingLemmas: 0, missingNormalized: 0 };
    const seen = new Set<string>();
    importedWords(segments).forEach((word, wordIndex) => {
        const { lemma, msa } = word;
        if (isMenota) {
            if (!lemma) result.missingLemmas += 1;
            if (!word.normalized) result.missingNormalized += 1;
        }
        if (!lemma || !msa) return;
        result.confirmations[wordIndex] = { lemma, msa };

        const wordform = (word.diplomatic ?? word.facsimile ?? "").trim();
        const key = `${wordform.toLowerCase()}|${lemma}|${analysisKey(msa)}`;
        if (!wordform || seen.has(key)) return;
        seen.add(key);
        result.inflections.push([
            wordform.toLowerCase(),
            {
                onp_id: `imported:${lemma}`,
                lemma,
                analysis: msa,
                part_of_speech: msa.split(/\s+/)[0] ?? "",
                facsimile: word.facsimile,
                diplomatic: word.diplomatic,
                normalized: word.normalized,
            },
        ]);
    });
    return result;
}
