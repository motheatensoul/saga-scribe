import { EditorView } from "@codemirror/view";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { tags as t } from "@lezer/highlight";

export const daisyTheme = EditorView.theme({
  "&": {
    color: "var(--color-base-content)",
    backgroundColor: "var(--color-base-100)",
    fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', 'Courier New', monospace",
  },
  ".cm-content": {
    caretColor: "var(--color-primary)",
  },
  "&.cm-focused .cm-cursor": {
    borderLeftColor: "var(--color-primary)",
  },
  "&.cm-focused .cm-selectionBackground, ::selection": {
    backgroundColor: "color-mix(in oklch, var(--color-primary) 20%, transparent)",
  },
  ".cm-gutters": {
    backgroundColor: "var(--color-base-200)",
    color: "var(--color-base-content)",
    borderRight: "1px solid var(--color-base-300)",
  },
  ".cm-activeLineGutter": {
    backgroundColor: "var(--color-base-300)",
  },
  ".cm-activeLine": {
    backgroundColor: "color-mix(in oklch, var(--color-base-content) 5%, transparent)",
  },
  // Fold gutter styling
  ".cm-foldGutter": {
    width: "1.2em",
  },
  ".cm-foldGutter .cm-gutterElement": {
    cursor: "pointer",
    color: "var(--color-base-content)",
    opacity: "0.5",
    transition: "opacity 0.15s ease",
  },
  ".cm-foldGutter .cm-gutterElement:hover": {
    opacity: "1",
    color: "var(--color-primary)",
  },
  // Collapsed fold placeholder
  ".cm-foldPlaceholder": {
    backgroundColor: "color-mix(in oklch, var(--color-primary) 15%, transparent)",
    border: "1px solid var(--color-primary)",
    borderRadius: "0.25rem",
    padding: "0 0.25rem",
    margin: "0 0.25rem",
    color: "var(--color-primary)",
    cursor: "pointer",
  },
}, { dark: false }); // We let CSS vars handle dark mode via DaisyUI

export const daisyHighlightStyle = HighlightStyle.define([
  { tag: t.keyword, color: "var(--color-primary)" },
  { tag: [t.name, t.deleted, t.character, t.propertyName, t.macroName], color: "var(--color-secondary)" },
  { tag: [t.function(t.variableName), t.labelName], color: "var(--color-accent)" },
  { tag: [t.color, t.constant(t.name), t.standard(t.name)], color: "var(--color-warning)" },
  { tag: [t.definition(t.name), t.separator], color: "var(--color-base-content)" },
  { tag: [t.typeName, t.className, t.number, t.changed, t.annotation, t.modifier, t.self, t.namespace], color: "var(--color-info)" },
  { tag: [t.operator, t.operatorKeyword, t.url, t.escape, t.regexp, t.link, t.special(t.string)], color: "var(--color-success)" },
  { tag: [t.meta, t.comment], color: "color-mix(in oklch, var(--color-base-content) 80%, transparent)", fontStyle: "italic" },
  { tag: t.strong, fontWeight: "bold" },
  { tag: t.emphasis, fontStyle: "italic" },
  { tag: t.strikethrough, textDecoration: "line-through" },
  { tag: t.link, color: "var(--color-primary)", textDecoration: "underline" },
  { tag: t.heading, fontWeight: "bold", color: "var(--color-primary)" },
  { tag: [t.atom, t.bool, t.special(t.variableName)], color: "var(--color-warning)" },
  { tag: [t.processingInstruction, t.string, t.inserted], color: "var(--color-success)" },
  { tag: t.invalid, color: "var(--error-text)" },
]);

export const daisyExtensions = [
    daisyTheme,
    syntaxHighlighting(daisyHighlightStyle),
];
