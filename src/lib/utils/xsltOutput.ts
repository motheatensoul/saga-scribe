/**
 * Safe handling of HTML produced by XSLT transforms.
 *
 * Both the XML (imported files) and the stylesheet (user-added XSLT) can come
 * from third parties, and the result is rendered inside the app, where scripts
 * would have access to the Tauri IPC bridge. Everything an XSLT produces goes
 * through here before it is rendered, exported or printed.
 */

import DOMPurify from "dompurify";

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
