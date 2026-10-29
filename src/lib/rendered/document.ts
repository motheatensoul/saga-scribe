/**
 * Rendered-text model: turns compiled TEI-XML into display tokens grouped by
 * manuscript page (split at <pb/>).
 */

import type { EntityMap } from "$lib/stores/entities";

export interface TextToken {
    type: 'word' | 'punctuation' | 'linebreak' | 'pagebreak' | 'space' | 'text';
    displayText: string;  // What to show (facsimile with resolved entities)
    diplomatic?: string;  // For lemmatization lookup
    wordIndex?: number;   // The word's index in the document (for per-instance lemmatization)
    lineNumber?: string;
    pageNumber?: string;
    hasError?: boolean;
    errorTooltip?: string;
}

export interface Page {
    pageNumber: string;        // e.g., "1r", "1v", or "intro" for content before first pb
    startTokenIndex: number;   // Absolute token index for quick lookups
    tokens: TextToken[];
    wordCount: number;
}

export interface ParsedDocument {
    pages: Page[];
    totalTokens: number;
    totalWords: number;
}

const yieldToMain = () => new Promise((resolve) => setTimeout(resolve, 0));

function sliceTag(xml: string, tagName: string): string | null {
    const lower = xml.toLowerCase();
    const start = lower.indexOf(`<${tagName}`);
    if (start === -1) return null;
    const end = lower.indexOf(`</${tagName}>`, start);
    if (end === -1) return null;
    return xml.slice(start, end + tagName.length + 3);
}

// Async version of parseXml that yields to main thread periodically
// Returns ParsedDocument with tokens grouped by page
export async function parseXmlAsync(xml: string, entityDefs: EntityMap): Promise<ParsedDocument> {
    const emptyDoc: ParsedDocument = { pages: [], totalTokens: 0, totalWords: 0 };
    if (!xml.trim()) return emptyDoc;

    try {
        // Strip XML declaration if present (breaks when wrapped)
        let cleanXml = xml.replace(/<\?xml[^?]*\?>\s*/gi, '');

        // Strip DOCTYPE/entity declarations (not supported when wrapped)
        cleanXml = cleanXml
            .replace(/<!DOCTYPE[\s\S]*?\]>\s*/gi, '')
            .replace(/<!DOCTYPE[^>]*>\s*/gi, '');

        const bodyFragment = sliceTag(cleanXml, 'body') ?? sliceTag(cleanXml, 'text');
        const xmlFragment = bodyFragment ?? cleanXml;

        // Replace entity references with placeholders to avoid XML parsing errors
        const entityPattern = /&([a-zA-Z][a-zA-Z0-9]*);/g;
        const placeholderGlyphs = new Map<string, string>();
        let entityCounter = 0;

        const sanitized = xmlFragment.replace(entityPattern, (match, name) => {
            const placeholder = `__ENTITY_${entityCounter}__`;
            const glyph = entityDefs[name]?.char || `[${name}]`;
            placeholderGlyphs.set(placeholder, glyph);
            entityCounter++;
            return placeholder;
        });

        // Wrap in a root element with namespace declarations
        const wrapped = `<root xmlns:me="http://www.menota.org/ns/1.0">${sanitized}</root>`;
        const parser = new DOMParser();
        const doc = parser.parseFromString(wrapped, 'text/xml');

        const parseError = doc.querySelector('parsererror');
        if (parseError) {
            console.warn('XML parse error:', parseError.textContent);
            return {
                pages: [{
                    pageNumber: 'error',
                    startTokenIndex: 0,
                    tokens: [{ type: 'text', displayText: xml }],
                    wordCount: 0
                }],
                totalTokens: 1,
                totalWords: 0
            };
        }

        const allTokens: TextToken[] = [];
        const root = doc.documentElement;

        // Find the body content - try multiple approaches for namespaced XML
        let body: Element | null = null;

        // Try querySelector first
        body = root.querySelector('body');

        // If not found, try getElementsByTagName (works better with namespaces)
        if (!body) {
            const bodies = root.getElementsByTagName('body');
            if (bodies.length > 0) {
                body = bodies[0];
            }
        }

        // Fall back to text element
        if (!body) {
            body = root.querySelector('text') || root.getElementsByTagName('text')[0];
        }

        // Final fallback to root
        const target = body || root;

        // Use a counter object to track word index and yield progress
        const wordCounter = { index: 0 };
        const yieldCounter = { count: 0 };
        const YIELD_EVERY = 500; // Yield every 500 tokens for smooth animation

        await extractTokensAsync(
            target,
            allTokens,
            placeholderGlyphs,
            wordCounter,
            yieldCounter,
            YIELD_EVERY,
        );

        // If no tokens extracted, show a message
        if (allTokens.length === 0) {
            return {
                pages: [{
                    pageNumber: 'empty',
                    startTokenIndex: 0,
                    tokens: [{ type: 'text', displayText: '(No content in body)' }],
                    wordCount: 0
                }],
                totalTokens: 1,
                totalWords: 0
            };
        }

        // Group tokens by page - split on pagebreak tokens
        const pages: Page[] = [];
        let currentPage: Page = {
            pageNumber: 'intro',
            startTokenIndex: 0,
            tokens: [],
            wordCount: 0
        };

        let absoluteTokenIndex = 0;
        for (const token of allTokens) {
            if (token.type === 'pagebreak') {
                // If current page has content, save it
                if (currentPage.tokens.length > 0) {
                    pages.push(currentPage);
                }
                // Start a new page
                currentPage = {
                    pageNumber: token.pageNumber || `page-${pages.length + 1}`,
                    startTokenIndex: absoluteTokenIndex,
                    tokens: [token], // Include the pagebreak token at start of its page
                    wordCount: 0
                };
            } else {
                currentPage.tokens.push(token);
                if (token.type === 'word') {
                    currentPage.wordCount++;
                }
            }
            absoluteTokenIndex++;
        }

        // Don't forget the last page
        if (currentPage.tokens.length > 0) {
            pages.push(currentPage);
        }

        // Calculate totals
        const totalWords = pages.reduce((sum, p) => sum + p.wordCount, 0);

        return {
            pages,
            totalTokens: allTokens.length,
            totalWords
        };
    } catch (e) {
        console.error('Parse error:', e);
        return {
            pages: [{
                pageNumber: 'error',
                startTokenIndex: 0,
                tokens: [{ type: 'text', displayText: `Parse error: ${e}` }],
                wordCount: 0
            }],
            totalTokens: 1,
            totalWords: 0
        };
    }
}

const placeholderPattern = /__ENTITY_\d+__/g;

// Resolve entity placeholders to actual glyphs
function resolveEntitiesToGlyphs(text: string, placeholderGlyphs: Map<string, string>): string {
    if (!text.includes('__ENTITY_')) {
        return text;
    }
    return text.replace(placeholderPattern, (placeholder) => {
        return placeholderGlyphs.get(placeholder) ?? placeholder;
    });
}

function normalizeWhitespace(text: string): string {
    return text.replace(/\s+/g, ' ').trim();
}

function collectWordParts(
    element: Element,
    placeholderGlyphs: Map<string, string>
): Array<{ text: string; lineNumber?: string }> {
    const parts: Array<{ text: string; lineNumber?: string }> = [];
    let buffer = '';

    const flush = (lineNumber?: string) => {
        const normalized = normalizeWhitespace(buffer);
        if (normalized) {
            parts.push({
                text: resolveEntitiesToGlyphs(normalized, placeholderGlyphs),
                lineNumber,
            });
        } else if (lineNumber) {
            parts.push({ text: '', lineNumber });
        }
        buffer = '';
    };

    const walk = (node: Node) => {
        for (const child of node.childNodes) {
            if (child.nodeType === Node.TEXT_NODE) {
                buffer += child.textContent ?? '';
                continue;
            }

            if (child.nodeType !== Node.ELEMENT_NODE) {
                continue;
            }

            const el = child as Element;
            if (el.localName === 'lb') {
                flush(el.getAttribute('n') ?? undefined);
                continue;
            }

            walk(el);
        }
    };

    walk(element);
    flush();

    return parts;
}

// Async version of extractTokens that yields periodically
async function extractTokensAsync(
    node: Node,
    result: TextToken[],
    placeholderGlyphs: Map<string, string>,
    wordCounter: { index: number },
    yieldCounter: { count: number },
    yieldEvery: number
): Promise<void> {
    for (const child of node.childNodes) {
        // Yield to main thread periodically
        yieldCounter.count++;
        if (yieldCounter.count % yieldEvery === 0) {
            await yieldToMain();
        }

        if (child.nodeType === Node.TEXT_NODE) {
            const text = child.textContent || '';
            if (text.trim()) {
                // Collapse source formatting (newlines, indentation) to single
                // spaces, keeping a space at either end as a word separator
                result.push({
                    type: 'text',
                    displayText: resolveEntitiesToGlyphs(text.replace(/\s+/g, ' '), placeholderGlyphs),
                });
            }
        } else if (child.nodeType === Node.ELEMENT_NODE) {
            const el = child as Element;
            const tagName = el.localName;

            switch (tagName) {
                case 'w': {
                    const facsEl = el.querySelector('me\\:facs, facs');
                    const diplEl = el.querySelector('me\\:dipl, dipl');
                    const wordIndex = wordCounter.index;
                    wordCounter.index++;

                    let parts = facsEl ? collectWordParts(facsEl, placeholderGlyphs) : [];
                    if (!parts.some(part => part.text || part.lineNumber)) {
                        parts = diplEl ? collectWordParts(diplEl, placeholderGlyphs) : [];
                    }
                    if (!parts.some(part => part.text || part.lineNumber)) {
                        parts = collectWordParts(el, placeholderGlyphs);
                    }

                    const diplomatic = normalizeWhitespace(
                        resolveEntitiesToGlyphs(
                            diplEl?.textContent || facsEl?.textContent || el.textContent || '',
                            placeholderGlyphs,
                        ),
                    );
                    const lastTextIndex = parts.reduce(
                        (last, part, index) => (part.text ? index : last),
                        -1,
                    );

                    if (lastTextIndex >= 0) {
                        parts.forEach((part, index) => {
                            if (part.text) {
                                result.push({
                                    type: 'word',
                                    displayText: part.text,
                                    diplomatic,
                                    wordIndex,
                                });
                            }
                            if (part.lineNumber) {
                                result.push({
                                    type: 'linebreak',
                                    displayText: '',
                                    lineNumber: part.lineNumber,
                                });
                            }
                            if (part.text && index === lastTextIndex) {
                                result.push({ type: 'space', displayText: ' ' });
                            }
                        });
                    } else {
                        parts.forEach((part) => {
                            if (part.lineNumber) {
                                result.push({
                                    type: 'linebreak',
                                    displayText: '',
                                    lineNumber: part.lineNumber,
                                });
                            }
                        });
                    }
                    break;
                }
                case 'pc': {
                    const facsEl = el.querySelector('me\\:facs, facs');
                    const facsRaw = facsEl?.textContent || el.textContent || '';
                    const displayText = resolveEntitiesToGlyphs(facsRaw, placeholderGlyphs).trim();
                    if (displayText) {
                        result.push({ type: 'punctuation', displayText });
                        result.push({ type: 'space', displayText: ' ' });
                    }
                    break;
                }
                case 'lb': {
                    const n = el.getAttribute('n');
                    result.push({ type: 'linebreak', displayText: '', lineNumber: n || undefined });
                    break;
                }
                case 'pb': {
                    const n = el.getAttribute('n');
                    result.push({ type: 'pagebreak', displayText: '', pageNumber: n || undefined });
                    break;
                }
                case 'gap': {
                    const quantity = el.getAttribute('quantity') || '?';
                    result.push({ type: 'text', displayText: `[…${quantity}]` });
                    break;
                }
                // Editorial wrappers: when they contain whole words (as MENOTA
                // files do), descend so the words render once, normally, and stay
                // clickable; otherwise show their text with the editorial marks.
                case 'supplied':
                    await extractWrapper(el, '⟨', '⟩', result, placeholderGlyphs, wordCounter, yieldCounter, yieldEvery);
                    break;
                case 'unclear':
                    await extractWrapper(el, '', '?', result, placeholderGlyphs, wordCounter, yieldCounter, yieldEvery);
                    break;
                case 'del':
                    await extractWrapper(el, '⟦', '⟧', result, placeholderGlyphs, wordCounter, yieldCounter, yieldEvery);
                    break;
                case 'add':
                    await extractWrapper(el, '\\', '/', result, placeholderGlyphs, wordCounter, yieldCounter, yieldEvery);
                    break;
                case 'choice': {
                    // Word-level alternatives (sic/corr, orig/reg): show the first
                    // reading, as the XSLT preview does. Words in the other readings
                    // still take their index, so numbering follows document order.
                    if (el.querySelector('w') !== null) {
                        const [shown, ...hidden] = Array.from(el.children);
                        if (shown) {
                            await extractTokensAsync(shown, result, placeholderGlyphs, wordCounter, yieldCounter, yieldEvery);
                        }
                        for (const branch of hidden) {
                            wordCounter.index += branch.getElementsByTagName('w').length + (branch.localName === 'w' ? 1 : 0);
                        }
                        break;
                    }
                    const abbr = el.querySelector('abbr')?.textContent || '';
                    const text = resolveEntitiesToGlyphs(abbr, placeholderGlyphs);
                    if (text) {
                        result.push({ type: 'text', displayText: text });
                        result.push({ type: 'space', displayText: ' ' });
                    }
                    break;
                }
                case 'head': {
                    await extractTokensAsync(
                        el,
                        result,
                        placeholderGlyphs,
                        wordCounter,
                        yieldCounter,
                        yieldEvery,
                    );
                    result.push({ type: 'linebreak', displayText: '' });
                    break;
                }
                default:
                    // Recurse into other elements
                    await extractTokensAsync(el, result, placeholderGlyphs, wordCounter, yieldCounter, yieldEvery);
            }
        }
    }
}

async function extractWrapper(
    el: Element,
    open: string,
    close: string,
    result: TextToken[],
    placeholderGlyphs: Map<string, string>,
    wordCounter: { index: number },
    yieldCounter: { count: number },
    yieldEvery: number,
): Promise<void> {
    if (el.querySelector('w') !== null) {
        if (open) result.push({ type: 'text', displayText: open });
        await extractTokensAsync(el, result, placeholderGlyphs, wordCounter, yieldCounter, yieldEvery);
        if (close) result.push({ type: 'text', displayText: close });
        return;
    }
    const text = normalizeWhitespace(resolveEntitiesToGlyphs(el.textContent || '', placeholderGlyphs));
    if (text) {
        result.push({ type: 'text', displayText: `${open}${text}${close}` });
    }
}
