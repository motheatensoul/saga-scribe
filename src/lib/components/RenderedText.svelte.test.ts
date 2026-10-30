import { afterEach, describe, expect, it } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import RenderedText from "$lib/components/RenderedText.svelte";
import { word } from "$lib/rendered/fixtures";
import { NO_MARKS, type WordMarks } from "$lib/rendered/marks";

const tei = (body: string) =>
    `<TEI xmlns="http://www.tei-c.org/ns/1.0" xmlns:me="http://www.menota.org/ns/1.0"><text><body><p>${body}</p></body></text></TEI>`;
const confirmed = (...words: number[]): WordMarks => ({ ...NO_MARKS, confirmed: new Set(words) });

async function settle() {
    for (let i = 0; i < 5; i++) {
        flushSync();
        await new Promise((resolve) => setTimeout(resolve, 0));
    }
}

/** Each shown word with a * if marked confirmed */
const shown = (target: HTMLElement) =>
    [...target.querySelectorAll(".word-token")].map(
        (w) => w.textContent!.trim() + (w.classList.contains("is-confirmed") ? "*" : ""),
    );

describe("RenderedText", () => {
    let component: Record<string, unknown> | null = null;
    afterEach(() => {
        if (component) unmount(component);
        component = null;
    });

    it("marks the words of the document it shows, numbered as that document is", async () => {
        const target = document.createElement("div");
        document.body.appendChild(target);
        const props = $state({ content: tei(word("ok") + word("sva")), marks: confirmed(1), active: true });
        component = mount(RenderedText, { target, props });
        await settle();
        expect(shown(target)).toEqual(["ok", "sva*"]);

        // A word inserted before them: "sva" is word 2 now
        props.content = tei(word("ek") + word("ok") + word("sva"));
        props.marks = confirmed(2);
        flushSync();
        // Until the new document is drawn, the old one keeps its own marks
        expect(shown(target)).toEqual(["ok", "sva*"]);
        await settle();
        expect(shown(target)).toEqual(["ek", "ok", "sva*"]);

        // Only the marks changed (a lemma confirmed): shown at once
        props.marks = confirmed(0, 2);
        await settle();
        expect(shown(target)).toEqual(["ek*", "ok", "sva*"]);
    });
});
