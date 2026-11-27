import { afterEach, describe, expect, it, vi } from "vitest";
import { flushSync, mount, tick, unmount } from "svelte";
import type { WordXmlChange } from "$lib/tauri";

const set = vi.fn(async (_word: number, xml: string): Promise<WordXmlChange> => {
    if (!xml.startsWith("<w")) throw "The XML must be a single <w> element";
    return { from: 3, to: 7, dsl: "mannr", lemma: "maðr", msa: "xNC" };
});
vi.mock("$lib/tauri", async (original) => ({
    ...(await original<typeof import("$lib/tauri")>()),
    importedWordXml: async () => '<w lemma="maðr"><me:dipl>ma<ex>n</ex>n</me:dipl></w>',
    setImportedWordXml: (word: number, xml: string) => set(word, xml),
}));

const { default: WordXmlPanel } = await import("$lib/components/WordXmlPanel.svelte");

describe("WordXmlPanel", () => {
    let component: Record<string, unknown> | null = null;
    afterEach(() => {
        if (component) unmount(component);
        component = null;
        document.body.innerHTML = "";
    });

    it("shows the word's XML and what it is made of, and applies an edit", async () => {
        const target = document.createElement("div");
        document.body.appendChild(target);
        const applied: WordXmlChange[] = [];
        component = mount(WordXmlPanel, { target, props: { wordIndex: 1, onapplied: (c: WordXmlChange) => applied.push(c) } });
        await tick();
        await new Promise((r) => setTimeout(r, 0));
        flushSync();
        const textarea = target.querySelector("textarea")!;
        expect(textarea.value).toContain("<ex>n</ex>");
        expect(target.textContent).toContain("diplomatic");
        expect(target.textContent).toContain("<ex>");

        // The attribute and level controls write into the XML
        const lemma = target.querySelector("#word-attribute-value-0") as HTMLInputElement;
        lemma.value = "maður";
        lemma.dispatchEvent(new Event("change", { bubbles: true }));
        flushSync();
        expect(textarea.value).toContain('<w lemma="maður">');
        (([...target.querySelectorAll("button")].find((b) => b.textContent?.trim() === "Add an attribute")) as HTMLButtonElement).click();
        flushSync();
        expect(textarea.value).toContain('<w lemma="maður" name="">');
        const dipl = [...target.querySelectorAll("label")].find((l) => l.textContent?.includes("diplomatic"))!.querySelector("input")!;
        expect(dipl.value).toBe("ma<ex>n</ex>n");
        dipl.value = "ma<ex>n</ex>nr";
        dipl.dispatchEvent(new Event("change", { bubbles: true }));
        flushSync();
        expect(textarea.value).toContain("<me:dipl>ma<ex>n</ex>nr</me:dipl>");

        textarea.value = "oops";
        textarea.dispatchEvent(new Event("input"));
        flushSync();
        const apply = [...target.querySelectorAll("button")].find((b) => b.textContent?.trim() === "Apply")!;
        apply.click();
        await new Promise((r) => setTimeout(r, 0));
        flushSync();
        expect(target.querySelector('[role="alert"]')?.textContent).toContain("single <w>");
        expect(applied).toEqual([]);

        textarea.value = '<w lemma="maðr" me:msa="xNC"><me:dipl>ma<ex>n</ex>nr</me:dipl></w>';
        textarea.dispatchEvent(new Event("input"));
        flushSync();
        apply.click();
        await new Promise((r) => setTimeout(r, 0));
        flushSync();
        expect(applied).toEqual([{ from: 3, to: 7, dsl: "mannr", lemma: "maðr", msa: "xNC" }]);
        expect(set).toHaveBeenLastCalledWith(1, textarea.value);
    });
});
