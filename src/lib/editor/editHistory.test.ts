import { describe, expect, it } from "vitest";
import { ChangeSet, Text } from "@codemirror/state";
import { EditHistory } from "$lib/editor/editHistory";

/** Applies a change to `doc`, recording it the way the editor does. */
function edit(history: EditHistory, doc: string, from: number, to: number, insert: string): string {
    const changes = ChangeSet.of({ from, to, insert }, doc.length);
    history.record(changes);
    return changes.apply(Text.of(doc.split("\n"))).toString();
}

describe("EditHistory", () => {
    it("knows nothing before the first request", () => {
        const history = new EditHistory();
        edit(history, "ok ok", 0, 3, "");
        const first = history.begin(2);
        expect(history.changesFor(first)).toBeNull();
    });

    it("composes the changes since the confirmed compile", () => {
        const history = new EditHistory();
        let doc = "ok ok maþr";
        const first = history.begin(doc.length);
        history.confirm(first);

        doc = edit(history, doc, 0, 3, ""); // "ok maþr"
        doc = edit(history, doc, 2, 2, "x"); // "okx maþr"
        const second = history.begin(doc.length);
        expect(history.changesFor(second)).toEqual([
            { from_a: 0, to_a: 3, from_b: 0, to_b: 0 },
            { from_a: 5, to_a: 5, from_b: 2, to_b: 3 },
        ]);
    });

    it("keeps a failed request's changes for the next one", () => {
        const history = new EditHistory();
        let doc = "a b c";
        history.confirm(history.begin(doc.length));

        doc = edit(history, doc, 0, 2, ""); // "b c"
        history.begin(doc.length); // fails: never confirmed
        doc = edit(history, doc, 3, 3, " d"); // "b c d"
        const next = history.begin(doc.length);
        expect(history.changesFor(next)).toEqual([
            { from_a: 0, to_a: 2, from_b: 0, to_b: 0 },
            { from_a: 5, to_a: 5, from_b: 3, to_b: 5 },
        ]);

        history.confirm(next);
        const idle = history.begin(doc.length);
        expect(history.changesFor(idle)).toEqual([]);
    });

    it("gives up on changes that don't fit the document", () => {
        const history = new EditHistory();
        history.confirm(history.begin(3));
        history.record(ChangeSet.of({ from: 0, insert: "x" }, 10)); // wrong length
        expect(history.changesFor(history.begin(11))).toBeNull();
    });
});
