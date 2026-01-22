/**
 * MENOTA morphological annotation constants
 *
 * These constants define the morphological categories used in MENOTA's
 * morphosyntactic annotation (MSA) scheme for Old Norse texts.
 *
 * @see https://www.menota.org/HB3_ch13.xml
 */

export interface MorphologyOption {
    code: string;
    label: string;
}

// MENOTA word classes
export const wordClasses: MorphologyOption[] = [
    { code: 'xNC', label: 'Noun (common)' },
    { code: 'xNP', label: 'Noun (proper)' },
    { code: 'xAJ', label: 'Adjective' },
    { code: 'xPE', label: 'Pronoun (personal)' },
    { code: 'xPR', label: 'Pronoun (reflexive)' },
    { code: 'xPQ', label: 'Pronoun (interrogative)' },
    { code: 'xPI', label: 'Pronoun (indefinite)' },
    { code: 'xDD', label: 'Determiner (demonstrative)' },
    { code: 'xDQ', label: 'Determiner (quantifier)' },
    { code: 'xDP', label: 'Determiner (possessive)' },
    { code: 'xVB', label: 'Verb' },
    { code: 'xAV', label: 'Adverb' },
    { code: 'xAQ', label: 'Adverb (interrogative)' },
    { code: 'xAP', label: 'Preposition' },
    { code: 'xCC', label: 'Conjunction (coordinating)' },
    { code: 'xCS', label: 'Conjunction (subordinating)' },
    { code: 'xIT', label: 'Interjection' },
    { code: 'xIM', label: 'Infinitive marker' },
    { code: 'xUA', label: 'Unassigned' },
];

// Morphological categories
export const cases: MorphologyOption[] = [
    { code: 'cN', label: 'Nominative' },
    { code: 'cG', label: 'Genitive' },
    { code: 'cD', label: 'Dative' },
    { code: 'cA', label: 'Accusative' },
];

export const numbers: MorphologyOption[] = [
    { code: 'nS', label: 'Singular' },
    { code: 'nD', label: 'Dual' },
    { code: 'nP', label: 'Plural' },
];

export const genders: MorphologyOption[] = [
    { code: 'gM', label: 'Masculine' },
    { code: 'gF', label: 'Feminine' },
    { code: 'gN', label: 'Neuter' },
];

export const species: MorphologyOption[] = [
    { code: 'sI', label: 'Indefinite' },
    { code: 'sD', label: 'Definite' },
];

export const grades: MorphologyOption[] = [
    { code: 'rP', label: 'Positive' },
    { code: 'rC', label: 'Comparative' },
    { code: 'rS', label: 'Superlative' },
];

export const persons: MorphologyOption[] = [
    { code: 'p1', label: '1st person' },
    { code: 'p2', label: '2nd person' },
    { code: 'p3', label: '3rd person' },
];

export const tenses: MorphologyOption[] = [
    { code: 'tPS', label: 'Present' },
    { code: 'tPT', label: 'Preterite' },
];

export const moods: MorphologyOption[] = [
    { code: 'mIN', label: 'Indicative' },
    { code: 'mSU', label: 'Subjunctive' },
    { code: 'mIP', label: 'Imperative' },
];

export const voices: MorphologyOption[] = [
    { code: 'vA', label: 'Active' },
    { code: 'vR', label: 'Reflexive (middle)' },
];

export const finitenessOptions: MorphologyOption[] = [
    { code: 'fF', label: 'Finite' },
    { code: 'fP', label: 'Participle' },
    { code: 'fS', label: 'Supine' },
    { code: 'fI', label: 'Infinitive' },
];
