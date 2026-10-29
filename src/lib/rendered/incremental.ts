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
import { parseXmlAsync, type Page, type ParsedDocument, type TextToken } from "./document";

/** An element open at some position in the source. */
interface OpenTag {
    /** Qualified name as written, e.g. `div` or `me:facs` */
    name: string;
    /** The full start tag as written, attributes included */
    tag: string;
}

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

/** Elements whose content the token extractor doesn't descend into, so a
 *  `<pb/>` inside them does not start a page. */
const OPAQUE_ELEMENTS = new Set(["w", "pc", "gap", "choice", "unclear", "del", "add", "supplied"]);

/** Edits containing any of these can change page structure or how the source is scanned. */
const STRUCTURAL_MARKERS = ["<pb", "<!--", "-->", "<![CDATA[", "]]>", "<?", "<body", "</body"];
/** Longest marker, minus one: how far a marker can reach past the changed range. */
const MARKER_CONTEXT = 8;

export async function renderFull(content: string, entities: EntityMap): Promise<RenderedState> {
    const doc = await parseXmlAsync(content, entities);
    return { content, entities, doc, index: buildIndex(content, doc), mode: "full" };
}

/**
 * Renders `content`, reusing `prev` where possible.
 * Returns `prev` itself when nothing changed.
 */
export async function renderUpdate(
    prev: RenderedState,
    content: string,
    entities: EntityMap,
): Promise<RenderedState> {
    if (entities !== prev.entities) return renderFull(content, entities);
    const change = changedRange(prev.content, content);
    if (!change) return prev;
    return (await tryIncremental(prev, content, change)) ?? renderFull(content, entities);
}

async function tryIncremental(
    prev: RenderedState,
    content: string,
    change: { from: number; to: number; insert: string },
): Promise<RenderedState | null> {
    const index = prev.index;
    if (!index) return null;

    const { from, to, insert } = change;
    // The diff may split a marker across its boundary (`</w><lb` -> `</w><pb/><lb`
    // shares the `<`), so look a few characters beyond the changed range.
    const removed = prev.content.slice(Math.max(0, from - MARKER_CONTEXT), to + MARKER_CONTEXT);
    const inserted = content.slice(Math.max(0, from - MARKER_CONTEXT), from + insert.length + MARKER_CONTEXT);
    if (STRUCTURAL_MARKERS.some((m) => removed.includes(m) || inserted.includes(m))) return null;
    if (to > index.bodyEnd) return null;

    const starts = index.pageStarts;
    // Text inserted right before a page's <pb/> belongs to the previous page
    const first = lastIndexBelow(starts, from);
    if (first < 0) return null;
    const last = Math.max(first, lastIndexBelow(starts, to));
    if (last - first + 1 > MAX_INCREMENTAL_PAGES) return null;

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
    if (!endStack || !sameStack(endStack, expectedEndStack)) return null;

    const firstIsIntro = isIntroPage(prev.doc, first);
    const pageCount = last - first + 1;
    if (breaks.length !== pageCount - (firstIsIntro ? 1 : 0)) return null;

    // Parse just these pages, wrapped in the elements open around them
    const fragmentXml =
        startStack.map((t) => t.tag).join("") +
        content.slice(fragStart, fragEnd) +
        [...endStack].reverse().map((t) => `</${t.name}>`).join("");
    const fragment = await parseXmlAsync(fragmentXml, prev.entities);
    if (fragment.pages.length !== pageCount) return null;
    if (fragment.pages.some((p) => p.pageNumber === "error" || p.pageNumber === "empty")) return null;

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

/**
 * Scans tags in `xml[from, to)` starting with `stack` open, reporting each
 * page-starting `<pb>` with the elements open at it. Returns the stack at `to`,
 * or null if the markup is unbalanced or can't be scanned.
 */
function scanTags(
    xml: string,
    from: number,
    to: number,
    stack: OpenTag[],
    onPageBreak: (offset: number, stack: OpenTag[]) => void,
): OpenTag[] | null {
    const open = [...stack];
    let pos = xml.indexOf("<", from);
    while (pos !== -1 && pos < to) {
        if (xml.startsWith("<!--", pos)) {
            const end = xml.indexOf("-->", pos + 4);
            if (end === -1 || end + 3 > to) return null;
            pos = xml.indexOf("<", end + 3);
            continue;
        }
        if (xml.startsWith("<![CDATA[", pos)) {
            const end = xml.indexOf("]]>", pos + 9);
            if (end === -1 || end + 3 > to) return null;
            pos = xml.indexOf("<", end + 3);
            continue;
        }
        if (xml.startsWith("<?", pos)) {
            const end = xml.indexOf("?>", pos + 2);
            if (end === -1 || end + 2 > to) return null;
            pos = xml.indexOf("<", end + 2);
            continue;
        }

        const end = tagEnd(xml, pos);
        if (end === -1 || end > to) return null;
        const closing = xml[pos + 1] === "/";
        const name = tagName(xml, closing ? pos + 2 : pos + 1);
        if (!name) return null;

        if (closing) {
            if (open.length === 0 || open[open.length - 1].name !== name) return null;
            open.pop();
        } else {
            const selfClosing = xml[end - 2] === "/";
            if (localName(name) === "pb" && !open.some((t) => OPAQUE_ELEMENTS.has(localName(t.name)))) {
                onPageBreak(pos, [...open]);
            }
            if (!selfClosing) open.push({ name, tag: xml.slice(pos, end) });
        }
        pos = xml.indexOf("<", end);
    }
    return open;
}

/** Offset just past the `>` of the tag starting at `start`, honouring quoted attributes. */
function tagEnd(xml: string, start: number): number {
    let quote: string | null = null;
    for (let i = start + 1; i < xml.length; i++) {
        const c = xml[i];
        if (quote) {
            if (c === quote) quote = null;
        } else if (c === '"' || c === "'") {
            quote = c;
        } else if (c === ">") {
            return i + 1;
        } else if (c === "<") {
            return -1;
        }
    }
    return -1;
}

function tagName(xml: string, at: number): string | null {
    const match = /^[A-Za-z_][\w:.-]*/.exec(xml.slice(at, at + 64));
    return match ? match[0] : null;
}

function localName(name: string): string {
    const colon = name.indexOf(":");
    return colon === -1 ? name : name.slice(colon + 1);
}

function attributeValue(tag: string, name: string): string | null {
    const match = new RegExp(`\\s${name}\\s*=\\s*(?:"([^"]*)"|'([^']*)')`).exec(tag);
    return match ? (match[1] ?? match[2]) : null;
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
