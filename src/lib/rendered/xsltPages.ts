/**
 * Page-wise rendering of the XSLT preview.
 *
 * Applying a stylesheet to a large manuscript takes seconds, and an edit
 * usually changes one page. Stylesheets that only depend on an element and
 * its ancestors (not on the rest of the document) can declare
 *
 *     <?saga-scribe render="page"?>
 *
 * and are then applied to one page at a time: the body is cut at its page
 * breaks into small, well-formed TEI documents, and only pages whose source
 * changed are transformed again.
 *
 * Elements open across a page break are reopened at the start of the next
 * page's document, with `saga-continued="true"`, so a stylesheet can skip
 * what it outputs at an element's start (like a paragraph mark). Facts about
 * the whole document are passed as parameters: `lineBased` is "1" when the
 * body contains line breaks (`<lb/>`), "" otherwise.
 */

import type { EntityMap } from "$lib/stores/entities";
import { MENOTA_NS, TEI_NS, transformToSafeHtml } from "$lib/utils/xslt";
import { scanTags, tagEnd, type OpenTag } from "./sourceScan";

const PAGE_LOCAL = /<\?saga-scribe\s+render\s*=\s*["']page["']\s*\?>/;

/** Whether a stylesheet declares it may be applied to one page at a time. */
export function isPageLocal(xsl: string): boolean {
    return PAGE_LOCAL.test(xsl);
}

/** A document's body as one TEI document per page. */
export interface PagedSource {
    pieces: string[];
    /** Whether the body contains line breaks (passed as `lineBased`) */
    lineBased: boolean;
}

/**
 * Cuts the body of `content` at its page breaks (the content before the
 * first one is a page too). Returns null if there is no body or its markup
 * can't be scanned; the caller then transforms the whole document.
 */
export function splitPages(content: string): PagedSource | null {
    const lower = content.toLowerCase();
    const bodyStart = lower.search(/<body[\s>]/);
    if (bodyStart === -1) return null;
    const bodyOpenEnd = tagEnd(content, bodyStart);
    const bodyEnd = lower.indexOf("</body>", bodyStart);
    if (bodyOpenEnd === -1 || bodyEnd === -1) return null;

    const bodyTag: OpenTag = { name: content.slice(bodyStart + 1, bodyStart + 5), tag: content.slice(bodyStart, bodyOpenEnd) };
    const starts: Array<{ offset: number; stack: OpenTag[] }> = [{ offset: bodyOpenEnd, stack: [bodyTag] }];
    const endStack = scanTags(content, bodyOpenEnd, bodyEnd, [bodyTag], (offset, stack) =>
        starts.push({ offset, stack }),
    );
    if (!endStack) return null;

    const pieces = starts.map((start, i) => {
        const next = starts[i + 1];
        const source = content.slice(start.offset, next ? next.offset : bodyEnd);
        return pageDocument(start.stack, source, next ? next.stack : endStack);
    });
    // Nothing before the first page break: no page for it
    if (starts.length > 1 && content.slice(bodyOpenEnd, starts[1].offset).trim() === "") {
        pieces.shift();
    }

    const lineBased = /<(?:[\w.-]+:)?lb[\s/>]/.test(content.slice(bodyOpenEnd, bodyEnd));
    return { pieces, lineBased };
}

/** One page as a TEI document: the elements open around it, reopened and closed. */
function pageDocument(open: OpenTag[], source: string, close: OpenTag[]): string {
    const reopened = open.map((t, i) => (i === 0 ? t.tag : continued(t.tag))).join("");
    const closed = [...close].reverse().map((t) => `</${t.name}>`).join("");
    return `<TEI xmlns="${TEI_NS}" xmlns:me="${MENOTA_NS}">${reopened}${source}${closed}</TEI>`;
}

/** A start tag marked as continuing from the previous page. */
function continued(tag: string): string {
    return `${tag.slice(0, -1)} saga-continued="true">`;
}

const yieldToMain = () => new Promise((resolve) => setTimeout(resolve, 0));
/** Transform pages for at most this long before letting the UI catch up. */
const SLICE_MS = 30;

/**
 * Renders every page to sanitized HTML. Pages whose document is in `cache`
 * (from the previous rendering, same stylesheet and entities) are reused.
 * Returns the HTML per page and the cache for the next rendering.
 */
export async function renderPages(
    processor: XSLTProcessor,
    source: PagedSource,
    entities: EntityMap,
    cache: Map<string, string>,
    signal?: AbortSignal,
): Promise<{ html: string[]; cache: Map<string, string> }> {
    processor.setParameter(null, "lineBased", source.lineBased ? "1" : "");
    const next = new Map<string, string>();
    const html: string[] = [];
    let sliceStart = performance.now();
    for (const piece of source.pieces) {
        let pageHtml = next.get(piece) ?? cache.get(piece);
        if (pageHtml === undefined) {
            if (performance.now() - sliceStart > SLICE_MS) {
                await yieldToMain();
                signal?.throwIfAborted();
                sliceStart = performance.now();
            }
            pageHtml = transformToSafeHtml(processor, piece, entities);
        }
        next.set(piece, pageHtml);
        html.push(pageHtml);
    }
    signal?.throwIfAborted();
    return { html, cache: next };
}
