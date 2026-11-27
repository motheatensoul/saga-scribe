/**
 * Keyboard behaviour of a tab bar (WAI-ARIA tabs pattern), as a Svelte
 * attachment on the `role="tablist"` element: Left/Right arrows move to the
 * previous/next tab (wrapping around), Home and End to the first and last.
 * The tab moved to is focused and clicked, so it is selected at once.
 *
 * The tabs themselves are `role="tab"` buttons with `aria-selected`, and only
 * the selected one has `tabindex="0"` (the others `-1`), so Tab moves from the
 * tab bar to the panel rather than through every tab.
 */
export function tablist(node: HTMLElement): () => void {
    function onKeydown(event: KeyboardEvent) {
        const tabs = Array.from(node.querySelectorAll<HTMLElement>('[role="tab"]:not([disabled])'));
        const current = tabs.indexOf(document.activeElement as HTMLElement);
        if (current < 0) return;
        let next: number;
        switch (event.key) {
            case "ArrowLeft":
                next = (current - 1 + tabs.length) % tabs.length;
                break;
            case "ArrowRight":
                next = (current + 1) % tabs.length;
                break;
            case "Home":
                next = 0;
                break;
            case "End":
                next = tabs.length - 1;
                break;
            default:
                return;
        }
        event.preventDefault();
        tabs[next].focus();
        tabs[next].click();
    }

    node.addEventListener("keydown", onKeydown);
    return () => node.removeEventListener("keydown", onKeydown);
}
