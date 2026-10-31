import { describe, expect, it } from "vitest";
import { TreeFragment, type Tree } from "@lezer/common";
import { parser } from "./tei-dsl-parser.js";
import corpus from "../../../tests/fixtures/dsl-corpus.json";

/**
 * The editor's grammar against the shared DSL corpus: it must find the
 * constructs the Rust parser finds (see src-tauri/src/parser/corpus_tests.rs).
 */

type Construct = [string, number, number];

/** Grammar node names, as the Rust parser names them */
const NAMES: Record<string, string> = {
    Text: "Text",
    LineBreak: "LineBreak",
    PageBreak: "PageBreak",
    ParagraphBreak: "ParagraphBreak",
    Head: "Head",
    Abbreviation: "Abbreviation",
    SuppliedBlock: "SuppliedBlock",
    NormBlock: "Norm",
    Gap: "Gap",
    Supplied: "Supplied",
    Deletion: "Deletion",
    Addition: "Addition",
    Note: "Note",
    Unclear: "Unclear",
    Entity: "Entity",
    WordBoundary: "WordBoundary",
    CompoundJoin: "CompoundJoin",
};

/** Top-level constructs the grammar finds, in the corpus's terms */
function constructs(dsl: string): Construct[] {
    const out: Construct[] = [];
    const push = (name: string, from: number, to: number) => {
        const last = out[out.length - 1];
        if (name === "Text" && last?.[0] === "Text" && last[2] === from) {
            last[2] = to;
        } else {
            out.push([name, from, to]);
        }
    };
    const cursor = parser.parse(dsl).cursor();
    if (!cursor.firstChild()) return out;
    do {
        const { name, from, to } = cursor;
        // One token in the grammar, two constructs in the parser
        if (name === "WordContinuationLineBreak" || name === "WordContinuationPageBreak") {
            push("WordContinuation", from, from + 1);
            push(name === "WordContinuationLineBreak" ? "LineBreak" : "PageBreak", from + 1, to);
        } else {
            push(NAMES[name] ?? `unexpected ${name}`, from, to);
        }
    } while (cursor.nextSibling());
    return out;
}

describe("the editor's grammar reads the DSL as the compiler does", () => {
    for (const [group, cases] of [
        ["written", corpus.cases],
        ["generated", corpus.generated],
    ] as const) {
        it(`on the ${group} cases`, () => {
            // Compact, as "Name 0-5", so a failure lists every difference readably
            const show = (list: Construct[]) => list.map(([name, from, to]) => `${name} ${from}-${to}`).join(", ");
            const mismatches = cases
                .map(({ dsl, nodes }) => ({ dsl, compiler: show(nodes as Construct[]), editor: show(constructs(dsl)) }))
                .filter(({ compiler, editor }) => compiler !== editor);
            expect(mismatches).toEqual([]);
        });
    }
});

describe("the editor's grammar after edits", () => {
    it("reparses incrementally as it parses from scratch", () => {
        const shape = (tree: Tree) => {
            const out: string[] = [];
            tree.iterate({ enter: (node) => void out.push(`${node.name} ${node.from}-${node.to}`) });
            return out.join(", ");
        };
        // Edits that open, close and break constructs far from where they are
        const pieces = ["-{", "}-", "<", ">", ".head{", "}", ".abbr[", "]{", "[...", "]", ":", "~", "//", " ", ".p ", "x"];
        let state = 12345;
        const random = (n: number) => {
            state = (state * 1103515245 + 12345) % 2 ** 31;
            return state % n;
        };
        let doc = corpus.cases.map(({ dsl }) => dsl).join("\n");
        let tree = parser.parse(doc);
        let fragments = TreeFragment.addTree(tree);
        for (let i = 0; i < 300; i++) {
            const from = random(doc.length + 1);
            const to = Math.min(doc.length, from + random(4));
            const insert = pieces[random(pieces.length)];
            doc = doc.slice(0, from) + insert + doc.slice(to);
            fragments = TreeFragment.applyChanges(fragments, [{ fromA: from, toA: to, fromB: from, toB: from + insert.length }]);
            tree = parser.parse(doc, fragments);
            fragments = TreeFragment.addTree(tree, fragments);
            expect(shape(tree)).toBe(shape(parser.parse(doc)));
        }
    });
});
