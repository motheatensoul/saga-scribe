import {
  HighlightStyle,
  syntaxHighlighting,
  LRLanguage,
  LanguageSupport,
} from "@codemirror/language";
import { tags } from "@lezer/highlight";
import { styleTags } from "@lezer/highlight";
import { parser } from "./tei-dsl-parser.js";

/**
 * TEI-DSL syntax highlighting styles using CSS classes for theme compatibility
 */
export const teiDslHighlightStyle = HighlightStyle.define([
  // Keywords/commands like .abbr
  { tag: tags.keyword, class: "cmt-keyword" },
  // Content in brackets [text] {text}
  { tag: tags.string, class: "cmt-string" },
  // Delimiters like // ///
  { tag: tags.processingInstruction, class: "cmt-processing" },
  // Notes ^{...}
  { tag: tags.comment, class: "cmt-comment" },
  // Unclear ?{...}?
  { tag: tags.emphasis, class: "cmt-emphasis" },
  // Supplied <...>
  { tag: tags.inserted, class: "cmt-inserted" },
  // Deleted -{...}-
  { tag: tags.deleted, class: "cmt-deleted" },
  // Added +{...}+
  { tag: tags.changed, class: "cmt-changed" },
  // Entities :name:
  { tag: tags.atom, class: "cmt-atom" },
  // Word markers ~ |
  { tag: tags.separator, class: "cmt-separator" },
  // Gap [...]
  { tag: tags.special(tags.string), class: "cmt-gap" },
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
      "Abbreviation/...": tags.keyword,
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

      // Word boundary and compound join
      WordBoundary: tags.separator,
      CompoundJoin: tags.separator,
    }),
  ],
});

/**
 * TEI-DSL language definition using Lezer parser
 * Note: Type assertion used to handle version mismatch between
 * @lezer/lr and @codemirror/language's nested @lezer/lr dependency
 */
export const teiDslLanguage = LRLanguage.define({
  parser: parserWithMetadata as any,
  languageData: {
    commentTokens: { block: { open: "^{", close: "}" } },
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
