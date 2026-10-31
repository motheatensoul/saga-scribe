/**
 * What the rendered text marks on its words (confirmed lemma, edited since
 * import, annotation types), by word number.
 *
 * Word numbers are those of a particular compile: when a word is inserted,
 * the stores are renumbered at once, but the preview may still show the
 * previous output for a moment (it waits for a pause in typing). So the
 * preview takes these marks together with the output they belong to and
 * shows both at the same time, instead of reading the stores live, which
 * marked the words after an edit with their neighbours' lemmas.
 */
import { derived, get } from "svelte/store";
import { annotationTypesByWord, lemmaMappings, type AnnotationType } from "$lib/stores/annotations";
import { importedStore } from "$lib/stores/imported.svelte";

export interface WordMarks {
    confirmed: ReadonlySet<number>;
    edited: ReadonlySet<number>;
    types: ReadonlyMap<number, ReadonlySet<AnnotationType>>;
}

export const NO_MARKS: WordMarks = { confirmed: new Set(), edited: new Set(), types: new Map() };

/** Words with a confirmed lemma */
const confirmedWords = derived(lemmaMappings, ($mappings) => new Set(Object.keys($mappings.mappings).map(Number)));

/** The marks as the stores have them now, numbered by the last compile. */
export function currentMarks(): WordMarks {
    return {
        confirmed: get(confirmedWords),
        edited: importedStore.editedWords,
        types: get(annotationTypesByWord),
    };
}
