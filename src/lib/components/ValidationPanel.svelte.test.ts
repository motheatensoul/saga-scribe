import { afterEach, describe, expect, it, vi } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import type { WordRecord } from "$lib/tauri";

vi.mock("$lib/tauri", async (original) => ({
    ...(await original<typeof import("$lib/tauri")>()),
    listSchemas: async () => [],
}));

const { default: ValidationPanel } = await import("$lib/components/ValidationPanel.svelte");

const record = (key: number, word: number | null, from: number, to: number): WordRecord => ({
    key,
    suggestion: "different",
    decision: null,
    word,
    from,
    to,
    was: "konungr",
    xml: "<w lemma=\"konungr\">konungr</w>",
    lemma: "konungr",
    msa: null,
});

describe("ValidationPanel", () => {
    let component: Record<string, unknown> | null = null;
    afterEach(() => {
        if (component) unmount(component);
        component = null;
    });

    it("lists replaced words still to be decided and opens them in the word panel", () => {
        const target = document.createElement("div");
        document.body.appendChild(target);
        const opened: number[] = [];
        component = mount(ValidationPanel, {
            target,
            props: {
                xmlContent: "",
                records: [record(3, 7, 4, 12), record(5, null, 13, 15)],
                recordsDsl: "sá: dróttinn ok",
                onopenword: (word: number) => opened.push(word),
            },
        });
        flushSync();

        const items = target.querySelectorAll("li");
        expect(items).toHaveLength(2);
        expect(items[0].textContent).toContain("konungr → dróttinn");
        expect(target.textContent).toContain("Replaced words to decide (2)");
        // Only a record with a word in the text can be opened
        const buttons = target.querySelectorAll("li button");
        expect(buttons).toHaveLength(1);
        (buttons[0] as HTMLButtonElement).click();
        expect(opened).toEqual([7]);
    });

    it("shows no list without open records", () => {
        const target = document.createElement("div");
        document.body.appendChild(target);
        component = mount(ValidationPanel, { target, props: { xmlContent: "" } });
        flushSync();
        expect(target.querySelector("#validation-records-title")).toBeNull();
    });
});
