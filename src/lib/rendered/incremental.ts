/**
 * Incremental updates of the rendered-text model.
 *
 * A full parse of a large manuscript takes seconds, but a typical edit only
 * touches one page. Pages start at `<pb/>` elements, so after an edit only the
 * pages containing the changed range are re-parsed, and the word indices of the
 * pages after them are shifted.
 *
 * Every shortcut is guarded: whenever the page structure might have changed
 * (a page break or comment edited, nesting changed, the edit spans many pages)
 * or the source can't be matched to the parsed pages, a full parse is done
 * instead. The incremental result is meant to be indistinguishable from a full
 * parse.
 */

import type { EntityMap } from "$lib/stores/entities";
import { changedRange } from "$lib/utils/textDiff";
import { perfRecord } from "$lib/perf";
import { parseXmlAsync, type Page, type ParsedDocument, type TextToken } from "./document";
import { attributeValue, scanTags, tagEnd, type OpenTag } from "./sourceScan";

/** Where each page starts in the source, and which elements are open there. */
interface SourceIndex {
    pageStarts: number[];
    pageStacks: OpenTag[][];
    /** Offset of the closing `</body>` tag */
    bodyEnd: number;
    /** Elements open just before `</body>` */
    endStack: OpenTag[];
}

/** A rendered document together with what it was rendered from. */
export interface RenderedState {
    content: string;
    entities: EntityMap;
    doc: ParsedDocument;
    /** Absent when the source couldn't be indexed; updates then parse fully */
    index: SourceIndex | null;
    /** How this state was produced (for diagnostics and tests) */
    mode: "full" | "incremental";
}

/** Edits touching more pages than this are re-parsed in full. */
const MAX_INCREMENTAL_PAGES = 8;

/** Gives up on an incremental update (logging why): the caller renders in full. */
function bail(reason: string, fields?: Record<string, string | number>): null {
    perfRecord("preview.rendered.full_reason", 0, { reason, ...fields });
    return null;
}

/** Start of the occurrence of `marker` in `text` that `at` falls inside, if any */
function markerAround(text: string, at: number, marker: string): number | null {
    const start = Math.max(0, at - marker.length + 1);
    const i = text.slice(start, at + marker.length - 1).indexOf(marker);
    return i >= 0 && start + i < at ? start + i : null;
}

/** Occurrences of `marker` in `text` */
function countOf(text: string, marker: string): number {
    let count = 0;
    for (let i = text.indexOf(marker); i >= 0; i = text.indexOf(marker, i + 1)) count++;
    return count;
}

/** Edits containing any of these can change page structure or how the source is scanned. */
const STRUCTURAL_MARKERS = ["<pb", "<!--", "-->", "<![CDATA[", "]]>", "<?", "<body", "</body"];
/** Longest marker, minus one: how far a marker split by the diff can reach past the changed range. */
const MARKER_CONTEXT = 8;

export async function renderFull(content: string, entities: EntityMap, signal?: AbortSignal): Promise<RenderedState> {
    const doc = await parseXmlAsync(content, entities, signal);
    return { content, entities, doc, index: buildIndex(content, doc), mode: "full" };
}

/**
 * Renders `content`, reusing `prev` where possible.
 * Returns `prev` itself when nothing changed. Rejects once `signal` is
 * aborted (a newer document is to be rendered instead).
 */
export async function renderUpdate(
    prev: RenderedState,
    content: string,
    entities: EntityMap,
    signal?: AbortSignal,
): Promise<RenderedState> {
    if (entities !== prev.entities) {
        bail("entities changed");
        return renderFull(content, entities, signal);
    }
    const change = changedRange(prev.content, content);
    if (!change) return prev;
    perfRecord("preview.rendered.change", 0, {
        from: change.from,
        removed: change.to - change.from,
        inserted: change.insert.length,
    });
    return (await tryIncremental(prev, content, change, signal)) ?? renderFull(content, entities, signal);
}

async function tryIncremental(
    prev: RenderedState,
    content: string,
    change: { from: number; to: number; insert: string },
    signal?: AbortSignal,
): Promise<RenderedState | null> {
    const index = prev.index;
    if (!index) return bail("no index");

    // The diff may start a change inside a marker (matching its `<` to
    // another tag's): a pure insertion or deletion slides left out of it
    // while its text allows, anything else is widened to include it
    let { from, to, insert } = change;
    const splits = (at: number) => STRUCTURAL_MARKERS.some((m) => markerAround(prev.content, at, m) !== null);
    if (from === to || insert === "") {
        const deletion = insert === "";
        let text = deletion ? prev.content.slice(from, to) : insert;
        // A deletion's end splits a marker when the diff kept its `<` with the deleted text
        const end = () => (deletion ? from + text.length : from);
        while (from > 0 && text && (splits(from) || splits(end())) && text.endsWith(prev.content[from - 1])) {
            text = prev.content[from - 1] + text.slice(0, -1);
            from--;
        }
        if (deletion) {
            to = from + text.length;
        } else {
            to = from;
            insert = text;
        }
    }
    for (const m of STRUCTURAL_MARKERS) {
        const start = markerAround(prev.content, from, m);
        if (start !== null) {
            insert = prev.content.slice(start, from) + insert;
            from = start;
        }
        const end = markerAround(prev.content, to, m);
        if (end !== null) {
            insert += prev.content.slice(to, end + m.length);
            to = end + m.length;
        }
    }

    // An edit changes a marker if the removed or inserted text holds one, or
    // the text around it holds a different number of them before and after.
    // The diff may split one that stays as it was (both sides have the `<`
    // of `<pb` there), which is no edit of it; an edit next to a page break
    // isn't either.
    const before = prev.content.slice(Math.max(0, from - MARKER_CONTEXT), to + MARKER_CONTEXT);
    const after = content.slice(Math.max(0, from - MARKER_CONTEXT), from + insert.length + MARKER_CONTEXT);
    const removedText = prev.content.slice(from, to);
    const marker = STRUCTURAL_MARKERS.find(
        (m) =>
            removedText.includes(m) ||
            insert.includes(m) ||
            countOf(before, m) !== countOf(after, m),
    );
    if (marker) return bail("structural edit", { marker });
    if (to > index.bodyEnd) return bail("edit after body");

    const starts = index.pageStarts;
    // Text inserted right before a page's <pb/> belongs to the previous page
    const first = lastIndexBelow(starts, from);
    if (first < 0) return bail("edit before first page");
    const last = Math.max(first, lastIndexBelow(starts, to));
    if (last - first + 1 > MAX_INCREMENTAL_PAGES) return bail("too many pages", { pages: last - first + 1 });

    const delta = insert.length - (to - from);
    const fragStart = starts[first];
    const fragEnd = (last + 1 < starts.length ? starts[last + 1] : index.bodyEnd) + delta;
    const startStack = index.pageStacks[first];
    const expectedEndStack = last + 1 < starts.length ? index.pageStacks[last + 1] : index.endStack;

    // Re-scan the edited pages; their nesting at the end must be unchanged
    const breaks: Array<{ offset: number; stack: OpenTag[] }> = [];
    const endStack = scanTags(content, fragStart, fragEnd, startStack, (offset, stack) =>
        breaks.push({ offset, stack }),
    );
    if (!endStack || !sameStack(endStack, expectedEndStack)) return bail("nesting changed");

    const firstIsIntro = isIntroPage(prev.doc, first);
    const pageCount = last - first + 1;
    if (breaks.length !== pageCount - (firstIsIntro ? 1 : 0)) return bail("page breaks changed");

    // Parse just these pages, wrapped in the elements open around them
    const fragmentXml =
        startStack.map((t) => t.tag).join("") +
        content.slice(fragStart, fragEnd) +
        [...endStack].reverse().map((t) => `</${t.name}>`).join("");
    const fragment = await parseXmlAsync(fragmentXml, prev.entities, signal);
    if (fragment.pages.length !== pageCount) return bail("fragment pages differ");
    if (fragment.pages.some((p) => p.pageNumber === "error" || p.pageNumber === "empty")) return bail("fragment did not parse");

    // Word indices: the fragment counted from 0; place it after the preceding words
    const oldPages = prev.doc.pages;
    const wordBase = nextWordIndexBefore(oldPages, first);
    const fragmentWords = nextWordIndexBefore(fragment.pages, fragment.pages.length);
    const oldNextAfter = firstWordIndexFrom(oldPages, last + 1);
    const shift = oldNextAfter === null ? 0 : wordBase + fragmentWords - oldNextAfter;

    const pages: Page[] = [
        ...oldPages.slice(0, first),
        ...fragment.pages.map((p) => shiftWords(p, wordBase)),
        ...oldPages.slice(last + 1).map((p) => shiftWords(p, shift)),
    ];
    let tokenIndex = 0;
    for (let i = 0; i < pages.length; i++) {
        if (i >= first && pages[i].startTokenIndex !== tokenIndex) {
            pages[i] = { ...pages[i], startTokenIndex: tokenIndex };
        }
        tokenIndex += pages[i].tokens.length;
    }

    const fragmentStarts = (firstIsIntro ? [fragStart] : []).concat(breaks.map((b) => b.offset));
    const fragmentStacks = (firstIsIntro ? [startStack] : []).concat(breaks.map((b) => b.stack));

    return {
        content,
        entities: prev.entities,
        doc: {
            pages,
            totalTokens: tokenIndex,
            totalWords: pages.reduce((sum, p) => sum + p.wordCount, 0),
        },
        index: {
            pageStarts: [
                ...starts.slice(0, first),
                ...fragmentStarts,
                ...starts.slice(last + 1).map((s) => s + delta),
            ],
            pageStacks: [
                ...index.pageStacks.slice(0, first),
                ...fragmentStacks,
                ...index.pageStacks.slice(last + 1),
            ],
            bodyEnd: index.bodyEnd + delta,
            endStack: index.endStack,
        },
        mode: "incremental",
    };
}

/**
 * Indexes where each parsed page starts in the source. Returns null unless the
 * page breaks found in the source line up exactly with the parsed pages.
 */
function buildIndex(content: string, doc: ParsedDocument): SourceIndex | null {
    if (doc.pages.length === 0) return null;
    if (doc.pages.some((p) => p.pageNumber === "error" || p.pageNumber === "empty")) return null;

    // Locate <body> exactly as the parser does
    const lower = content.toLowerCase();
    const bodyStart = lower.indexOf("<body");
    if (bodyStart === -1) return null;
    const bodyOpenEnd = tagEnd(content, bodyStart);
    if (bodyOpenEnd === -1) return null;
    const bodyEnd = lower.indexOf("</body>", bodyStart);
    if (bodyEnd === -1) return null;

    const bodyTag: OpenTag = { name: content.slice(bodyStart + 1, bodyStart + 5), tag: content.slice(bodyStart, bodyOpenEnd) };
    const breaks: Array<{ offset: number; stack: OpenTag[]; n: string | null }> = [];
    const endStack = scanTags(content, bodyOpenEnd, bodyEnd, [bodyTag], (offset, stack) =>
        breaks.push({ offset, stack, n: attributeValue(content.slice(offset, tagEnd(content, offset)), "n") }),
    );
    if (!endStack) return null;

    const hasIntro = isIntroPage(doc, 0);
    const pbPages = doc.pages.slice(hasIntro ? 1 : 0);
    if (breaks.length !== pbPages.length) return null;
    if (breaks.some((b, i) => b.n !== null && b.n !== pbPages[i].pageNumber)) return null;

    return {
        pageStarts: (hasIntro ? [bodyOpenEnd] : []).concat(breaks.map((b) => b.offset)),
        pageStacks: (hasIntro ? [[bodyTag]] : []).concat(breaks.map((b) => b.stack)),
        bodyEnd,
        endStack,
    };
}

function sameStack(a: OpenTag[], b: OpenTag[]): boolean {
    return a.length === b.length && a.every((t, i) => t.name === b[i].name && t.tag === b[i].tag);
}

/** Whether page `i` is the content before the first page break. */
function isIntroPage(doc: ParsedDocument, i: number): boolean {
    return i === 0 && doc.pages[0]?.tokens[0]?.type !== "pagebreak";
}

/** Rightmost `i` with `values[i] < limit`, or -1. */
function lastIndexBelow(values: number[], limit: number): number {
    let lo = 0;
    let hi = values.length;
    while (lo < hi) {
        const mid = (lo + hi) >> 1;
        if (values[mid] < limit) lo = mid + 1;
        else hi = mid;
    }
    return lo - 1;
}

/** One past the highest word index in `pages[0, end)`, i.e. the next word's index. */
function nextWordIndexBefore(pages: Page[], end: number): number {
    for (let p = end - 1; p >= 0; p--) {
        const tokens = pages[p].tokens;
        for (let t = tokens.length - 1; t >= 0; t--) {
            const index = tokens[t].wordIndex;
            if (index !== undefined) return index + 1;
        }
    }
    return 0;
}

/** The lowest word index in `pages[start, ...)`, or null if there are no words. */
function firstWordIndexFrom(pages: Page[], start: number): number | null {
    for (let p = start; p < pages.length; p++) {
        for (const token of pages[p].tokens) {
            if (token.wordIndex !== undefined) return token.wordIndex;
        }
    }
    return null;
}

function shiftWords(page: Page, shift: number): Page {
    if (shift === 0) return page;
    return {
        ...page,
        tokens: page.tokens.map((t: TextToken) =>
            t.wordIndex === undefined ? t : { ...t, wordIndex: t.wordIndex + shift },
        ),
    };
}
