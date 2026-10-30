/**
 * Whether the user is typing, so that work which would compete with it for
 * the main thread (redrawing the previews) can wait for a pause. Typing
 * must never wait for a preview: a keystroke that doesn't show up at once
 * makes transcribing in the app impossible.
 */

/** How long after the last edit counts as a pause */
export const TYPING_PAUSE_MS = 200;

let lastEdit = Number.NEGATIVE_INFINITY;

/** Records that the user just edited the document. */
export function markEdit(now = performance.now()) {
    lastEdit = now;
}

/**
 * Runs `work` once no edit has been made for `pauseMs` (at once if none
 * was made that recently). Returns a function that cancels it.
 */
export function whenTypingPauses(work: () => void, pauseMs = TYPING_PAUSE_MS): () => void {
    let timer: ReturnType<typeof setTimeout> | null = null;
    const check = () => {
        const wait = lastEdit + pauseMs - performance.now();
        if (wait > 0) {
            timer = setTimeout(check, wait);
        } else {
            timer = null;
            work();
        }
    };
    check();
    return () => {
        if (timer !== null) clearTimeout(timer);
    };
}
