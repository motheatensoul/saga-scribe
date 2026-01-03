import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import { tags } from '@lezer/highlight';
import { StreamLanguage } from '@codemirror/language';

/**
 * TEI-DSL syntax tokens for custom highlighting
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
]);

/**
 * Simple stream-based parser for TEI-DSL syntax highlighting
 * A full Lezer grammar would be better for production use
 */
const teiDslParser = StreamLanguage.define({
    token(stream) {
        // Page break ///
        if (stream.match('///')) {
            stream.eatWhile(/[^\s]/);
            return 'processingInstruction';
        }

        // Line break //
        if (stream.match('//')) {
            return 'processingInstruction';
        }

        // Abbreviation .abbr[...]{...}
        if (stream.match('.abbr')) {
            return 'keyword';
        }

        // Gap [...] or [...n]
        if (stream.match('[...')) {
            stream.eatWhile(/[0-9]/);
            stream.eat(']');
            return 'processingInstruction';
        }

        // Note ^{...}
        if (stream.match('^{')) {
            stream.eatWhile(/[^}]/);
            stream.eat('}');
            return 'comment';
        }

        // Unclear ?{...}?
        if (stream.match('?{')) {
            stream.eatWhile(/[^}]/);
            stream.eat('}');
            stream.eat('?');
            return 'emphasis';
        }

        // Deletion -{...}-
        if (stream.match('-{')) {
            stream.eatWhile(/[^}]/);
            stream.eat('}');
            stream.eat('-');
            return 'deleted';
        }

        // Addition +{...}+
        if (stream.match('+{')) {
            stream.eatWhile(/[^}]/);
            stream.eat('}');
            stream.eat('+');
            return 'changed';
        }

        // Supplied <...>
        if (stream.eat('<')) {
            stream.eatWhile(/[^>]/);
            stream.eat('>');
            return 'inserted';
        }

        // Bracket content
        if (stream.eat('[') || stream.eat('{')) {
            stream.eatWhile(/[^\]\}]/);
            stream.eat(/[\]\}]/);
            return 'string';
        }

        // Entity :name:
        if (stream.eat(':')) {
            if (stream.eatWhile(/[a-zA-Z0-9_]/)) {
                if (stream.eat(':')) {
                    return 'atom';
                }
            }
            // Not a valid entity, backtrack effect handled by returning null
            return null;
        }

        // Word continuation ~// or ~///
        if (stream.match('~///')) {
            stream.eatWhile(/[^\s]/);
            return 'separator';
        }
        if (stream.match('~//')) {
            return 'separator';
        }

        // Word boundary |
        if (stream.eat('|')) {
            return 'separator';
        }

        // Regular text
        stream.next();
        return null;
    },
});

export const teiDslLanguage = teiDslParser;
export const teiDslHighlighting = syntaxHighlighting(teiDslHighlightStyle);
