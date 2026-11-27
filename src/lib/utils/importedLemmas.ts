import type { ImportedWord, InflectedForm } from "$lib/tauri";
import { analysisKey } from "$lib/utils/msa";

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

/**
 * The lemmas in an imported document's `words` (as the backend reads them,
 * numbered as the rendered text counts words): those with an analysis
 * become the words' confirmed lemmas and teach the dictionary their forms.
 * The file's normalized forms aren't the user's, so they aren't confirmed.
 */
export function importedLemmas(words: ImportedWord[], isMenota: boolean): ImportedLemmas {
    const result: ImportedLemmas = { confirmations: {}, inflections: [], missingLemmas: 0, missingNormalized: 0 };
    const seen = new Set<string>();
    words.forEach((word, wordIndex) => {
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
