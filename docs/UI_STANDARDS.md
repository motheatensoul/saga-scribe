# UI Standards for Saga-Scribe

To ensure a consistent user experience, please adhere to the following UI standards when developing components.

## General Principles

*   **DaisyUI & Tailwind:** Exclusively use Tailwind CSS utility classes and DaisyUI component classes.
*   **Theming:** Never hardcode colors (e.g., `bg-white`, `text-black`). Use semantic color variables (`bg-base-100`, `text-base-content`, `text-primary`, `bg-error`, etc.) to ensure compatibility with both light ("caramellatte") and dark ("coffee") themes.
*   **Icons:** Use `lucide-svelte` for all icons.

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
*Verified: 2026-01-11*
