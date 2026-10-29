import { afterEach, describe, expect, it, vi } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import XsltRenderer from "$lib/components/XsltRenderer.svelte";
import { entityStore } from "$lib/stores/entities";

describe("XsltRenderer", () => {
    let component: Record<string, unknown> | null = null;
    afterEach(() => {
        if (component) unmount(component);
        component = null;
        vi.unstubAllGlobals();
    });

    it("mounts and updates without its effects re-triggering themselves", async () => {
        // jsdom has no XSLTProcessor: loading the stylesheet fails, which the
        // component shows as an error; mounting must not throw
        vi.stubGlobal("fetch", async () => new Response("<xsl:stylesheet/>"));
        const target = document.createElement("div");
        document.body.appendChild(target);

        const props = $state({ content: "", active: true });
        component = mount(XsltRenderer, { target, props });
        flushSync();
        await new Promise((resolve) => setTimeout(resolve, 0));
        flushSync();

        props.content = "<TEI><text><body><p>a</p></body></text></TEI>";
        flushSync();
        expect(target.querySelector(".xslt-rendered")).not.toBeNull();
    });
});

/** Stands in for the browser's XSLTProcessor (jsdom has none): renders each
 *  page document as a div holding its text. */
class FakeXsltProcessor {
    importStylesheet() {}
    setParameter() {}
    removeParameter() {}
    transformToDocument(doc: Document): Document {
        const result = document.implementation.createHTMLDocument("");
        const div = result.createElement("div");
        div.className = "page-text";
        div.textContent = doc.documentElement.textContent ?? "";
        result.replaceChild(div, result.documentElement);
        return result;
    }
}

const PAGE_LOCAL_XSL = `<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform"><?saga-scribe render="page"?></xsl:stylesheet>`;
const tei = (body: string) => `<TEI xmlns="http://www.tei-c.org/ns/1.0"><text><body>${body}</body></text></TEI>`;

describe("XsltRenderer page by page", () => {
    afterEach(() => vi.unstubAllGlobals());

    async function settle() {
        for (let i = 0; i < 5; i++) {
            flushSync();
            await new Promise((resolve) => setTimeout(resolve, 0));
        }
    }

    it("renders pages separately and replaces only changed ones", async () => {
        vi.stubGlobal("fetch", async () => new Response(PAGE_LOCAL_XSL));
        vi.stubGlobal("XSLTProcessor", FakeXsltProcessor);
        // The renderer waits for the entity list (as loaded at app start)
        entityStore.setBuiltinEntities({});
        const target = document.createElement("div");
        document.body.appendChild(target);

        const props = $state({ content: tei(`<pb n="1"/><p>one</p><pb n="2"/><p>two</p>`), active: true });
        const component = mount(XsltRenderer, { target, props });
        await settle();

        const pages = () => [...target.querySelectorAll<HTMLElement>(".xslt-page")];
        expect(pages().map((p) => p.textContent)).toEqual(["one", "two"]);
        const [first, second] = pages();

        // An edit on page 2 leaves page 1's element alone
        props.content = tei(`<pb n="1"/><p>one</p><pb n="2"/><p>two!</p>`);
        await settle();
        expect(pages().map((p) => p.textContent)).toEqual(["one", "two!"]);
        expect(pages()[0]).toBe(first);
        expect(pages()[1]).toBe(second);

        // A new page: existing pages keep their elements
        props.content = tei(`<pb n="1"/><p>one</p><pb n="2"/><p>two!</p><pb n="3"/><p>three</p>`);
        await settle();
        expect(pages().map((p) => p.textContent)).toEqual(["one", "two!", "three"]);
        expect(pages()[0]).toBe(first);

        unmount(component);
    });
});
