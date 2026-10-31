/// <reference types="node" />
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { renderFull, renderUpdate, type RenderedState } from "$lib/rendered/incremental";
import { syntheticDocument, testEntities, word } from "$lib/rendered/fixtures";
import type { EntityMap } from "$lib/stores/entities";

/** Deterministic pseudo-random numbers (high bits of an LCG). */
function random(seed: number) {
    return (n: number) => {
        seed = (seed * 1103515245 + 12345) % 2147483648;
        return Math.floor((seed / 2147483648) * n);
    };
}

type Edit = { label: string; structural: boolean; apply: (xml: string, rnd: (n: number) => number) => string };

function pick(xml: string, re: RegExp, rnd: (n: number) => number) {
    const body = xml.indexOf("<body");
    const all = [...xml.slice(body).matchAll(re)];
    const m = all[rnd(all.length)];
    return { at: body + (m.index ?? 0), text: m[0] };
}

const edits: Edit[] = [
    { label: "word text", structural: false, apply: (s, r) => { const { at, text } = pick(s, /<me:facs>[^<]{1,20}<\/me:facs>/g, r); return s.slice(0, at) + `<me:facs>zz${r(99)}</me:facs>` + s.slice(at + text.length); } },
    { label: "insert word", structural: false, apply: (s, r) => { const { at, text } = pick(s, /<\/w>/g, r); return s.slice(0, at + text.length) + "\n" + word(`new${r(9)}`) + s.slice(at + text.length); } },
    { label: "delete word", structural: false, apply: (s, r) => { const { at, text } = pick(s, /<w[ >][\s\S]{0,400}?<\/w>/g, r); return s.slice(0, at) + s.slice(at + text.length); } },
    { label: "insert line break", structural: false, apply: (s, r) => { const { at, text } = pick(s, /<\/w>/g, r); return s.slice(0, at + text.length) + `<lb n="9${r(9)}"/>` + s.slice(at + text.length); } },
    { label: "edit attribute", structural: false, apply: (s, r) => { const { at, text } = pick(s, /lemma="[^"]*"/g, r); return s.slice(0, at) + `lemma="x${r(9)}"` + s.slice(at + text.length); } },
    { label: "insert page break", structural: true, apply: (s, r) => { const { at, text } = pick(s, /<\/w>/g, r); return s.slice(0, at + text.length) + `<pb n="new${r(99)}"/>` + s.slice(at + text.length); } },
];

/** Applies random edits; after each, the incremental result must equal a full parse. */
async function checkEquivalence(start: string, entities: EntityMap, steps: number, seed: number) {
    const rnd = random(seed);
    let state: RenderedState = await renderFull(start, entities);
    let content = start;
    const modes = { incremental: 0, full: 0 };
    for (let step = 0; step < steps; step++) {
        const edit = edits[rnd(edits.length)];
        content = edit.apply(content, rnd);
        const next = await renderUpdate(state, content, entities);
        const reference = await renderFull(content, entities);
        expect(next.doc, `step ${step}: ${edit.label}`).toEqual(reference.doc);
        if (edit.structural) expect(next.mode, `step ${step}: ${edit.label}`).toBe("full");
        modes[next.mode]++;
        state = next;
    }
    return modes;
}

describe("renderUpdate", () => {
    it("matches a full parse after random edits, re-parsing only edited pages", async () => {
        const modes = await checkEquivalence(syntheticDocument(30), testEntities, 80, 4242);
        // Most edits are local; nearly all must take the incremental path
        expect(modes.incremental).toBeGreaterThan(50);
    });

    it("re-parses only the edited page for an edit right next to a page break", async () => {
        const xml = syntheticDocument(10);
        const state = await renderFull(xml, testEntities);
        const pb = xml.indexOf("<pb", xml.indexOf("<pb") + 1);
        const pbEnd = xml.indexOf("/>", pb) + 2;
        for (const edited of [
            xml.slice(0, pbEnd) + word("next") + xml.slice(pbEnd),
            xml.slice(0, pb) + word("prev") + xml.slice(pb),
        ]) {
            const next = await renderUpdate(state, edited, testEntities);
            expect(next.mode).toBe("incremental");
            expect(next.doc).toEqual((await renderFull(edited, testEntities)).doc);

            // And the word deleted again
            const back = await renderUpdate(next, xml, testEntities);
            expect(back.mode).toBe("incremental");
            expect(back.doc).toEqual(state.doc);
        }
    });

    it("returns the previous state when nothing changed", async () => {
        const state = await renderFull(syntheticDocument(3), testEntities);
        expect(await renderUpdate(state, state.content, testEntities)).toBe(state);
    });

    it("falls back to a full parse for broken markup and recovers afterwards", async () => {
        const xml = syntheticDocument(10);
        const state = await renderFull(xml, testEntities);
        const broken = xml.replace("</head>", "");
        const next = await renderUpdate(state, broken, testEntities);
        expect(next.mode).toBe("full");
        expect(next.doc).toEqual((await renderFull(broken, testEntities)).doc);
    });

    // Opt-in (HOLMPERG=1): the real 8 MB manuscript; slow in jsdom
    // Paths relative to the project root, where vitest runs
    const holmperg = join(process.cwd(), "tests/fixtures/HolmPerg-34-4to-MLL.xml");
    it.runIf(process.env.HOLMPERG === "1" && existsSync(holmperg))(
        "matches a full parse on the HolmPerg manuscript",
        async () => {
            const menota = JSON.parse(
                readFileSync(join(process.cwd(), "static/entities/menota.json"), "utf8"),
            ).entities as EntityMap;
            await checkEquivalence(readFileSync(holmperg, "utf8"), menota, 10, 777);
        },
    );
});

describe("abandoned renders", () => {
    it("stop at their next pause instead of finishing", async () => {
        const controller = new AbortController();
        const render = renderFull(syntheticDocument(40), testEntities, controller.signal);
        controller.abort();
        await expect(render).rejects.toThrow();
    });
});
