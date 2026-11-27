# UI Standards for Saga-Scribe

To ensure a consistent user experience, please adhere to the following UI standards when developing components.

## General Principles

*   **DaisyUI & Tailwind:** Exclusively use Tailwind CSS utility classes and DaisyUI component classes.
*   **Theming:** Never hardcode colors (e.g., `bg-white`, `text-black`). Use semantic color variables (`bg-base-100`, `text-base-content`, `text-primary`, `bg-error`, etc.) to ensure compatibility with both light ("caramellatte") and dark ("coffee") themes.
*   **Theme changes:** the themes are daisyUI's, with one change in `src/app.css`: in caramellatte,
    whose `primary` is pure black, `primary` takes the theme's own `secondary` (dark brown) and
    `secondary` its `accent` (medium brown). Such overrides are plain rules outside any CSS layer:
    a `@plugin 'daisyui/theme'` override loses to daisyUI's own theme selector. Don't otherwise
    change theme colours to fix contrast; change how they are used (see Accessibility).
*   **Icons:** Use `@lucide/svelte` for all icons (e.g. `import { X as CloseButton } from "@lucide/svelte"`).

## Components

### Modals and Panels

Modals and floating panels (like Settings, Help, Error Log, Validation) should follow this structure:

```svelte
<div class="bg-base-100 rounded-lg shadow-xl ...">
    <!-- Header -->
    <div class="flex items-center justify-between p-4 border-b border-base-300">
        <h2 class="text-xl font-bold">Title</h2>
        <button
            class="btn btn-ghost btn-sm btn-circle"
            onclick={handleClose}
            aria-label="Close"
        >
            <CloseButton size="16" strokeWidth="3" />
        </button>
    </div>

    <!-- Content -->
    <div class="p-4 overflow-y-auto">
        <!-- ... -->
    </div>
</div>
```

*   **Background:** `bg-base-100` (Main background color).
*   **Header:** `border-b border-base-300` separator.
*   **Close Button:** Always use `btn btn-ghost btn-sm btn-circle` with the `X` icon (`CloseButton`) from Lucide, size 16 or 18.

### Toolbar and headers

Actions are placed by what they act on, in groups (`role="group"` with an `aria-label`):

*   **Toolbar** (`Toolbar.svelte`, across the window), left to right:
    1.  **File**: Open, Import, Save, Export menu. Filled `btn-primary` buttons, except Export.
    2.  **Document**: template select, template manager, Metadata, Validate.
    3.  **Lemmatization**: lemma undo and redo.
    4.  **Application** (pushed right with `ml-auto`): log, help, settings, then the file name
        (truncated) and the unsaved-changes mark.
*   **Pane headers** (editor, preview): only actions on that pane's content. The editor
    header has text undo/redo, search, and the entity browser; the preview header switches views.

A new action goes into the group whose scope it shares; a new kind of action gets a new group,
not a button squeezed between groups.

Narrow windows:

*   Text labels are visually hidden below a breakpoint with `sr-only lg:not-sr-only` (or `xl:`,
    `2xl:` for less important labels), so the button keeps its accessible name; give it a
    `title` as well. Icon-only buttons always have an `aria-label`. Icons get `aria-hidden="true"`.
*   Each group is a non-wrapping flex row; the toolbar itself wraps (`flex-wrap`), so groups
    move to a second row whole, never button by button.
*   Ghost buttons on the toolbar's `bg-neutral` use the toolbar's colours (`text-neutral-content`,
    `hover:bg-neutral-content/15`, `disabled:text-neutral-content/40`): daisyUI's ghost colours
    are meant for base backgrounds and are barely visible on neutral.

### Buttons

*   **Primary Actions:** `btn btn-primary`
*   **Secondary Actions:** `btn btn-ghost` or `btn btn-outline`
*   **Destructive Actions:** `btn btn-ghost hover:bg-error/15 focus-visible:bg-error/15`, with a
    trash icon or a label that says what is removed (not red text: see Contrast)
*   **Icon Buttons:** Use `btn-circle` modifier for standalone icon buttons.
*   **Sizes:** Default to `btn-sm` for UI density in desktop apps. Use `btn-xs` for tight spaces (like inside tables or dense toolbars).

### Typography

*   **Headings:** Use `font-bold`.
*   **Monospace:** Use `font-mono` for code, XML, or technical logs.
*   **Sizes:** Use Tailwind's text scale (`text-xs`, `text-sm`, `text-base`, `text-lg`, `text-xl`).

## Accessibility

Svelte's compile-time checks (`bun run check`, 0 warnings) are the floor, not the whole
job. These rules come from the review of November 2026 (see "What was checked" below).

### Dialogs

*   The element that is the dialog (the backdrop wrapper in the component dialogs, the
    `modal-box` in `+page.svelte`) has `role="dialog"`, `aria-modal="true"`,
    `tabindex="-1"`, a name (`aria-labelledby` pointing at its heading, which gets an `id`)
    and `{@attach dialogFocus}` (`$lib/utils/dialogFocus`). That moves focus in when it
    opens, keeps Tab inside, and gives focus back to the opener when it closes.
*   Focus lands on the first control that isn't a close button. Mark a better landing place
    (usually a search field) with `data-autofocus`.
*   Escape closes it (the component's own window handler, or `handleKeydown` in
    `+page.svelte`).

### Tabs

*   `role="tablist"` (with an `aria-label`) and `{@attach tablist}` (`$lib/utils/tabs`) on
    the bar; `role="tab"`, `aria-selected`, `aria-controls` and `tabindex` 0 on the selected
    tab, -1 on the others; `role="tabpanel"` with `aria-labelledby` on the panel. Arrow keys
    move between tabs, Tab moves to the panel.

### Names

*   Icon-only buttons have an `aria-label`; a `title` alone is not read reliably. Inside a
    repeated row, the name says which row: "Remove editor 2", "Insert entity thorn".
*   Icons get `aria-hidden="true"`; badges and icons that carry meaning (error counts,
    valid/invalid) get `sr-only` text.
*   Every form field has a label: a `<label for>`, a wrapping `<label>`, or an `aria-label`.
    A placeholder is not a label.
*   Use `role="listbox"`/`"option"` only for a real single-choice list whose options contain
    no controls; a list of rows with buttons in them is a `<ul>`.

### Focus

*   Focus must be visible on everything. Don't remove the outline without a replacement, and
    show focus with an `outline` (`:focus-visible`), not a border colour, which state classes
    override (as happened with confirmed words in the preview).
*   Controls revealed on hover (`opacity-0 group-hover:opacity-100`) also need
    `group-focus-within:opacity-100 focus-visible:opacity-100`.

### Contrast (WCAG AA)

*   Text needs 4.5:1. Secondary text is `text-base-content/80` or `opacity-80` at the
    faintest; `/70` and lower fail in both themes. On the toolbar (`bg-neutral`), text is
    full `text-neutral-content`; caramellatte's neutral leaves no room for fading.
*   The themes' status colours are left as daisyUI defines them. They are designed as
    surfaces with their `-content` colour on top, not as text colours on base backgrounds:
    in caramellatte, `error` is 2.8:1 on base-100 and `warning` 1.7:1, too light even for
    icons (3:1); `badge-error` and filled `alert-error` are 3.5:1, too little for text.
    So the words carry the meaning and the colour is an accent:
    *   messages (errors, warnings) are base-content text with a coloured icon beside it,
        e.g. `<CircleX class="text-error" aria-hidden="true" />`; in a box, use
        `alert alert-error alert-soft text-base-content` (daisyUI's soft alert colours the
        text itself, which fails; `text-base-content` overrides it). Tints behind
        base-content text stay at 15% or less (`bg-error/15`), which keeps 4.5:1 in both themes;
    *   counts are base-content numbers next to a coloured icon of a distinct shape, not
        numbers in a red badge;
    *   status glyphs (✕, ⚠) may be coloured if the row says the same in text; give them
        `aria-hidden` and an `sr-only` "Error:"/"Warning:";
    *   never `text-error` or `text-warning` on the toolbar.
*   Where text has to be red or yellow (editor highlighting, red ink in the XSLT view),
    use `var(--error-text)` and `var(--warning-text)` (`src/app.css`): the theme colour
    blended towards base-content just enough for 4.5:1 in both themes (55% error, 35%
    warning).
*   Marks (lines, outlines, grips) need 3:1. Those that don't come in a passing theme
    colour use base-content at 60% or more (3.4:1 and 3.3:1), like the dotted line under
    words with a suggested lemma, the wavy line under words edited since import, and the
    pane splitter's grip. Or they blend the theme colour towards base-content just enough:
    notes use 70% `accent`, semantic annotations 55% `secondary` (3.1:1 and 3.0:1 in
    coffee, the tighter theme). In the previews, lemma state and annotations are told
    apart by line style as well as colour: solid (confirmed lemma), dotted (suggested),
    dashed (note).
*   The HTML export has fixed colours on white: greys no lighter than `#666` (5.7:1).
*   Disabled controls are exempt from the contrast requirement.

### The XSLT view and the keyboard

The XSLT view shows what the stylesheet will produce, so its words are whatever markup the
stylesheet writes (spans with a click handler in `simple.xsl`), not buttons, and can't be
reached with the keyboard. That is deliberate: everything the XSLT view offers on a word
(opening the word panel) is also in the Text view, whose words are buttons. Don't add
keyboard handling to stylesheet output; if the XSLT view ever gets functions of its own,
they need a keyboard route outside the rendered text.

### What was checked

Last full check: 2026-11-04.

*   Keyboard: dialogs (settings, help, template manager, metadata editor, log, validation,
    entity browser, word panel), tab bars, the export menu (a native `<details>`; Escape
    closes it and returns to its button), the entity list, the Text view's words and page
    navigation (the XSLT view's words are left out on purpose, see above). Verified by code review and unit tests (`dialogFocus.test.ts`,
    `tabs.test.ts`), not yet by working through the app with the keyboard alone.
*   Names: every `<button>` and form field in `src/` has an accessible name (checked by
    scanning the components).
*   Contrast: both themes' colours from daisyUI, computed for each text/background and
    mark/background pair used in the dialogs, panels, toolbar, the editor's syntax
    highlighting, the Text and XSLT previews, and the HTML export's fixed colours. All
    text and marks found there pass.
*   Not yet checked: screen readers (Orca, VoiceOver, NVDA), 200% zoom and the largest
    editor font. See `TODO.md`.

## CodeMirror Editor

*   The editor theme is centrally defined in `$lib/editor/theme.ts`.
*   Do not manually import or apply other themes (like `one-dark`).
*   The custom theme maps DaisyUI CSS variables to CodeMirror elements, ensuring it adapts to the active DaisyUI theme automatically.

## Svelte 5

*   Use Runes (`$state`, `$derived`, `$effect`, `$props`) for reactivity.
*   Avoid legacy `export let` or `$:`.

---
*Verified: 2026-11-04*
