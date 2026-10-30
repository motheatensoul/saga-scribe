import { afterEach, describe, expect, it, vi } from "vitest";
import { markEdit, whenTypingPauses } from "./typing";

describe("whenTypingPauses", () => {
    afterEach(() => vi.useRealTimers());

    it("runs on the next turn when nobody is typing", () => {
        vi.useFakeTimers();
        markEdit(performance.now() - 1000);
        const work = vi.fn();
        whenTypingPauses(work, 200);
        expect(work).not.toHaveBeenCalled();
        vi.advanceTimersByTime(0);
        expect(work).toHaveBeenCalledOnce();
    });

    it("waits until no edit was made for the pause, however long typing goes on", () => {
        vi.useFakeTimers();
        const work = vi.fn();
        markEdit();
        whenTypingPauses(work, 200);
        for (let i = 0; i < 5; i++) {
            vi.advanceTimersByTime(150);
            markEdit();
        }
        expect(work).not.toHaveBeenCalled();
        vi.advanceTimersByTime(199);
        expect(work).not.toHaveBeenCalled();
        vi.advanceTimersByTime(1);
        expect(work).toHaveBeenCalledOnce();
    });

    it("can be cancelled", () => {
        vi.useFakeTimers();
        const work = vi.fn();
        markEdit();
        const cancel = whenTypingPauses(work, 200);
        cancel();
        vi.advanceTimersByTime(1000);
        expect(work).not.toHaveBeenCalled();
    });
});
