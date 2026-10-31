/**
 * Moving annotations along when words are inserted or deleted.
 *
 * Annotations are stored by word number. Each compile reports where the
 * previous compile's words went (see `CompileResult.renumbering`); applying
 * that keeps every annotation on its word. Annotations on deleted words go;
 * a span keeps whatever of it survives.
 */

import type { Annotation, AnnotationSet, AnnotationTarget } from "$lib/types/annotations";

/** For each previous word number, its number now (null: the word is gone). */
export type Renumbering = (number | null)[];

/**
 * A word's number now: undefined if it is gone. Numbers beyond the previous
 * compile's words didn't refer to a word, and are left as they are.
 */
export function renumberWord(map: Renumbering, index: number): number | undefined {
    if (index < 0 || index >= map.length) return index;
    return map[index] ?? undefined;
}

export function renumberTarget(target: AnnotationTarget, map: Renumbering): AnnotationTarget | null {
    switch (target.type) {
        case "word":
        case "char": {
            const wordIndex = renumberWord(map, target.wordIndex);
            return wordIndex === undefined ? null : { ...target, wordIndex };
        }
        case "span": {
            // The span's surviving words, in their new order
            const survivors: number[] = [];
            for (let i = target.startWord; i <= target.endWord; i++) {
                const now = renumberWord(map, i);
                if (now !== undefined) survivors.push(now);
            }
            if (survivors.length === 0) return null;
            return { type: "span", startWord: Math.min(...survivors), endWord: Math.max(...survivors) };
        }
    }
}

/** The annotation on its word's new number, or null if its word is gone. */
export function renumberAnnotation(annotation: Annotation, map: Renumbering): Annotation | null {
    const target = renumberTarget(annotation.target, map);
    if (!target) return null;
    // Lemma IDs are derived from the word number (`lemma-<n>`)
    const id =
        annotation.type === "lemma" && target.type === "word" && annotation.id.startsWith("lemma-")
            ? `lemma-${target.wordIndex}`
            : annotation.id;
    return { ...annotation, id, target };
}

export function renumberSet(set: AnnotationSet, map: Renumbering): AnnotationSet {
    return {
        ...set,
        annotations: set.annotations
            .map((annotation) => renumberAnnotation(annotation, map))
            .filter((annotation): annotation is Annotation => annotation !== null),
    };
}
