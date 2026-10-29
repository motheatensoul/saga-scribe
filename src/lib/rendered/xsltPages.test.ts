import { describe, expect, it } from "vitest";
import { isPageLocal, splitPages } from "$lib/rendered/xsltPages";

const TEI = (body: string) =>
    `<?xml version="1.0"?><TEI xmlns="http://www.tei-c.org/ns/1.0"><teiHeader/><text><body>${body}</body></text></TEI>`;

function parse(xml: string): Document {
    const doc = new DOMParser().parseFromString(xml, "application/xml");
    expect(doc.querySelector("parsererror")).toBeNull();
    return doc;
}

describe("isPageLocal", () => {
    it("recognises the declaration", () => {
        expect(isPageLocal(`<xsl:stylesheet><?saga-scribe render="page"?></xsl:stylesheet>`)).toBe(true);
        expect(isPageLocal(`<xsl:stylesheet><xsl:template match="/"/></xsl:stylesheet>`)).toBe(false);
    });
});

describe("splitPages", () => {
    it("cuts at page breaks into well-formed documents, reopening open elements", () => {
        const source = splitPages(
            TEI(`<div n="1"><p>intro <lb/>a<pb n="1r"/><lb/>b</p><p>c</p><pb n="1v"/><p>d</p></div>`),
        )!;
        expect(source.pieces).toHaveLength(3);
        const docs = source.pieces.map(parse);

        // Page 1r continues the paragraph and div opened on the intro page
        const p = docs[1].getElementsByTagName("p")[0];
        expect(p.getAttribute("saga-continued")).toBe("true");
        expect(docs[1].getElementsByTagName("div")[0].getAttribute("saga-continued")).toBe("true");
        expect(docs[1].getElementsByTagName("div")[0].getAttribute("n")).toBe("1");
        // A paragraph starting on the page is not continued
        expect(docs[2].getElementsByTagName("p")[0].hasAttribute("saga-continued")).toBe(false);

        // Every text and break is on exactly one page
        expect(docs.map((d) => d.documentElement.textContent).join("|")).toBe("intro a|bc|d");
        expect(docs.map((d) => d.getElementsByTagName("pb").length)).toEqual([0, 1, 1]);
        expect(source.lineBased).toBe(true);
    });

    it("has no page for an empty start, and spots prose without line breaks", () => {
        const source = splitPages(TEI(`\n  <pb n="1"/><p>a</p><pb n="2"/><p>b</p>`))!;
        expect(source.pieces).toHaveLength(2);
        expect(source.lineBased).toBe(false);
    });

    it("doesn't cut inside a word broken across pages", () => {
        const source = splitPages(TEI(`<p><w>a<pb n="2"/>b</w><pb n="3"/>c</p>`))!;
        expect(source.pieces.map((xml) => parse(xml).documentElement.textContent)).toEqual(["ab", "c"]);
    });

    it("gives up on markup it can't scan", () => {
        expect(splitPages("<TEI><text>no body</text></TEI>")).toBeNull();
        expect(splitPages(TEI(`<p>unclosed <hi>tag</p>`))).toBeNull();
    });
});
