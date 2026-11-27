/**
 * Keyboard focus for modal dialogs, as a Svelte attachment
 * (`<div role="dialog" tabindex="-1" {@attach dialogFocus}>`):
 *
 * - on opening, focus moves into the dialog: to the element marked
 *   `data-autofocus`, else the first focusable element that isn't a close
 *   button, else the dialog itself;
 * - Tab and Shift+Tab cycle within the dialog;
 * - on closing, focus returns to the element that had it before (the button
 *   that opened the dialog), if it is still in the document.
 */

const FOCUSABLE = [
    "a[href]",
    "button:not([disabled])",
    "input:not([disabled]):not([type='hidden'])",
    "select:not([disabled])",
    "textarea:not([disabled])",
    "summary",
    "[contenteditable='true']",
    "[tabindex]:not([tabindex='-1'])",
].join(",");

/** The focusable elements in `root`, in tab order, leaving out hidden ones. */
export function focusableIn(root: HTMLElement): HTMLElement[] {
    return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
        (el) => !el.closest("[hidden], .hidden, [inert]") && el.getClientRects().length > 0,
    );
}

/** Where focus goes when the dialog opens. */
function initialFocus(node: HTMLElement): HTMLElement {
    const items = focusableIn(node);
    const marked = items.find((el) => el.hasAttribute("data-autofocus"));
    if (marked) return marked;
    // A close button first in the header is a poor landing place: Enter would close
    const first = items.find((el) => !/^close\b/i.test(el.getAttribute("aria-label") ?? ""));
    return first ?? node;
}

export function dialogFocus(node: HTMLElement): () => void {
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;

    // Children may not have rendered their content yet; wait a frame
    const frame = requestAnimationFrame(() => {
        if (!node.contains(document.activeElement)) {
            initialFocus(node).focus();
        }
    });

    function trapTab(event: KeyboardEvent) {
        if (event.key !== "Tab") return;
        const items = focusableIn(node);
        if (items.length === 0) {
            event.preventDefault();
            node.focus();
            return;
        }
        const first = items[0];
        const last = items[items.length - 1];
        const active = document.activeElement;
        if (event.shiftKey && (active === first || active === node)) {
            event.preventDefault();
            last.focus();
        } else if (!event.shiftKey && (active === last || !node.contains(active))) {
            event.preventDefault();
            first.focus();
        }
    }

    node.addEventListener("keydown", trapTab);

    return () => {
        cancelAnimationFrame(frame);
        node.removeEventListener("keydown", trapTab);
        // Restore after the dialog has gone, so nothing in it steals focus back
        queueMicrotask(() => {
            if (previous?.isConnected) previous.focus();
        });
    };
}
