/**
 * Scanning XML source text for its element structure, without parsing it.
 *
 * Shared by the incremental renderers: they find where pages start in the
 * source and which elements are open there, to work on single pages.
 */

/** An element open at some position in the source. */
export interface OpenTag {
    /** Qualified name as written, e.g. `div` or `me:facs` */
    name: string;
    /** The full start tag as written, attributes included */
    tag: string;
}

/** Elements whose content the token extractor doesn't descend into, so a
 *  `<pb/>` inside them does not start a page. */
export const OPAQUE_ELEMENTS = new Set(["w", "pc", "gap", "choice", "unclear", "del", "add", "supplied"]);

/**
 * Scans tags in `xml[from, to)` starting with `stack` open, reporting each
 * page-starting `<pb>` with the elements open at it. Returns the stack at `to`,
 * or null if the markup is unbalanced or can't be scanned.
 */
export function scanTags(
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
export function tagEnd(xml: string, start: number): number {
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

export function tagName(xml: string, at: number): string | null {
    const match = /^[A-Za-z_][\w:.-]*/.exec(xml.slice(at, at + 64));
    return match ? match[0] : null;
}

export function localName(name: string): string {
    const colon = name.indexOf(":");
    return colon === -1 ? name : name.slice(colon + 1);
}

export function attributeValue(tag: string, name: string): string | null {
    const match = new RegExp(`\\s${name}\\s*=\\s*(?:"([^"]*)"|'([^']*)')`).exec(tag);
    return match ? (match[1] ?? match[2]) : null;
}
