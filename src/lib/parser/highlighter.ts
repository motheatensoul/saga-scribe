import { HighlightStyle, syntaxHighlighting, LRLanguage, LanguageSupport } from '@codemirror/language';
import { tags } from '@lezer/highlight';
import { styleTags } from '@lezer/highlight';
import { parser } from './tei-dsl-parser.js';

/**
 * TEI-DSL syntax highlighting styles
 */
export const teiDslHighlightStyle = HighlightStyle.define([
    // Keywords/commands like .abbr
    { tag: tags.keyword, color: '#d73a49', fontWeight: 'bold' },
    // Content in brackets [text] {text}
    { tag: tags.string, color: '#032f62' },
    // Delimiters like // ///
    { tag: tags.processingInstruction, color: '#6f42c1' },
    // Notes ^{...}
    { tag: tags.comment, color: '#6a737d', fontStyle: 'italic' },
    // Unclear ?{...}?
    { tag: tags.emphasis, color: '#e36209', fontStyle: 'italic' },
    // Supplied <...>
    { tag: tags.inserted, color: '#22863a' },
    // Deleted -{...}-
    { tag: tags.deleted, color: '#cb2431', textDecoration: 'line-through' },
    // Added +{...}+
    { tag: tags.changed, color: '#22863a', backgroundColor: '#dcffe4' },
    // Entities :name:
    { tag: tags.atom, color: '#005cc5', fontWeight: 'bold' },
    // Word markers ~ |
    { tag: tags.separator, color: '#6f42c1', fontWeight: 'bold' },
    // Gap [...]
    { tag: tags.special(tags.string), color: '#6f42c1', fontStyle: 'italic' },
]);

/**
 * Configure the parser with syntax highlighting tags
 */
const parserWithMetadata = parser.configure({
    props: [
        styleTags({
            // Breaks and continuations
            PageBreak: tags.processingInstruction,
            LineBreak: tags.processingInstruction,
            WordContinuationPageBreak: tags.separator,
            WordContinuationLineBreak: tags.separator,

            // Abbreviation
            'Abbreviation/...': tags.keyword,
            BracketContent: tags.string,
            BraceContent: tags.string,

            // Gap
            Gap: tags.special(tags.string),

            // Supplied
            Supplied: tags.inserted,
            SuppliedContent: tags.inserted,

            // Deletion
            Deletion: tags.deleted,
            DeletionContent: tags.deleted,

            // Addition
            Addition: tags.changed,
            AdditionContent: tags.changed,

            // Note
            Note: tags.comment,
            NoteContent: tags.comment,

            // Unclear
            Unclear: tags.emphasis,
            UnclearContent: tags.emphasis,

            // Entity
            Entity: tags.atom,

            // Word boundary
            WordBoundary: tags.separator,
        }),
    ],
});

/**
 * TEI-DSL language definition using Lezer parser
 */
export const teiDslLanguage = LRLanguage.define({
    parser: parserWithMetadata,
    languageData: {
        commentTokens: { block: { open: '^{', close: '}' } },
    },
});

/**
 * Full language support including the parser and highlighting
 */
export const teiDsl = new LanguageSupport(teiDslLanguage);

/**
 * Syntax highlighting extension
 */
export const teiDslHighlighting = syntaxHighlighting(teiDslHighlightStyle);
