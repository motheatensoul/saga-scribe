import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { tablist } from "./tabs";

let cleanup: () => void;
let selected: string;

function key(name: string) {
    const event = new KeyboardEvent("keydown", { key: name, bubbles: true, cancelable: true });
    document.activeElement?.dispatchEvent(event);
    return event;
}

describe("tablist", () => {
    beforeEach(() => {
        document.body.innerHTML = `
            <div role="tablist">
                <button role="tab" id="a">A</button>
                <button role="tab" id="b">B</button>
                <button role="tab" id="c" disabled>C</button>
                <button role="tab" id="d">D</button>
            </div>
            <input id="outside" />
        `;
        selected = "a";
        for (const tab of document.querySelectorAll<HTMLElement>('[role="tab"]')) {
            tab.addEventListener("click", () => (selected = tab.id));
        }
        cleanup = tablist(document.querySelector('[role="tablist"]') as HTMLElement);
        (document.getElementById("a") as HTMLElement).focus();
    });

    afterEach(() => {
        cleanup();
        document.body.innerHTML = "";
    });

    it("moves to and selects the next tab, skipping disabled ones", () => {
        expect(key("ArrowRight").defaultPrevented).toBe(true);
        expect(document.activeElement?.id).toBe("b");
        expect(selected).toBe("b");
        key("ArrowRight");
        expect(selected).toBe("d");
    });

    it("wraps around at either end", () => {
        key("ArrowLeft");
        expect(selected).toBe("d");
        key("ArrowRight");
        expect(selected).toBe("a");
    });

    it("jumps to the first and last tab with Home and End", () => {
        key("End");
        expect(selected).toBe("d");
        key("Home");
        expect(selected).toBe("a");
    });

    it("leaves other keys alone", () => {
        expect(key("ArrowDown").defaultPrevented).toBe(false);
        expect(selected).toBe("a");
    });
});
