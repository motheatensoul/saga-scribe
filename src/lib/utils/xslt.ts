/**
 * XSLT rendering pipeline shared by the live preview and HTML/PDF export.
 *
 * Both the XML (imported files) and the stylesheet (user-added XSLT) can come
 * from third parties, and the result is rendered inside the app, where scripts
 * would have access to the Tauri IPC bridge. Everything an XSLT produces is
 * sanitized here before it is rendered, exported or printed.
 *
 * The browser's XSLTProcessor and DOMParser are main-thread only, so the
 * pipeline yields between stages: a spinner can animate, progress can be shown,
 * and the run can be cancelled via an AbortSignal.
 */

import DOMPurify from "dompurify";
import { readTextFile } from "@tauri-apps/plugin-fs";
import type { EntityMap } from "$lib/stores/entities";

export type XsltStage = "preparing" | "parsing" | "transforming" | "sanitizing";

export interface XsltRunOptions {
    /** Render only `<body>` (or `<text>`), as the live preview does. */
    bodyOnly?: boolean;
    /** Aborting stops the run at the next stage boundary with an AbortError. */
    signal?: AbortSignal;
    /** Called as each stage starts, e.g. to show progress. */
    onStage?: (stage: XsltStage) => void;
}

const STANDARD_ENTITIES = new Set(["lt", "gt", "amp", "quot", "apos"]);

const yieldToMain = () => new Promise((resolve) => setTimeout(resolve, 0));

/** Loads a bundled (`/xsl/...`) or user-provided stylesheet. */
export async function loadStylesheetText(path: string): Promise<string> {
    if (path.startsWith("/xsl/")) {
        const response = await fetch(path);
        if (!response.ok) {
            throw new Error(`Failed to load stylesheet: ${response.statusText}`);
        }
        return response.text();
    }
    return readTextFile(path);
}

export function createXsltProcessor(xslText: string): XSLTProcessor {
    const xslDoc = new DOMParser().parseFromString(xslText, "application/xml");
    const parseError = xslDoc.querySelector("parsererror");
    if (parseError) {
        throw new Error(`XSL parse error: ${parseError.textContent}`);
    }
    const processor = new XSLTProcessor();
    processor.importStylesheet(xslDoc);
    return processor;
}

/**
 * Transforms TEI-XML to sanitized HTML.
 *
 * Named entities are swapped for placeholders before parsing (DOMParser has no
 * DTD) and resolved to glyphs afterwards in a single pass.
 */
export async function runXsltTransform(
    processor: XSLTProcessor,
    xml: string,
    entities: EntityMap,
    options: XsltRunOptions = {},
): Promise<string> {
    const { bodyOnly = false, signal, onStage } = options;
    const stage = async (name: XsltStage) => {
        await yieldToMain();
        signal?.throwIfAborted();
        onStage?.(name);
    };

    await stage("preparing");
    // Strip DOCTYPE/entity declarations (not supported by DOMParser)
    let prepared = xml
        .replace(/<!DOCTYPE[\s\S]*?\]>\s*/gi, "")
        .replace(/<!DOCTYPE[^>]*>\s*/gi, "");

    if (bodyOnly) {
        const fragment = sliceTag(prepared, "body") ?? sliceTag(prepared, "text");
        if (fragment) {
            prepared = `<TEI xmlns="${TEI_NS}" xmlns:me="${MENOTA_NS}">${fragment}</TEI>`;
        }
    }
    const placeheld = withEntityPlaceholders(prepared, entities);

    await stage("parsing");
    const xmlDoc = parseXml(placeheld.xml);

    await stage("transforming");
    const html = transformParsed(processor, xmlDoc, placeheld.glyphs);

    await stage("sanitizing");
    const clean = sanitizeXsltHtml(html);
    signal?.throwIfAborted();
    return clean;
}

/** Namespaces of TEI and MENOTA elements. */
export const TEI_NS = "http://www.tei-c.org/ns/1.0";
export const MENOTA_NS = "http://www.menota.org/ns/1.0";

/**
 * Transforms a small TEI document (e.g. one page) to sanitized HTML in one
 * go, for callers that do their own yielding between pieces.
 */
export function transformToSafeHtml(processor: XSLTProcessor, xml: string, entities: EntityMap): string {
    const placeheld = withEntityPlaceholders(xml, entities);
    const html = transformParsed(processor, parseXml(placeheld.xml), placeheld.glyphs);
    return sanitizeXsltHtml(html);
}

/**
 * Swaps named entities for placeholders of ASCII letters and digits, which
 * XML parsing and XSLT leave intact; `glyphs[n]` is what `XENTnX` stands for.
 */
function withEntityPlaceholders(xml: string, entities: EntityMap): { xml: string; glyphs: string[] } {
    const glyphs: string[] = [];
    const replaced = xml.replace(/&([a-zA-Z][a-zA-Z0-9]*);/g, (match, name: string) => {
        if (STANDARD_ENTITIES.has(name)) return match;
        glyphs.push(entities[name]?.char || `[${name}]`);
        return `XENT${glyphs.length - 1}X`;
    });
    return { xml: replaced, glyphs };
}

function parseXml(xml: string): Document {
    const xmlDoc = new DOMParser().parseFromString(xml, "application/xml");
    const parseError = xmlDoc.querySelector("parsererror");
    if (parseError) {
        throw new Error(`XML parse error: ${parseError.textContent}`);
    }
    return xmlDoc;
}

/** Runs the stylesheet and serializes its result, with entity glyphs restored. */
function transformParsed(processor: XSLTProcessor, xmlDoc: Document, glyphs: string[]): string {
    const resultDoc = processor.transformToDocument(xmlDoc);
    if (!resultDoc || !resultDoc.documentElement) {
        throw new Error("XSLT transformation produced no output");
    }
    return transformResultToHtml(resultDoc).replace(/XENT(\d+)X/g, (match, index: string) => {
        const glyph = glyphs[Number(index)];
        return glyph !== undefined ? escapeHtml(glyph) : match;
    });
}

function sliceTag(xml: string, tagName: string): string | null {
    const lower = xml.toLowerCase();
    const start = lower.indexOf(`<${tagName}`);
    if (start === -1) return null;
    const end = lower.indexOf(`</${tagName}>`, start);
    if (end === -1) return null;
    return xml.slice(start, end + tagName.length + 3);
}

/** Escapes text for insertion into HTML markup. */
export function escapeHtml(text: string): string {
    return text
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;");
}

/**
 * Serializes an XSLT result document to an HTML fragment.
 *
 * When the stylesheet emits a full `<html>` document, the `<style>` blocks from
 * its head are kept ahead of the body content so they still apply.
 */
export function transformResultToHtml(resultDoc: Document): string {
    const root = resultDoc.documentElement;
    if (root.localName.toLowerCase() !== "html") {
        return root.outerHTML;
    }
    const styles = Array.from(root.querySelectorAll("head style"))
        .map((style) => style.outerHTML)
        .join("");
    const body = root.querySelector("body");
    return styles + (body ? body.innerHTML : "");
}

/**
 * Removes anything executable from XSLT output: scripts, event handlers,
 * `javascript:` URLs, frames, objects, forms and document-level elements.
 * Styling, classes and `data-*` attributes (used for word selection) are kept.
 */
export function sanitizeXsltHtml(html: string): string {
    return DOMPurify.sanitize(html, {
        // Keep leading <style> blocks instead of letting the parser move them out
        FORCE_BODY: true,
        FORBID_TAGS: ["form", "base", "meta", "link"],
    });
}
