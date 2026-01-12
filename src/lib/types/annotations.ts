/**
 * Annotation system types for word-, character-, and span-level annotations.
 * Mirrors the Rust annotations module for type-safe IPC.
 */

/** Types of annotations supported */
export type AnnotationType =
    | "lemma"
    | "semantic"
    | "note"
    | "paleographic"
    | "syntax"
    | "reference"
    | "custom";

/** Target specification for an annotation */
export type AnnotationTarget =
    | { type: "word"; wordIndex: number }
    | { type: "char"; wordIndex: number; charStart: number; charEnd: number }
    | { type: "span"; startWord: number; endWord: number };

/** Types of paleographic observations */
export type PaleographicType =
    | "unclear"
    | "damage"
    | "erasure"
    | "letterform"
    | "abbreviation"
    | "correction"
    | "addition"
    | "decoration"
    | "other";

/** Lemma annotation value */
export interface LemmaValue {
    kind: "lemma";
    lemma: string;
    msa: string;
    normalized?: string;
    onpId?: string;
}

/** Semantic category annotation value */
export interface SemanticValue {
    kind: "semantic";
    category: string;
    subcategory?: string;
    identifier?: string;
    label?: string;
}

/** Free-text note annotation value */
export interface NoteValue {
    kind: "note";
    text: string;
    category?: string;
}

/** Paleographic observation value */
export interface PaleographicValue {
    kind: "paleographic";
    observationType: PaleographicType;
    description?: string;
    certainty?: number;
}

/** Syntactic structure value */
export interface SyntaxValue {
    kind: "syntax";
    function: string;
    details?: string;
}

/** Cross-reference value */
export interface ReferenceValue {
    kind: "reference";
    target: string;
    refType: string;
    label?: string;
}

/** Custom user-defined annotation value */
export interface CustomValue {
    kind: "custom";
    customType: string;
    data: Record<string, string>;
}

/** Union of all annotation value types */
export type AnnotationValue =
    | LemmaValue
    | SemanticValue
    | NoteValue
    | PaleographicValue
    | SyntaxValue
    | ReferenceValue
    | CustomValue
    | MenotaPaleographicValue;

/** Metadata about an annotation */
export interface AnnotationMetadata {
    author?: string;
    created?: string;
    modified?: string;
    confidence?: number;
    source?: string;
    note?: string;
}

/** A single annotation attached to a target in the text */
export interface Annotation {
    id: string;
    type: AnnotationType;
    target: AnnotationTarget;
    value: AnnotationValue;
    metadata?: AnnotationMetadata;
}

/** Collection of all annotations for a document */
export interface AnnotationSet {
    version: string;
    annotations: Annotation[];
}

// ============================================================================
// Helper Functions
// ============================================================================

/** Create a new empty annotation set */
export function createEmptyAnnotationSet(): AnnotationSet {
    return {
        version: "1.0",
        annotations: [],
    };
}

/** Generate a simple unique ID */
function generateId(prefix: string): string {
    const timestamp = Date.now().toString(16);
    const random = Math.floor(Math.random() * 0xffff)
        .toString(16)
        .padStart(4, "0");
    return `${prefix}-${timestamp}${random}`;
}

/** Create a lemma annotation */
export function createLemmaAnnotation(
    wordIndex: number,
    lemma: string,
    msa: string,
    normalized?: string,
): Annotation {
    return {
        id: `lemma-${wordIndex}`,
        type: "lemma",
        target: { type: "word", wordIndex },
        value: { kind: "lemma", lemma, msa, normalized },
    };
}

/** Create a note annotation */
export function createNoteAnnotation(
    target: AnnotationTarget,
    text: string,
    category?: string,
): Annotation {
    return {
        id: generateId("note"),
        type: "note",
        target,
        value: { kind: "note", text, category },
    };
}

/** Create a semantic annotation */
export function createSemanticAnnotation(
    target: AnnotationTarget,
    category: string,
    subcategory?: string,
    label?: string,
): Annotation {
    return {
        id: generateId("sem"),
        type: "semantic",
        target,
        value: { kind: "semantic", category, subcategory, label },
    };
}

/** Create a paleographic annotation */
export function createPaleographicAnnotation(
    target: AnnotationTarget,
    observationType: PaleographicType,
    description?: string,
    certainty?: number,
): Annotation {
    return {
        id: generateId("paleo"),
        type: "paleographic",
        target,
        value: { kind: "paleographic", observationType, description, certainty },
    };
}

/** Create a MENOTA paleographic annotation */
export function createMenotaPaleographicAnnotation(
    target: AnnotationTarget,
    value: Omit<MenotaPaleographicValue, "kind">,
): Annotation {
    return {
        id: generateId("menota"),
        type: "paleographic",
        target,
        value: {
            kind: "menota-paleographic",
            ...value,
        },
    };
}

/** Check if a target includes a specific word index */
export function targetIncludesWord(
    target: AnnotationTarget,
    wordIndex: number,
): boolean {
    switch (target.type) {
        case "word":
            return target.wordIndex === wordIndex;
        case "char":
            return target.wordIndex === wordIndex;
        case "span":
            return wordIndex >= target.startWord && wordIndex <= target.endWord;
    }
}

/** Get the primary word index from a target */
export function getPrimaryWordIndex(target: AnnotationTarget): number {
    switch (target.type) {
        case "word":
        case "char":
            return target.wordIndex;
        case "span":
            return target.startWord;
    }
}

/** Get all annotations for a specific word from a set */
export function getAnnotationsForWord(
    set: AnnotationSet,
    wordIndex: number,
): Annotation[] {
    return set.annotations.filter((a) => targetIncludesWord(a.target, wordIndex));
}

/** Get all annotations of a specific type from a set */
export function getAnnotationsByType(
    set: AnnotationSet,
    type: AnnotationType,
): Annotation[] {
    return set.annotations.filter((a) => a.type === type);
}

/** Get lemma annotations as a map (for backward compatibility) */
export function getLemmaMap(
    set: AnnotationSet,
): Map<number, { lemma: string; msa: string; normalized?: string }> {
    const map = new Map<
        number,
        { lemma: string; msa: string; normalized?: string }
    >();
    for (const ann of set.annotations) {
        if (
            ann.type === "lemma" &&
            ann.target.type === "word" &&
            ann.value.kind === "lemma"
        ) {
            map.set(ann.target.wordIndex, {
                lemma: ann.value.lemma,
                msa: ann.value.msa,
                normalized: ann.value.normalized,
            });
        }
    }
    return map;
}

/** Common semantic categories for Old Norse texts */
export const SEMANTIC_CATEGORIES = [
    { id: "person", label: "Person", subcategories: ["masculine-name", "feminine-name", "patronymic", "nickname", "title"] },
    { id: "place", label: "Place", subcategories: ["settlement", "region", "country", "toponym", "building"] },
    { id: "organization", label: "Organization", subcategories: ["family", "political", "religious"] },
    { id: "event", label: "Event", subcategories: ["battle", "assembly", "voyage", "legal"] },
    { id: "object", label: "Object", subcategories: ["weapon", "ship", "treasure", "tool"] },
    { id: "concept", label: "Concept", subcategories: ["legal-term", "religious", "social"] },
    { id: "divine", label: "Divine/Mythological", subcategories: ["god", "giant", "creature", "realm"] },
] as const;

/** Common note categories */
export const NOTE_CATEGORIES = [
    "editorial",
    "translation",
    "commentary",
    "textual",
    "historical",
    "linguistic",
] as const;

// ============================================================================
// MENOTA-Specific Types (based on MENOTA Handbook v3)
// ============================================================================

/** MENOTA observation types */
export type MenotaObservationType =
    | "unclear"
    | "addition"
    | "deletion"
    | "supplied"
    | "character";

/** MENOTA unclear reading reasons (HB3 ch.9) */
export type MenotaUnclearReason =
    | "illegible"      // Cannot be read at all
    | "faded"          // Ink has faded
    | "smudged"        // Ink is smudged
    | "damage"         // Physical damage to manuscript
    | "erasure"        // Text has been erased
    | "overwriting"    // Text written over other text
    | "binding"        // Hidden in binding
    | "other";

/** MENOTA addition placement (HB3 ch.9) */
export type MenotaAddPlace =
    | "inline"
    | "supralinear"
    | "infralinear"
    | "margin-left"
    | "margin-right"
    | "margin-top"
    | "margin-bottom"
    | "interlinear";

/** MENOTA addition type (HB3 ch.9) */
export type MenotaAddType =
    | "supplement"     // Scribe's own addition
    | "gloss"          // Explanatory gloss
    | "correction";    // Correction by scribe

/** MENOTA deletion rendering (HB3 ch.9) */
export type MenotaDelRend =
    | "overstrike"     // Struck through
    | "erasure"        // Erased
    | "subpunction"    // Dots beneath
    | "expunction"     // Dots above
    | "bracketed";     // Bracketed for deletion

/** MENOTA supplied reason (HB3 ch.9) */
export type MenotaSuppliedReason =
    | "omitted"        // Scribe omitted
    | "damage"         // Lost due to damage
    | "illegible"      // Cannot be read
    | "restoration"    // Editorial restoration
    | "emendation";    // Editorial emendation

/** MENOTA character type for <c> element (HB3 ch.4) */
export type MenotaCharType =
    | "initial"        // Decorated initial
    | "capital"        // Capital letter (littera notabilior)
    | "rubric"         // Rubricated character
    | "colored";       // Other colored character

/** MENOTA-specific paleographic value with proper attributes */
export interface MenotaPaleographicValue {
    kind: "menota-paleographic";
    /** The type of observation */
    observationType: MenotaObservationType;
    /** For unclear: the reason */
    unclearReason?: MenotaUnclearReason;
    /** For addition: placement */
    addPlace?: MenotaAddPlace;
    /** For addition: type */
    addType?: MenotaAddType;
    /** For addition/deletion: hand identifier */
    hand?: string;
    /** For deletion: rendering method */
    delRend?: MenotaDelRend;
    /** For supplied: reason */
    suppliedReason?: MenotaSuppliedReason;
    /** For supplied: responsibility */
    resp?: string;
    /** For supplied: source */
    source?: string;
    /** For character: type (initial, capital, etc.) */
    charType?: MenotaCharType;
    /** For character: size (for initials) */
    charSize?: number;
    /** General description */
    description?: string;
    /** Certainty level (0-1) */
    certainty?: number;
}

/** MENOTA unclear reason options for UI */
export const MENOTA_UNCLEAR_REASONS: { id: MenotaUnclearReason; label: string }[] = [
    { id: "illegible", label: "Illegible" },
    { id: "faded", label: "Faded ink" },
    { id: "smudged", label: "Smudged" },
    { id: "damage", label: "Physical damage" },
    { id: "erasure", label: "Erasure" },
    { id: "overwriting", label: "Overwriting" },
    { id: "binding", label: "Hidden in binding" },
    { id: "other", label: "Other" },
];

/** MENOTA addition placement options for UI */
export const MENOTA_ADD_PLACES: { id: MenotaAddPlace; label: string }[] = [
    { id: "inline", label: "Inline" },
    { id: "supralinear", label: "Above the line" },
    { id: "infralinear", label: "Below the line" },
    { id: "margin-left", label: "Left margin" },
    { id: "margin-right", label: "Right margin" },
    { id: "margin-top", label: "Top margin" },
    { id: "margin-bottom", label: "Bottom margin" },
    { id: "interlinear", label: "Between lines" },
];

/** MENOTA deletion rendering options for UI */
export const MENOTA_DEL_RENDS: { id: MenotaDelRend; label: string }[] = [
    { id: "overstrike", label: "Struck through" },
    { id: "erasure", label: "Erased" },
    { id: "subpunction", label: "Dots beneath" },
    { id: "expunction", label: "Dots above" },
    { id: "bracketed", label: "Bracketed" },
];

/** MENOTA character type options for UI */
export const MENOTA_CHAR_TYPES: { id: MenotaCharType; label: string }[] = [
    { id: "initial", label: "Decorated initial" },
    { id: "capital", label: "Capital letter" },
    { id: "rubric", label: "Rubricated" },
    { id: "colored", label: "Colored" },
];

