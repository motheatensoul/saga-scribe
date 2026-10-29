/**
 * Pagination for PDF export.
 *
 * Vivliostyle, a CSS Paged Media typesetter, lays the export document out into
 * pages: it honours `@page` size and margins, page-margin boxes (page numbers),
 * widows/orphans and break rules. The result is a static document of finished,
 * page-sized boxes, which any platform's print engine prints as-is.
 *
 * Layout runs in an off-screen iframe in the (visible) main window: hidden
 * windows get their timers throttled, which slows Vivliostyle's time-sliced
 * layout down by orders of magnitude. The iframe keeps the app's own styles
 * away from the pages, and Vivliostyle yields between slices, so the app
 * stays responsive meanwhile.
 */

import { CoreViewer } from "@vivliostyle/core";

/** Paginates a standalone HTML document; resolves to a printable static HTML document. */
export async function paginateForPrint(html: string, signal?: AbortSignal): Promise<string> {
    signal?.throwIfAborted();
    const exportDoc = new DOMParser().parseFromString(html, "text/html");

    const iframe = document.createElement("iframe");
    iframe.setAttribute("aria-hidden", "true");
    iframe.tabIndex = -1;
    // Off-screen but not hidden, so it is laid out normally
    iframe.style.cssText =
        "position: fixed; left: -20000px; top: 0; width: 1200px; height: 1200px; border: 0;";
    document.body.appendChild(iframe);

    try {
        const frameWindow = iframe.contentWindow;
        const frameDoc = iframe.contentDocument;
        if (!frameWindow || !frameDoc) {
            throw new Error("could not create the layout frame");
        }
        frameDoc.body.style.margin = "0";
        const viewport = frameDoc.createElement("div");
        viewport.style.cssText = "width: 100vw; height: 100vh;";
        frameDoc.body.appendChild(viewport);

        await new Promise<void>((resolve, reject) => {
            const onAbort = () => reject(signal?.reason);
            signal?.addEventListener("abort", onAbort, { once: true });

            const viewer = new CoreViewer(
                { viewportElement: viewport, window: frameWindow, debug: false },
                { renderAllPages: true, autoResize: false, pageViewMode: "singlePage" as never },
            );
            viewer.addListener("readystatechange", () => {
                if (viewer.readyState === "complete") {
                    signal?.removeEventListener("abort", onAbort);
                    resolve();
                }
            });
            viewer.addListener("error", (payload) => {
                // Mostly unsupported CSS, which is skipped; a fatal error carries an Error
                const error = payload.content?.error;
                if (error) {
                    signal?.removeEventListener("abort", onAbort);
                    reject(error);
                } else {
                    console.warn("Vivliostyle:", ...(payload.content?.messages ?? []));
                }
            });
            // The document is passed in directly; the app's URL only serves as
            // its base, so `/fonts/...` resolve
            viewer.loadDocument({ url: location.href }, { documentObject: exportDoc });
        });

        // Print engines that ignore @page margins (WebKitGTK) would spill a box
        // that is a fraction of a pixel taller than the sheet onto a blank
        // page. This also replaces Vivliostyle's own clamp to the window
        // height, which is 0 px in the hidden print window.
        for (const page of viewport.querySelectorAll<HTMLElement>("[data-vivliostyle-page-container]")) {
            page.style.maxHeight = `calc(${page.style.height} - 2px)`;
            page.style.overflow = "hidden";
        }

        // Every page is styled inline; the <style> elements hold Vivliostyle's
        // print rules, the sheet size and @font-face rules (absolute URLs)
        const styles = Array.from(frameDoc.querySelectorAll("style"))
            .map((style) => style.outerHTML)
            .join("\n");
        const title = exportDoc.title.replace(/[<&]/g, "");
        return `<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<title>${title}</title>
${styles}
<style>
    html, body { margin: 0; padding: 0; background: #fff; }
    /* Print every page, not just the viewer's scroll area */
    [data-vivliostyle-viewer-viewport] {
        width: auto !important;
        height: auto !important;
        overflow: visible !important;
    }
    /* Vivliostyle sizes these to the window, which is 0 px high in the hidden
       print window; every page has its exact size inline already */
    @media print {
        [data-vivliostyle-outer-zoom-box],
        [data-vivliostyle-spread-container] {
            width: auto !important;
            height: auto !important;
        }
    }
</style>
</head>
<body>${viewport.outerHTML}</body>
</html>`;
    } finally {
        iframe.remove();
    }
}
