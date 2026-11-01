# UI Standards for Saga-Scribe

To ensure a consistent user experience, please adhere to the following UI standards when developing components.

## General Principles

*   **DaisyUI & Tailwind:** Exclusively use Tailwind CSS utility classes and DaisyUI component classes.
*   **Theming:** Never hardcode colors (e.g., `bg-white`, `text-black`). Use semantic color variables (`bg-base-100`, `text-base-content`, `text-primary`, `bg-error`, etc.) to ensure compatibility with both light ("caramellatte") and dark ("coffee") themes.
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
*   **Destructive Actions:** `btn btn-error` or `btn-ghost text-error`
*   **Icon Buttons:** Use `btn-circle` modifier for standalone icon buttons.
*   **Sizes:** Default to `btn-sm` for UI density in desktop apps. Use `btn-xs` for tight spaces (like inside tables or dense toolbars).

### Typography

*   **Headings:** Use `font-bold`.
*   **Monospace:** Use `font-mono` for code, XML, or technical logs.
*   **Sizes:** Use Tailwind's text scale (`text-xs`, `text-sm`, `text-base`, `text-lg`, `text-xl`).

## CodeMirror Editor

*   The editor theme is centrally defined in `$lib/editor/theme.ts`.
*   Do not manually import or apply other themes (like `one-dark`).
*   The custom theme maps DaisyUI CSS variables to CodeMirror elements, ensuring it adapts to the active DaisyUI theme automatically.

## Svelte 5

*   Use Runes (`$state`, `$derived`, `$effect`, `$props`) for reactivity.
*   Avoid legacy `export let` or `$:`.

---
*Verified: 2026-11-01*
