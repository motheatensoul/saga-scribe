/**
 * Store for preserved XML sections during round-trip import/export.
 * When a MENOTA/TEI file is imported, these sections preserve the exact
 * original content for faithful reproduction on export.
 */

interface PreservedSections {
    /** Content from start of file to opening <body> tag */
    preamble: string | null;
    /** Content from closing </body> tag to end of file */
    postamble: string | null;
    /** Original <front> matter content */
    frontMatter: string | null;
    /** Original <back> matter content */
    backMatter: string | null;
}

const initial: PreservedSections = {
    preamble: null,
    postamble: null,
    frontMatter: null,
    backMatter: null,
};

function createPreservationStore() {
    let _preamble = $state<string | null>(null);
    let _postamble = $state<string | null>(null);
    let _frontMatter = $state<string | null>(null);
    let _backMatter = $state<string | null>(null);

    return {
        get preamble() { return _preamble; },
        get postamble() { return _postamble; },
        get frontMatter() { return _frontMatter; },
        get backMatter() { return _backMatter; },

        /** True if any preserved sections exist (file was imported from XML) */
        get hasPreservedSections() {
            return _preamble !== null || _postamble !== null;
        },

        /** Set all preserved sections at once (typically on import) */
        setSections(sections: Partial<PreservedSections>) {
            _preamble = sections.preamble ?? null;
            _postamble = sections.postamble ?? null;
            _frontMatter = sections.frontMatter ?? null;
            _backMatter = sections.backMatter ?? null;
        },

        /** Clear all preserved sections (new file, etc) */
        clear() {
            _preamble = null;
            _postamble = null;
            _frontMatter = null;
            _backMatter = null;
        },

        /** Get all sections for saving to project file */
        getAllSections(): PreservedSections {
            return {
                preamble: _preamble,
                postamble: _postamble,
                frontMatter: _frontMatter,
                backMatter: _backMatter,
            };
        },
    };
}

export const preservationStore = createPreservationStore();
