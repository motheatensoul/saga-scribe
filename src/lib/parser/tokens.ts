import { ExternalTokenizer, type InputStream } from "@lezer/lr";
import {
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
        }
    }
    return -1;
}

/** After the next `end`, or -1 */
function until(peek: Peek, offset: number, end: string): number {
    const close = end.charCodeAt(0);
    for (let code = peek(offset); code >= 0; code = peek(++offset)) {
        if (code === close) return offset + 1;
    }
    return -1;
}

const found = (token: number, length: number): Found => ({ token, length });

/** The characters a construct can start with: / . [ < - + ^ ? : ~ | */
const OPENERS = new Set([..."/.[<-+^?:~|"].map((c) => c.charCodeAt(0)));

/** The construct starting at `offset`, if any */
function scan(peek: Peek, offset: number): Found | Failed | null {
    if (!OPENERS.has(peek(offset))) return null;
    const at = (text: string) => startsWith(peek, offset, text);
    const block = (token: number, keyword: number) =>
        bracedBlock(peek, offset + keyword + 1) < 0 ? { opener: keyword + 1 } : found(token, keyword);
    const inline = (token: number, closing: string | null): Found | Failed => {
        let end = bracketed(peek, offset + 2, "}");
        if (end >= 0 && closing !== null) end = peek(end) === closing.charCodeAt(0) ? end + 1 : -1;
        return end < 0 ? { opener: 2 } : found(token, end - offset);
    };

    if (at("///")) {
        return found(PageBreak, skipWhile(peek, offset + 3, (c) => !isWhitespace(c)) - offset);
    }
    if (at("//")) {
        return found(LineBreak, skipWhile(peek, offset + 2, isDigit) - offset);
    }
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
    if (at("^{")) return inline(Note, null);
    if (at("?{")) return inline(Unclear, "?");
    if (at(":")) {
        let end = offset + 1;
        for (let [cp, length] = codePoint(peek, end); length > 0 && isNameChar(cp); [cp, length] = codePoint(peek, end)) {
            end += length;
        }
        return end > offset + 1 && peek(end) === 58 ? found(Entity, end + 1 - offset) : null;
    }
    if (at("~///")) {
        return found(WordContinuationPageBreak, skipWhile(peek, offset + 4, (c) => !isWhitespace(c)) - offset);
    }
    if (at("~//")) {
        return found(WordContinuationLineBreak, skipWhile(peek, offset + 3, isDigit) - offset);
    }
    if (at("~")) return found(CompoundJoin, 1);
    if (at("|")) return found(WordBoundary, 1);
    return null;
}

export const dslTokens = new ExternalTokenizer((input: InputStream, stack) => {
    if (input.next < 0) return;
    const peek: Peek = (offset) => input.peek(offset);

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
});
