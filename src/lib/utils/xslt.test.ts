import { describe, expect, it } from "vitest";
import { escapeHtml, sanitizeXsltHtml, transformResultToHtml } from "$lib/utils/xslt";

describe("sanitizeXsltHtml", () => {
    it("removes executable content", () => {
        const dirty =
            '<span onclick="alert(1)">a</span><img src="x" onerror="alert(2)">' +
            '<a href="javascript:alert(3)">l</a><script>alert(4)</script>' +
            '<iframe src="about:blank"></iframe><form action="https://evil"><input></form>' +
            "<svg><script>alert(5)</script></svg>";
        const clean = sanitizeXsltHtml(dirty);
        expect(clean).not.toMatch(/onclick|onerror|javascript:|<script|<iframe|<form/i);
    });

    it("keeps styles, classes and data attributes", () => {
        const clean = sanitizeXsltHtml(
            '<style>.w{color:red}</style><span class="w" data-word-index="3">orð</span>',
        );
        expect(clean).toContain("<style>.w{color:red}</style>");
        expect(clean).toContain('class="w"');
        expect(clean).toContain('data-word-index="3"');
    });
});

describe("transformResultToHtml", () => {
    it("keeps head styles of a full html result", () => {
        const doc = new DOMParser().parseFromString(
            "<html><head><style>p{}</style><title>t</title></head><body><p>x</p></body></html>",
            "text/html",
        );
        expect(transformResultToHtml(doc)).toBe("<style>p{}</style><p>x</p>");
    });
});

describe("escapeHtml", () => {
    it("escapes markup characters", () => {
        expect(escapeHtml('<a href="x">&</a>')).toBe("&lt;a href=&quot;x&quot;&gt;&amp;&lt;/a&gt;");
    });
});
