import { afterEach, describe, expect, it } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import ImportProfileDialog from "$lib/components/ImportProfileDialog.svelte";
import type { Level, Profile } from "$lib/tauri";

const profile: Profile = {
    levels: ["facs", "dipl", "norm"],
    choice: true,
    abbreviations: "am_ex",
    punctuation: "pc",
    lineEdition: null,
    counts: {
        words: 90,
        tokens: 100,
        levels: { facs: 100, dipl: 100, norm: 98 },
        wordsWithChoice: 100,
        am: 3,
        ex: 4,
        choiceAbbr: 0,
        pc: 10,
        mePunct: 0,
        lineEditions: { ms: 5 },
        hidden: { hi: 2 },
    },
};

describe("ImportProfileDialog", () => {
    let component: Record<string, unknown> | null = null;
    afterEach(() => {
        if (component) unmount(component);
        component = null;
        document.body.innerHTML = "";
    });

    function open(onconfirm: (levels: Level[], choice: boolean) => void) {
        const target = document.createElement("div");
        document.body.appendChild(target);
        component = mount(ImportProfileDialog, { target, props: { profile: structuredClone(profile), onconfirm } });
        flushSync();
        return target;
    }

    it("shows the file's levels and the markup the shorthand doesn't show", () => {
        const target = open(() => {});
        expect(target.querySelector('[role="dialog"]')).not.toBeNull();
        expect(target.textContent).toContain("in 98% of words, numerals and punctuation");
        expect(target.textContent).toContain("<hi> 2");
    });

    it("reports changed levels when confirmed, and nothing when unchanged", () => {
        const calls: [Level[], boolean][] = [];
        let target = open((levels, choice) => calls.push([levels, choice]));
        const button = () => [...target.querySelectorAll("button")].find((b) => b.textContent?.trim() === "Confirm")!;
        button().click();
        flushSync();
        expect(calls).toEqual([]);
        expect(target.querySelector('[role="dialog"]')).toBeNull();

        unmount(component!);
        target = open((levels, choice) => calls.push([levels, choice]));
        const norm = [...target.querySelectorAll("label")].find((l) => l.textContent?.includes("Normalized"))!;
        norm.querySelector("input")!.click();
        flushSync();
        button().click();
        flushSync();
        expect(calls).toEqual([[["facs", "dipl"], true]]);
    });
});
