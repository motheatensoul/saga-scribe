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

    it("compiles only the newest of the requests made while one runs, with all their changes", async () => {
        compileSession.reset();
        const compiled: string[] = [];
        let release!: () => void;
        const gate = new Promise<void>((resolve) => (release = resolve));
        const first = compileSession.run("ab", async () => {
            await gate;
            compiled.push("ab");
            return { revision: 1 };
        });

        // Typed on while the first compiles: "abc", then "abcd"
        compileSession.record(ChangeSet.of({ from: 2, insert: "c" }, 2));
        const second = compileSession.run("abc", async () => {
            compiled.push("abc");
            return { revision: 2 };
        });
        compileSession.record(ChangeSet.of({ from: 3, insert: "d" }, 3));
        let guidance: CompileGuidance | undefined;
        const third = compileSession.run("abcd", async (g) => {
            guidance = g;
            compiled.push("abcd");
            return { revision: 3 };
        });

        release();
        const results = await Promise.all([first, second, third]);
        expect(compiled).toEqual(["ab", "abcd"]);
        // The replaced request's caller gets the newer document's result
        expect(results.map((r) => r.revision)).toEqual([1, 3, 3]);
        expect(guidance).toEqual({
            baseRevision: 1,
            changes: [{ from_a: 2, to_a: 2, from_b: 2, to_b: 4 }],
        });
    });

    it("puts the document back together from the changes the backend sends", async () => {
        compileSession.reset();
        const bases: (number | undefined)[] = [];
        const first = await compileSession.run("a", async (_, xmlBase) => {
            bases.push(xmlBase);
            return { revision: 1, xml: "<w>a</w>" };
        });
        const second = await compileSession.run("ab", async (_, xmlBase) => {
            bases.push(xmlBase);
            return { revision: 2, delta: { from: 4, to: 4, insert: "b" } };
        });
        expect([first.xml, second.xml]).toEqual(["<w>a</w>", "<w>ab</w>"]);
        // Nothing held at first; then the first compile's output
        expect(bases).toEqual([undefined, 1]);

        // After a reset, nothing is held: the next is sent whole
        compileSession.reset();
        await compileSession.run("x", async (_, xmlBase) => {
            bases.push(xmlBase);
            return { revision: 1, xml: "<w>x</w>" };
        });
        expect(bases[2]).toBeUndefined();
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
