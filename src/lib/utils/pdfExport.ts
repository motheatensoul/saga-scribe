/**
 * PDF Export Utility
 * Uses browser's print-to-PDF functionality via a hidden iframe
 */

export interface PdfExportOptions {
    title?: string;
    pageSize?: "A4" | "Letter";
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

        // Add print-specific styles to the HTML
        const pageSize = options.pageSize || "A4";
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

        const printHtml = htmlContent.replace("</head>", `${printStyles}</head>`);

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
