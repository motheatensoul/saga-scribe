import { afterEach, describe, expect, it, vi } from "vitest";
import { flushSync, mount, tick, unmount } from "svelte";
import type { ElementInfo } from "$lib/tauri";

const elements: ElementInfo[] = [
    { name: "s", ordinal: 0, place: "around", attributes: [], fixed: [] },
    { name: "lb", ordinal: 2, place: "before", attributes: [["n", "3"]], fixed: ["n"] },
];
const set = vi.fn(async (_name: string, _ordinal: number, _attributes: [string, string][]) => {});
vi.mock("$lib/tauri", async (original) => ({
    ...(await original<typeof import("$lib/tauri")>()),
    importedWordElements: async () => elements,
    setImportedElementAttributes: (name: string, ordinal: number, attributes: [string, string][]) =>
        set(name, ordinal, attributes),
}));

const { default: WordMarkupPanel } = await import("$lib/components/WordMarkupPanel.svelte");

const settle = async () => {
    await tick();
    await new Promise((r) => setTimeout(r, 0));
    flushSync();
};

describe("WordMarkupPanel", () => {
    let component: Record<string, unknown> | null = null;
    afterEach(() => {
        if (component) unmount(component);
        component = null;
        document.body.innerHTML = "";
    });

    it("lists the word's elements and sets an attribute, the shorthand's ones read-only", async () => {
        const target = document.createElement("div");
        document.body.appendChild(target);
        let applied = 0;
        component = mount(WordMarkupPanel, { target, props: { wordIndex: 4, onapplied: () => applied++ } });
        await settle();
        expect(target.textContent).toContain("<s>");
        expect(target.textContent).toContain("<lb>");
        const n = target.querySelector("#markup-1-value-0") as HTMLInputElement;
        expect(n.readOnly).toBe(true);

        // Add `ed="ms"` to the break
        const add = [...target.querySelectorAll("button")].filter((b) => b.textContent?.trim() === "Add an attribute")[1];
        add.click();
        flushSync();
        const name = target.querySelector("#markup-1-name-1") as HTMLInputElement;
        name.value = "ed";
        name.dispatchEvent(new Event("change", { bubbles: true }));
        const value = target.querySelector("#markup-1-value-1") as HTMLInputElement;
        value.value = "ms";
        value.dispatchEvent(new Event("change", { bubbles: true }));
        flushSync();
        const apply = [...target.querySelectorAll("button")].filter((b) => b.textContent?.trim() === "Apply")[1];
        expect(apply.disabled).toBe(false);
        apply.click();
        await settle();
        expect(set).toHaveBeenCalledWith("lb", 2, [["n", "3"], ["ed", "ms"]]);
        expect(applied).toBe(1);
    });
});
