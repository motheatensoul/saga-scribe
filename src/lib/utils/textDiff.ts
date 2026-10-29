/**
 * The single contiguous edit that turns `before` into `after`, found by
 * trimming the common prefix and suffix. Linear time, so cheap even for
 * multi-megabyte documents; a typical keystroke yields a tiny range.
 *
 * Returns `null` when the strings are equal.
 */
export function changedRange(
    before: string,
    after: string,
): { from: number; to: number; insert: string } | null {
    if (before === after) return null;

    const max = Math.min(before.length, after.length);
    let prefix = 0;
    while (prefix < max && before.charCodeAt(prefix) === after.charCodeAt(prefix)) {
        prefix++;
    }

    let suffix = 0;
    while (
        suffix < max - prefix &&
        before.charCodeAt(before.length - 1 - suffix) ===
            after.charCodeAt(after.length - 1 - suffix)
    ) {
        suffix++;
    }

    return {
        from: prefix,
        to: before.length - suffix,
        insert: after.slice(prefix, after.length - suffix),
    };
}
