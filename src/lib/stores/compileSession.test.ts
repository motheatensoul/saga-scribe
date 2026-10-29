import { describe, expect, it } from "vitest";
import { ChangeSet } from "@codemirror/state";
import { compileSession } from "$lib/stores/compileSession";
import type { CompileGuidance } from "$lib/tauri";

describe("compileSession", () => {
    it("applies a renumbering before the next queued compile gathers its payload", async () => {
        compileSession.reset();
        const events: string[] = [];
        compileSession.onRenumbering = () => events.push("renumbered");

        let release!: () => void;
        const gate = new Promise<void>((resolve) => (release = resolve));
        const first = compileSession.run("ab", async () => {
            await gate;
            events.push("first done");
            return { revision: 1, renumbering: [1] };
        });
        // Queued while the first is running
        const second = compileSession.run("ab", async () => {
            events.push("second gathers payload");
            return { revision: 2 };
        });
        release();
        await Promise.all([first, second]);
        expect(events).toEqual(["first done", "renumbered", "second gathers payload"]);
    });

    it("sends no guidance after a reset, and the changes since otherwise", async () => {
        compileSession.reset();
        const seen: (CompileGuidance | undefined)[] = [];
        const compile = (revision: number) => async (guidance?: CompileGuidance) => {
            seen.push(guidance);
            return { revision };
        };
        await compileSession.run("ab", compile(1));
        compileSession.record(ChangeSet.of({ from: 2, insert: "c" }, 2));
        await compileSession.run("abc", compile(2));
        expect(seen[0]).toBeUndefined();
        expect(seen[1]).toEqual({ baseRevision: 1, changes: [{ from_a: 2, to_a: 2, from_b: 2, to_b: 3 }] });
    });

    it("keeps a failed compile's changes for the next one", async () => {
        compileSession.reset();
        await compileSession.run("ab", async () => ({ revision: 1 }));
        compileSession.record(ChangeSet.of({ from: 0, insert: "x" }, 2));
        await expect(compileSession.run("xab", async () => { throw new Error("syntax"); })).rejects.toThrow();
        compileSession.record(ChangeSet.of({ from: 3, insert: "y" }, 3));
        let guidance: CompileGuidance | undefined;
        await compileSession.run("xaby", async (g) => {
            guidance = g;
            return { revision: 2 };
        });
        expect(guidance?.baseRevision).toBe(1);
        expect(guidance?.changes).toEqual([
            { from_a: 0, to_a: 0, from_b: 0, to_b: 1 },
            { from_a: 2, to_a: 2, from_b: 3, to_b: 4 },
        ]);
    });
});
