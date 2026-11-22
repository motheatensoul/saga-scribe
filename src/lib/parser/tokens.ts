import { ContextTracker, ExternalTokenizer, type InputStream } from "@lezer/lr";
import {
    ColumnBreak,
    WordContinuationColumnBreak,
    MarkupOpen,
    MarkupMiddle,
    MarkupClose,
    HandShift,
    Space,
    PageBreak,
    LineBreak,
    WordContinuationPageBreak,
    WordContinuationLineBreak,
    ParagraphBreak,
    headKeyword,
    abbrKeyword,
    suppliedKeyword,
    normKeyword,
    BlockContent,
    BracketContent,
    BraceContent,
    Gap,
    Supplied,
    Deletion,
    Addition,
    Note,
    Unclear,
    Entity,
    Expansion,
    Literal,
    Numeral,
    WordBoundary,
    CompoundJoin,
    Text,
} from "./tei-dsl-parser.terms.js";

/**
 * The DSL's tokens for the editor, found exactly as the compiler's parser
 * finds them (`src-tauri/src/parser/syntax.rs`, whose rules and their order
 * this follows): at each position the first construct that starts there, or
 * text. A construct that starts but doesn't close is text from its opener on,
 * as the parser recovers. The shared corpus (`corpus.test.ts`) checks both
 * agree.
 *
 * Offsets are relative to the token's start, in UTF-16 code units.
 */

type Peek = (offset: number) => number;

/** A construct at a position: its token and length (keyword length for split ones) */
type Found = { token: number; length: number };
/** A construct that starts but fails: its opener is text */
type Failed = { opener: number };

const isWhitespace = (code: number) => code >= 0 && /\s/.test(String.fromCharCode(code)) && code !== 0xfeff;
const isDigit = (code: number) => code >= 48 && code <= 57;
const isNameChar = (cp: number) => cp === 95 || /[\p{Alphabetic}\p{N}]/u.test(String.fromCodePoint(cp));

/** The code point at `offset` and its length in code units (-1, 0 at the end) */
function codePoint(peek: Peek, offset: number): [number, number] {
    const code = peek(offset);
    if (code >= 0xd800 && code < 0xdc00) {
        const low = peek(offset + 1);
        if (low >= 0xdc00 && low < 0xe000) return [(code - 0xd800) * 0x400 + low - 0xdc00 + 0x10000, 2];
    }
    return [code, code < 0 ? 0 : 1];
}

function startsWith(peek: Peek, offset: number, text: string): boolean {
    for (let i = 0; i < text.length; i++) {
        if (peek(offset + i) !== text.charCodeAt(i)) return false;
    }
    return true;
}

/** End of the run of characters matching `test` from `offset` */
function skipWhile(peek: Peek, offset: number, test: (code: number) => boolean): number {
    while (peek(offset) >= 0 && test(peek(offset))) offset++;
    return offset;
}

const BACKSLASH = 92;

/** Past the character escaped by a backslash at `offset` (a backslash escapes a whole code point) */
const escaped = (peek: Peek, offset: number) => offset + codePoint(peek, offset + 1)[1];

/** After the bracket closing with `end` (nested brackets counted), or -1 */
function bracketed(peek: Peek, offset: number, end: string): number {
    const close = end.charCodeAt(0);
    let depth = 1;
    for (let code = peek(offset); code >= 0; code = peek(++offset)) {
        if (code === close) {
            if (--depth === 0) return offset + 1;
        } else if (code === 123 || code === 91 || code === 60) { // { [ <
            depth++;
        } else if ((code === 125 || code === 93 || code === 62) && depth > 1) { // } ] >
            depth--;
        } else if (code === BACKSLASH) {
            offset = escaped(peek, offset);
        }
    }
    return -1;
}

/** After the `}` closing a block (only braces counted), or -1 */
function bracedBlock(peek: Peek, offset: number): number {
    let depth = 1;
    for (let code = peek(offset); code >= 0; code = peek(++offset)) {
        if (code === 125) {
            if (--depth === 0) return offset + 1;
        } else if (code === 123) {
            depth++;
        } else if (code === BACKSLASH) {
            offset = escaped(peek, offset);
        }
    }
    return -1;
}

/** After the next `end`, or -1 */
function until(peek: Peek, offset: number, end: string): number {
    const close = end.charCodeAt(0);
    for (let code = peek(offset); code >= 0; code = peek(++offset)) {
        if (code === close) return offset + 1;
        if (code === BACKSLASH) offset = escaped(peek, offset);
    }
    return -1;
}

const found = (token: number, length: number): Found => ({ token, length });

/** The characters a construct can start with: \\ # / . [ < - + ^ ? { : ~ | * & $ */
const OPENERS = new Set([..."\\#/.[<-+^?{:~|*&$"].map((c) => c.charCodeAt(0)));

/** The first `=` in `from..to` outside brackets and escapes (as the parser's `top_level_equals`), or -1 */
function topLevelEquals(peek: Peek, from: number, to: number): number {
    let depth = 0;
    for (let offset = from; offset < to; offset++) {
        const code = peek(offset);
        if (code === BACKSLASH) offset = escaped(peek, offset);
        else if (code === 123 || code === 91 || code === 60) depth++;
        else if (code === 125 || code === 93 || code === 62) depth = Math.max(0, depth - 1);
        else if (code === 61 && depth === 0) return offset;
    }
    return -1;
}

const hasTopLevelEquals = (peek: Peek, from: number, to: number) => topLevelEquals(peek, from, to) >= 0;

/** Constructs written `.name{` that aren't markup (as the parser's `NOT_MARKUP`) */
const NOT_MARKUP = new Set(["p", "supplied", "norm", "head", "abbr"]);
const isAsciiAlpha = (code: number) => (code >= 65 && code <= 90) || (code >= 97 && code <= 122);
const isAsciiAlnum = (code: number) => isAsciiAlpha(code) || isDigit(code);

/** `.name` before `{` (or `[`) at `offset`: the name, as the parser's `markup_name` */
function markupName(peek: Peek, offset: number): string | null {
    if (peek(offset) !== 46) return null;
    let end = offset + 1;
    let name = "";
    let colon = false;
    for (let code = peek(end); ; code = peek(end)) {
        const ok = name === "" ? isAsciiAlpha(code) : code === 58 && !colon ? (colon = true) : isAsciiAlnum(code);
        if (!ok) break;
        name += String.fromCharCode(code);
        end++;
    }
    if (name === "" || name.endsWith(":") || NOT_MARKUP.has(name)) return null;
    const next = peek(end);
    const takes = next === 91 ? name === "rdg" || name === "space" : next === 123 ? name !== "space" : false;
    return takes ? name : null;
}

/** Markup's parts: its opener's length, and where its content ends (the `=` of a pair, the closer) */
type MarkupParts = { opener: number; middle: number; close: number; closer: number };

/** The markup at `offset` (`.q{`, `*{`, …), as the parser's `named_markup` and `symbol_markup` */
function markupAt(peek: Peek, offset: number): MarkupParts | Failed | Found | null {
    const name = markupName(peek, offset);
    if (name !== null) {
        let from = offset + 1 + name.length;
        if (name === "space") {
            const end = bracketed(peek, from + 1, "]");
            return end < 0 ? { opener: 1 } : found(Space, end - offset);
        }
        if (name === "hs") {
            const end = bracedBlock(peek, from + 1);
            return end < 0 ? { opener: 1 } : found(HandShift, end - offset);
        }
        if (peek(from) === 91) {
            from = bracketed(peek, from + 1, "]");
            if (from < 0 || peek(from) !== 123) return { opener: 1 };
        }
        const end = bracedBlock(peek, from + 1);
        if (end < 0) return { opener: 1 };
        let middle = -1;
        if (name === "corr" || name === "trans") {
            middle = topLevelEquals(peek, from + 1, end - 1);
            if (middle < 0) return { opener: 1 };
        }
        return { opener: from + 1 - offset, middle: middle < 0 ? -1 : middle - offset, close: end - 1 - offset, closer: 1 };
    }
    const symbol = peek(offset);
    if ((symbol === 42 || symbol === 38 || symbol === 36) && peek(offset + 1) === 123) { // * & $
        const end = bracketed(peek, offset + 2, "}");
        if (end < 0) return { opener: 2 };
        if (symbol === 42) return peek(end) === 42 ? { opener: 2, middle: -1, close: end - 1 - offset, closer: 2 } : { opener: 2 };
        return { opener: 2, middle: -1, close: end - 1 - offset, closer: 1 };
    }
    return null;
}

/**
 * Where the markup being read ends: its content is read as the parser reads
 * it, up to there and no further (`bound`, then `close` after a pair's `=`).
 */
class Bound {
    constructor(
        readonly parent: Bound | null,
        /** Where the open part ends: the `=` of a pair, or the closer */
        readonly bound: number,
        /** The length of what is there: `=`, `}` or `}*` */
        readonly length: number,
        readonly token: number,
        /** After a pair's `=`: the closer's place and length */
        readonly then: { bound: number; length: number } | null,
    ) {}

    get hash(): number {
        return (this.parent ? this.parent.hash * 31 : 0) + this.bound * 3 + this.length + 1;
    }
}

export const markupContext = new ContextTracker<Bound | null>({
    start: null,
    shift(context, term, _stack, input) {
        if (term === MarkupOpen) {
            const peek: Peek = (offset) => input.peek(offset);
            const parts = markupAt(peek, 0) as MarkupParts;
            const at = input.pos;
            if (parts.middle >= 0) {
                return new Bound(context, at + parts.middle, 1, MarkupMiddle, { bound: at + parts.close, length: parts.closer });
            }
            return new Bound(context, at + parts.close, parts.closer, MarkupClose, null);
        }
        if (term === MarkupMiddle && context?.then) {
            return new Bound(context.parent, context.then.bound, context.then.length, MarkupClose, null);
        }
        if (term === MarkupClose && context) return context.parent;
        return context;
    },
    hash: (context) => (context ? context.hash : 0),
});

/** The construct starting at `offset`, if any */
function scan(peek: Peek, offset: number): Found | Failed | null {
    if (!OPENERS.has(peek(offset))) return null;
    const at = (text: string) => startsWith(peek, offset, text);
    const block = (token: number, keyword: number) =>
        bracedBlock(peek, offset + keyword + 1) < 0 ? { opener: keyword + 1 } : found(token, keyword);
    // A break and its number: `{any}`, or up to whitespace (page) or digits only (line)
    const breakAt = (token: number, opener: number, page: boolean): Found | Failed => {
        const from = offset + opener;
        if (peek(from) === 123) {
            const end = until(peek, from + 1, "}");
            return end < 0 ? { opener } : found(token, end - offset);
        }
        return found(token, skipWhile(peek, from, page ? (c) => !isWhitespace(c) : isDigit) - offset);
    };
    const inline = (token: number, closing: string | null): Found | Failed => {
        let end = bracketed(peek, offset + 2, "}");
        if (end >= 0 && closing !== null) end = peek(end) === closing.charCodeAt(0) ? end + 1 : -1;
        return end < 0 ? { opener: 2 } : found(token, end - offset);
    };

    // Any character after a backslash is text (a backslash at the very end is itself text)
    if (peek(offset) === BACKSLASH) {
        return peek(offset + 1) < 0 ? null : found(Literal, 1 + codePoint(peek, offset + 1)[1]);
    }
    if (at("#")) {
        const end = until(peek, offset + 1, "#");
        return end < 0 ? { opener: 1 } : found(Numeral, end - offset);
    }
    if (at("///c=")) return breakAt(ColumnBreak, 5, true);
    if (at("///")) return breakAt(PageBreak, 3, true);
    if (at("//")) return breakAt(LineBreak, 2, false);
    // Standalone: whitespace (or nothing) on both sides
    const spaceOrEnd = (code: number) => code < 0 || isWhitespace(code);
    if (at(".p") && spaceOrEnd(peek(offset - 1)) && spaceOrEnd(peek(offset + 2))) {
        return found(ParagraphBreak, 2);
    }
    if (at(".supplied{")) return block(suppliedKeyword, 9);
    if (at(".norm{")) return block(normKeyword, 5);
    if (at(".head{")) return block(headKeyword, 5);
    if (at(".abbr[")) {
        const abbrEnd = bracketed(peek, offset + 6, "]");
        const end = abbrEnd >= 0 && peek(abbrEnd) === 123 ? bracketed(peek, abbrEnd + 1, "}") : -1;
        return end < 0 ? { opener: 6 } : found(abbrKeyword, 5);
    }
    const markup = markupAt(peek, offset);
    if (markup !== null) return "close" in markup ? found(MarkupOpen, markup.opener) : markup;
    if (at("[...")) {
        let end = skipWhile(peek, offset + 4, isDigit);
        if (peek(end) === 60) end = until(peek, end + 1, ">");
        end = end >= 0 && peek(end) === 93 ? end + 1 : -1;
        return end < 0 ? { opener: 4 } : found(Gap, end - offset);
    }
    if (at("<") && !at("<<")) {
        const end = until(peek, offset + 1, ">");
        return end < 0 ? { opener: 1 } : found(Supplied, end - offset);
    }
    if (at("-{")) return inline(Deletion, "-");
    if (at("+{")) return inline(Addition, "+");
    // Free prose ("ð < g"): only braces nest, as in blocks
    if (at("^{")) {
        const end = bracedBlock(peek, offset + 2);
        return end < 0 ? { opener: 2 } : found(Note, end - offset);
    }
    if (at("?{")) return inline(Unclear, "?");
    // {mark=expansion}, or {mark} (the store's expansion)
    if (at("{")) {
        const end = bracketed(peek, offset + 1, "}");
        if (end < 0) return { opener: 1 };
        let blank = true;
        for (let i = offset + 1; i < end - 1; i++) if (!isWhitespace(peek(i))) blank = false;
        return blank && !hasTopLevelEquals(peek, offset + 1, end - 1) ? { opener: 1 } : found(Expansion, end - offset);
    }
    if (at(":")) {
        let end = offset + 1;
        for (let [cp, length] = codePoint(peek, end); length > 0 && isNameChar(cp); [cp, length] = codePoint(peek, end)) {
            end += length;
        }
        return end > offset + 1 && peek(end) === 58 ? found(Entity, end + 1 - offset) : null;
    }
    if (at("~///c=")) return breakAt(WordContinuationColumnBreak, 6, true);
    if (at("~///")) return breakAt(WordContinuationPageBreak, 4, true);
    if (at("~//")) return breakAt(WordContinuationLineBreak, 3, false);
    if (at("~")) return found(CompoundJoin, 1);
    if (at("|")) return found(WordBoundary, 1);
    return null;
}

export const dslTokens = new ExternalTokenizer((input: InputStream, stack) => {
    if (input.next < 0) return;
    // Inside markup, reading stops where its content ends (as the parser's
    // nested parse of the content does)
    const context = stack.context as Bound | null;
    const bound = context ? context.bound - input.pos : Infinity;
    if (bound === 0 && context) {
        input.acceptToken(context.token, context.length);
        return;
    }
    const peek: Peek = (offset) => (offset >= bound ? -1 : input.peek(offset));

    // The parts of an abbreviation or block, after its keyword (checked whole by `scan`)
    if (stack.canShift(BracketContent) && input.next === 91) {
        input.acceptToken(BracketContent, bracketed(peek, 1, "]"));
        return;
    }
    if (stack.canShift(BraceContent) && input.next === 123) {
        input.acceptToken(BraceContent, bracketed(peek, 1, "}"));
        return;
    }
    if (stack.canShift(BlockContent) && input.next === 123) {
        input.acceptToken(BlockContent, bracedBlock(peek, 1));
        return;
    }

    const first = scan(peek, 0);
    if (first && "token" in first) {
        input.acceptToken(first.token, first.length);
        return;
    }
    // Text up to the next construct
    let offset = 0;
    let next: Found | Failed | null = first;
    do {
        offset += next ? (next as Failed).opener : codePoint(peek, offset)[1];
        next = scan(peek, offset);
    } while (peek(offset) >= 0 && !(next && "token" in next));
    input.acceptToken(Text, offset);
}, { contextual: true });
