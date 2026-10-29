/**
 * Morphosyntactic analyses (`me:msa`, e.g. "xNC gM nS cN sI") as sets of
 * tags. Menota suggests an order for the tags, but it carries no meaning:
 * analyses with the same tags in another order are the same analysis.
 */

/** A key identical for analyses with the same tags in any order. */
export function analysisKey(msa: string): string {
    return msa.split(/\s+/).filter(Boolean).sort().join(" ");
}

export function sameAnalysis(a: string, b: string): boolean {
    return analysisKey(a) === analysisKey(b);
}
