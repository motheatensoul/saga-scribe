import { afterEach, describe, expect, it } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import { get } from "svelte/store";
import AbbreviationsDialog from "$lib/components/AbbreviationsDialog.svelte";
import { abbreviationStore } from "$lib/stores/abbreviations";

describe("AbbreviationsDialog", () => {
    let component: Record<string, unknown> | null = null;
    afterEach(() => {
        if (component) unmount(component);
        component = null;
        document.body.innerHTML = "";
        abbreviationStore.clear();
    });

    it("lists the store, changes a default, removes and adds expansions", () => {
        abbreviationStore.load({ ":kbar:": ["konun", "kon"], ":et:": ["ok"] });
        const target = document.createElement("div");
        document.body.appendChild(target);
        component = mount(AbbreviationsDialog, { target, props: { isopen: true } });
        flushSync();
        expect(target.textContent).toContain(":kbar:");
        expect(target.textContent).toContain("(default)");

        const kon = [...target.querySelectorAll("label")].find((l) => l.textContent?.trim().startsWith("kon") && !l.textContent.includes("konun"))!;
        kon.querySelector("input")!.click();
        flushSync();
        expect(get(abbreviationStore)[":kbar:"]).toEqual(["kon", "konun"]);

        (target.querySelector('[aria-label="Remove ok from :et:"]') as HTMLButtonElement).click();
        flushSync();
        expect(get(abbreviationStore)[":et:"]).toBeUndefined();

        const mark = target.querySelector("#abbreviation-mark") as HTMLInputElement;
        const expansion = target.querySelector("#abbreviation-expansion") as HTMLInputElement;
        mark.value = ":thornbar:";
        mark.dispatchEvent(new Event("input"));
        expansion.value = "þat";
        expansion.dispatchEvent(new Event("input"));
        flushSync();
        (target.querySelector('button[type="submit"]') as HTMLButtonElement).click();
        flushSync();
        expect(get(abbreviationStore)[":thornbar:"]).toEqual(["þat"]);
        expect(abbreviationStore.changedByEditor()).toBe(true);
    });
});
