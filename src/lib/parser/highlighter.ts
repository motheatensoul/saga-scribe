import {
  HighlightStyle,
  syntaxHighlighting,
  LRLanguage,
  LanguageSupport,
  foldService,
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
 * Configure the parser with syntax highlighting tags and fold regions
 */
const parserWithMetadata = parser.configure({
  props: [
    styleTags({
      // Breaks and continuations
      PageBreak: tags.processingInstruction,
      LineBreak: tags.processingInstruction,
      WordContinuationPageBreak: tags.separator,
      WordContinuationLineBreak: tags.separator,

      // Heading
      "Head/...": tags.keyword,

      // Abbreviation
      "Abbreviation/...": tags.keyword,
      BracketContent: tags.string,
      BraceContent: tags.string,

      // Supplied block
      "SuppliedBlock/...": tags.inserted,

      // Normalized-only
      "NormBlock/...": tags.keyword,

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

/**
 * Fold service for page breaks - allows folding content between /// markers
 * This is separate from the language so it can be added as an extension
 */
export const teiDslFolding = foldService.of((state, lineStart, _lineEnd) => {
  // Get the text of the current line
  const line = state.doc.lineAt(lineStart);
  const lineText = state.sliceDoc(line.from, line.to);

  // Check if this line starts with /// (page break) but not ~/// (word continuation)
  const trimmed = lineText.trimStart();
  if (!trimmed.startsWith("///")) {
    return null;
  }

  // Make sure it's not preceded by ~ on the same line
  const prefixLength = lineText.length - trimmed.length;
  if (prefixLength > 0 && lineText[prefixLength - 1] === "~") {
    return null;
  }

  // Find the next /// in the document after this line
  const docText = state.doc.toString();
  const searchStart = line.to + 1; // Start after this line

  let nextPageBreakPos: number | null = null;
  let searchPos = searchStart;

  while (searchPos < docText.length) {
    const idx = docText.indexOf("///", searchPos);
    if (idx === -1) break;

    // Make sure it's not a word continuation (~///)
    if (idx === 0 || docText[idx - 1] !== "~") {
      // Also check it's at the start of a line (or after whitespace)
      const charBefore = idx > 0 ? docText[idx - 1] : "\n";
      if (charBefore === "\n" || charBefore === " " || charBefore === "\t") {
        nextPageBreakPos = idx;
        break;
      }
    }
    searchPos = idx + 1;
  }

  // Calculate fold range: from end of current line to start of next page break line
  const foldFrom = line.to;
  let foldTo: number;

  if (nextPageBreakPos !== null) {
    // Fold to start of the line containing the next page break
    foldTo = state.doc.lineAt(nextPageBreakPos).from;
  } else {
    // No next page break - fold to end of document
    foldTo = state.doc.length;
  }

  // Only create fold if there's actual content to fold
  if (foldTo > foldFrom + 1) {
    return { from: foldFrom, to: foldTo };
  }

  return null;
});
