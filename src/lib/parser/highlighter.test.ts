import { describe, expect, it } from "vitest";
import { highlightTree } from "@lezer/highlight";
import { teiDslLanguage, teiDslHighlightStyle } from "./highlighter";

/** The highlight class of each highlighted stretch of `dsl`, as "text: class" */
function highlighted(dsl: string): string[] {
    const tree = teiDslLanguage.parser.parse(dsl);
    const out: string[] = [];
    highlightTree(tree, teiDslHighlightStyle, (from, to, classes) => {
        out.push(`${dsl.slice(from, to)}: ${classes}`);
    });
    return out;
}

describe("highlighting", () => {
    it("styles every construct", () => {
        expect(
            highlighted("///1r //2 .p .head{H} .abbr[k]{konungr} .supplied{s} .norm{n} [...2] <a> -{d}- +{e}+ ^{f} ?{g}? :et: a|b upp~haf hel~//3lo"),
        ).toEqual([
            "///1r: cmt-processing",
            "//2: cmt-processing",
            ".p: cmt-processing",
            ".head: cmt-keyword",
            "{H}: cmt-keyword cmt-string",
            ".abbr: cmt-keyword",
            "[k]: cmt-keyword cmt-string",
            "{konungr}: cmt-keyword cmt-string",
            ".supplied: cmt-inserted",
            "{s}: cmt-inserted cmt-string",
            ".norm: cmt-keyword",
            "{n}: cmt-keyword cmt-string",
            "[...2]: cmt-gap",
            "<a>: cmt-inserted",
            "-{d}-: cmt-deleted",
            "+{e}+: cmt-changed",
            "^{f}: cmt-comment",
            "?{g}?: cmt-emphasis",
            ":et:: cmt-atom",
            "|: cmt-separator",
            "~: cmt-separator",
            "~//3: cmt-separator",
        ]);
    });

    it("leaves an unclosed construct as text, as the compiler reads it", () => {
        expect(highlighted("-{open <a>")).toEqual(["<a>: cmt-inserted"]);
    });
});
