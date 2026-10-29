import { describe, expect, it } from "vitest";
import { withExtension } from "$lib/utils/saveDialog";

describe("withExtension", () => {
    it("appends the first extension to a bare name", () => {
        expect(withExtension("/home/user/export", ["pdf"])).toBe("/home/user/export.pdf");
        expect(withExtension("/home/user/page", ["html", "htm"])).toBe("/home/user/page.html");
    });

    it("keeps any accepted extension, ignoring case", () => {
        expect(withExtension("/home/user/page.htm", ["html", "htm"])).toBe("/home/user/page.htm");
        expect(withExtension("C:\\Users\\me\\EXPORT.PDF", ["pdf"])).toBe("C:\\Users\\me\\EXPORT.PDF");
    });

    it("appends after a different extension or a dotted name", () => {
        expect(withExtension("/home/user/HolmPerg.teis", ["pdf"])).toBe("/home/user/HolmPerg.teis.pdf");
        expect(withExtension("/home/user/v1.2", ["xml"])).toBe("/home/user/v1.2.xml");
    });
});
