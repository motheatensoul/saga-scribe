/**
 * PDF Export Utility
 *
 * Where supported (Linux, Windows) the backend writes the PDF directly from a
 * hidden window (see src-tauri/src/commands/pdf.rs). Elsewhere this falls back
 * to the system print dialog via a hidden iframe.
 */

export interface PdfExportOptions {
    title?: string;
    pageSize?: "A4" | "Letter";
}

/**
 * The app's bundled Junicode, so MUFI characters print even where the font
 * isn't installed. Only resolves in documents served from the app's origin
 * (the print window, or an iframe in the app).
 */
const APP_FONT_FACES = `
    <style>
        @font-face {
            font-family: 'Junicode';
            src: url('/fonts/Junicode.woff2') format('woff2');
            font-weight: 300 700;
            font-style: normal;
        }
        @font-face {
            font-family: 'Junicode';
            src: url('/fonts/JunicodeVF-Italic.woff2') format('woff2');
            font-weight: 300 700;
            font-style: italic;
        }
    </style>
`;

/** Adds page size, margins, print-only rules and the app fonts to a standalone HTML document. */
export function withPrintStyles(htmlContent: string, pageSize: "A4" | "Letter" = "A4"): string {
    const printStyles = `
        <style>
            @page {
                size: ${pageSize};
                margin: 2cm;
            }
            @media print {
                body {
                    margin: 0;
                    padding: 0;
                    max-width: none;
                }
                .pagebreak {
                    page-break-before: always;
                }
            }
        </style>
    `;
    return htmlContent.replace("</head>", `${APP_FONT_FACES}${printStyles}</head>`);
}

/**
 * Opens the browser's print dialog with the given HTML content.
 * User can select "Save as PDF" to export.
 */
export async function printToPdf(
    htmlContent: string,
    options: PdfExportOptions = {},
): Promise<void> {
    return new Promise((resolve, reject) => {
        // Create hidden iframe
        const iframe = document.createElement("iframe");
        iframe.style.position = "absolute";
        iframe.style.left = "-9999px";
        iframe.style.width = "210mm"; // A4 width
        iframe.style.height = "297mm"; // A4 height
        document.body.appendChild(iframe);

        const iframeDoc =
            iframe.contentDocument || iframe.contentWindow?.document;
        if (!iframeDoc) {
            document.body.removeChild(iframe);
            reject(new Error("Could not access iframe document"));
            return;
        }

        const printHtml = withPrintStyles(htmlContent, options.pageSize || "A4");

        // Write content to iframe
        iframeDoc.open();
        iframeDoc.write(printHtml);
        iframeDoc.close();

        // Wait for content to load, then print
        const handleLoad = () => {
            try {
                iframe.contentWindow?.focus();
                iframe.contentWindow?.print();

                // Clean up after a delay (print dialog may still be open)
                setTimeout(() => {
                    if (iframe.parentNode) {
                        document.body.removeChild(iframe);
                    }
                    resolve();
                }, 1000);
            } catch (e) {
                if (iframe.parentNode) {
                    document.body.removeChild(iframe);
                }
                reject(e);
            }
        };

        // Handle both load event and direct execution for already-loaded content
        if (iframeDoc.readyState === "complete") {
            handleLoad();
        } else {
            iframe.onload = handleLoad;
        }
    });
}
