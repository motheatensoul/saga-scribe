/**
 * Synthetic MENOTA documents for tests: small, deterministic, and covering
 * the constructs the renderer treats specially.
 */
import type { EntityMap } from "$lib/stores/entities";

export const testEntities: EntityMap = {
    slong: { unicode: "U+017F", char: "ſ", description: "LATIN SMALL LETTER LONG S", category: "letter" },
};

export function word(facs: string, dipl = facs, norm = facs, lemma?: string): string {
    const attr = lemma ? ` lemma="${lemma}"` : "";
    return `<w${attr}>\n\t<choice>\n\t\t<me:facs>${facs}</me:facs>\n\t\t<me:dipl>${dipl}</me:dipl>\n\t\t<me:norm>${norm}</me:norm>\n\t</choice>\n</w>`;
}

const pc = "<pc>\n\t<choice>\n\t\t<me:facs>.</me:facs>\n\t\t<me:dipl>.</me:dipl>\n\t\t<me:norm>.</me:norm>\n\t</choice>\n</pc>";

/** A document with `pages` pages; paragraphs change between some pages, so nesting varies. */
export function syntheticDocument(pages: number): string {
    const parts: string[] = [];
    for (let p = 1; p <= pages; p++) {
        if (p > 1 && p % 3 === 0) parts.push("</s>\n</p>\n<p>\n<s>");
        parts.push(`<pb n="${p}r"/>`);
        if (p % 5 === 1) parts.push(`<head>${word("Upphaf")}</head>`);
        for (let line = 1; line <= 6; line++) {
            parts.push(`<lb n="${line}"/>`);
            parts.push(word(`maþr${p}x${line}`, "maþr", "maðr", "maðr"));
            parts.push(word("&slong;ꝥ", "sꝥ", "sá"));
            if (line === 2) parts.push(`<add place="supralinear">\n${word("a", "a", "á", "á")}\n</add>`);
            if (line === 3) parts.push(`<choice>\n<sic>\n${word("hemt")}\n</sic>\n<corr>\n${word("beiddi")}\n</corr>\n</choice>`);
            if (line === 4) parts.push(`<supplied>\n${word("ok")}\n</supplied>`);
            parts.push(pc);
        }
    }
    return (
        '<TEI xmlns="http://www.tei-c.org/ns/1.0" xmlns:me="http://www.menota.org/ns/1.0"><text><body>\n' +
        "<div>\n<p>\n<s>\n" +
        parts.join("\n") +
        "\n</s>\n</p>\n</div>\n</body></text></TEI>"
    );
}
