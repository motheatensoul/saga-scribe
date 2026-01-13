/**
 * HTML Export Utility
 * Generates standalone HTML documents from XSLT-rendered content
 */

export interface HtmlExportOptions {
    title?: string;
    includeLineNumbers?: boolean;
    pageBreakStyle?: "visible" | "print-break" | "none";
}

function escapeHtml(str: string): string {
    return str
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;");
}

/**
 * Generates a standalone HTML document with embedded CSS
 * suitable for viewing or printing without the application
 */
export function generateStandaloneHtml(
    bodyHtml: string,
    options: HtmlExportOptions = {},
): string {
    const title = options.title || "Saga Scribe Export";
    const pageBreakStyle = options.pageBreakStyle ?? "visible";

    // CSS that works standalone (no DaisyUI variables)
    const css = `
        * {
            box-sizing: border-box;
        }

        body {
            font-family: 'Junicode', Georgia, 'Times New Roman', serif;
            font-size: 16px;
            line-height: 1.8;
            max-width: 800px;
            margin: 2rem auto;
            padding: 0 1rem;
            color: #1a1a1a;
            background: #fff;
        }

        .word {
            display: inline;
        }

        .line-number {
            display: inline-block;
            min-width: 2rem;
            margin-right: 0.5rem;
            font-size: 0.75rem;
            color: #666;
            font-family: monospace;
            text-align: right;
            user-select: none;
        }

        .linebreak {
            display: block;
        }

        .pagebreak {
            display: block;
            margin: 1.5rem 0;
            text-align: center;
            ${pageBreakStyle === "print-break" ? "page-break-before: always;" : ""}
        }

        .page-indicator {
            display: ${pageBreakStyle === "visible" ? "inline-block" : "none"};
            padding: 0.25rem 0.75rem;
            font-size: 0.875rem;
            color: #666;
            font-family: monospace;
            border: 1px dashed #666;
            border-radius: 0.25rem;
        }

        del {
            text-decoration: line-through;
            color: #c53030;
            background: rgba(197, 48, 48, 0.1);
        }

        ins, .add {
            text-decoration: none;
            color: #2f855a;
            background: rgba(47, 133, 90, 0.1);
        }

        .choice {
            border-bottom: 1px dotted #3182ce;
        }

        .supplied {
            color: #666;
            font-style: italic;
        }

        .supplied::before {
            content: '[';
        }

        .supplied::after {
            content: ']';
        }

        .unclear {
            opacity: 0.7;
            text-decoration: underline dotted;
        }

        .gap {
            color: #888;
            font-style: italic;
        }

        .note {
            font-size: 0.875rem;
            color: #666;
            font-style: italic;
        }

        @media print {
            body {
                margin: 0;
                padding: 1cm;
                max-width: none;
            }

            .pagebreak {
                page-break-before: always;
            }

            .page-indicator {
                display: ${pageBreakStyle === "none" ? "none" : "inline-block"};
            }
        }
    `;

    return `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>${escapeHtml(title)}</title>
    <style>${css}</style>
</head>
<body>
${bodyHtml}
</body>
</html>`;
}
