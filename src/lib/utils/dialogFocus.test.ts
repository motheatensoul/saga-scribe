import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { dialogFocus } from "./dialogFocus";

/** jsdom lays nothing out: report one box for every element so they count as visible. */
function pretendLaidOut() {
    vi.spyOn(HTMLElement.prototype, "getClientRects").mockReturnValue([
        new DOMRect(0, 0, 10, 10),
    ] as unknown as DOMRectList);
}

function setUp(dialogHtml: string) {
    document.body.innerHTML = `
        <button id="opener">Open</button>
        <div id="dialog" role="dialog" tabindex="-1">${dialogHtml}</div>
    `;
    const opener = document.getElementById("opener") as HTMLButtonElement;
    const dialog = document.getElementById("dialog") as HTMLElement;
    opener.focus();
    return { opener, dialog };
}

function tab(dialog: HTMLElement, shiftKey = false) {
    const event = new KeyboardEvent("keydown", { key: "Tab", shiftKey, bubbles: true, cancelable: true });
    (document.activeElement ?? dialog).dispatchEvent(event);
    return event;
}

describe("dialogFocus", () => {
    beforeEach(() => {
        pretendLaidOut();
        vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
            callback(0);
            return 1;
        });
        vi.stubGlobal("cancelAnimationFrame", () => {});
    });

    afterEach(() => {
        vi.restoreAllMocks();
        vi.unstubAllGlobals();
        document.body.innerHTML = "";
    });

    it("focuses the first control that isn't a close button", () => {
        const { dialog } = setUp(`
            <button aria-label="Close settings">×</button>
            <input id="first" />
            <button id="ok">OK</button>
        `);
        dialogFocus(dialog);
        expect(document.activeElement?.id).toBe("first");
    });

    it("prefers a control marked data-autofocus", () => {
        const { dialog } = setUp(`
            <input id="first" />
            <input id="search" data-autofocus />
        `);
        dialogFocus(dialog);
        expect(document.activeElement?.id).toBe("search");
    });

    it("skips a marked control that is disabled or hidden", () => {
        const { dialog } = setUp(`
            <input id="first" />
            <div class="hidden"><input id="search" data-autofocus /></div>
        `);
        dialogFocus(dialog);
        expect(document.activeElement?.id).toBe("first");
    });

    it("focuses the dialog itself when it has no controls", () => {
        const { dialog } = setUp(`<p>Nothing to press</p>`);
        dialogFocus(dialog);
        expect(document.activeElement).toBe(dialog);
    });

    it("keeps Tab and Shift+Tab inside the dialog", () => {
        const { dialog } = setUp(`
            <input id="first" />
            <button id="last">OK</button>
        `);
        dialogFocus(dialog);
        (document.getElementById("last") as HTMLElement).focus();
        expect(tab(dialog).defaultPrevented).toBe(true);
        expect(document.activeElement?.id).toBe("first");

        expect(tab(dialog, true).defaultPrevented).toBe(true);
        expect(document.activeElement?.id).toBe("last");
    });

    it("leaves Tab between controls in the middle to the browser", () => {
        const { dialog } = setUp(`
            <input id="first" />
            <input id="middle" />
            <button id="last">OK</button>
        `);
        dialogFocus(dialog);
        expect(tab(dialog).defaultPrevented).toBe(false);
    });

    it("gives focus back to the opener on closing", async () => {
        const { opener, dialog } = setUp(`<input id="first" />`);
        const cleanup = dialogFocus(dialog);
        expect(document.activeElement?.id).toBe("first");
        cleanup();
        dialog.remove();
        await Promise.resolve();
        expect(document.activeElement).toBe(opener);
    });

    it("doesn't take focus from a control the dialog focused itself", () => {
        const { dialog } = setUp(`<input id="first" /><input id="second" />`);
        (document.getElementById("second") as HTMLElement).focus();
        dialogFocus(dialog);
        expect(document.activeElement?.id).toBe("second");
    });
});
