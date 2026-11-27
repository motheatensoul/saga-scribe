/**
 * Asks the previews to show a word: the Text view turns to its page and both
 * views scroll it into sight. Used when the word panel is opened from the
 * keyboard (F8), where no click in a view brought the word on screen.
 */
class WordReveal {
    /** The word to show (as the previews number words); `seq` makes a repeat a new request */
    request = $state<{ word: number; seq: number } | null>(null);

    show(word: number) {
        this.request = { word, seq: (this.request?.seq ?? 0) + 1 };
    }
}

export const wordReveal = new WordReveal();

/**
 * The marked word after `from` (`direction` 1) or before it (-1), wrapping
 * around the document; null if none is marked. `from` -1 starts at the
 * beginning (forwards) or the end (backwards).
 */
export function nextMarkedWord(marked: Iterable<number>, from: number, direction: 1 | -1): number | null {
    const words = [...marked].sort((a, b) => a - b);
    if (words.length === 0) return null;
    if (direction === 1) {
        return words.find((w) => w > from) ?? words[0];
    }
    const before = from < 0 ? words : words.filter((w) => w < from);
    return before.length > 0 ? before[before.length - 1] : words[words.length - 1];
}
