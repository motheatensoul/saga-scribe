/**
 * Manuscript metadata schema for TEI P5 and MENOTA documents.
 * Mirrors the Rust metadata module for type-safe IPC.
 */

/** Complete manuscript metadata covering TEI P5 header elements */
export interface Metadata {
    /** Title statement (titleStmt) */
    titleStmt: TitleStmt;

    /** Publication statement (publicationStmt) */
    publicationStmt: PublicationStmt;

    /** Source description - manuscript identifier (sourceDesc/msDesc/msIdentifier) */
    msIdentifier: MsIdentifier;

    /** Manuscript contents summary (msDesc/msContents) */
    msContents: MsContents;

    /** Physical description (msDesc/physDesc) */
    physDesc: PhysDesc;

    /** History of the manuscript (msDesc/history) */
    history: History;

    /** Language profile (profileDesc/langUsage) */
    languages: Language[];

    /** Encoding description notes (encodingDesc) */
    encodingNotes?: string;
}

/** Title statement - who created and is responsible for the digital edition */
export interface TitleStmt {
    /** Main title of the transcription */
    title?: string;

    /** Subtitle or alternative title */
    subtitle?: string;

    /** Original author of the text (if known) */
    author?: string;

    /** Modern editor(s) responsible for the transcription */
    editors: Person[];

    /** Other responsibility statements (transcriber, encoder, etc.) */
    respStmts: RespStmt[];
}

/** A person with optional identifiers */
export interface Person {
    /** Person's name */
    name: string;

    /** Optional ORCID, VIAF, or other identifier */
    identifier?: string;

    /** Identifier type (orcid, viaf, etc.) */
    identifierType?: string;
}

/** Responsibility statement - who did what */
export interface RespStmt {
    /** What they did (e.g., "Transcription", "Encoding", "Proofreading") */
    resp: string;

    /** Who did it */
    name: string;
}

/** Publication information for the digital edition */
export interface PublicationStmt {
    /** Publisher or institution */
    publisher?: string;

    /** Place of publication */
    pubPlace?: string;

    /** Publication date (ISO format preferred) */
    date?: string;

    /** Distributor (if different from publisher) */
    distributor?: string;

    /** License or availability statement */
    availability?: Availability;

    /** Identifier for the digital edition (DOI, URN, etc.) */
    idno?: string;

    /** Type of identifier */
    idnoType?: string;
}

/** License and availability information */
export interface Availability {
    /** Status: "free", "restricted", "unknown" */
    status?: string;

    /** License name (e.g., "CC BY 4.0") */
    license?: string;

    /** License URL */
    licenseUrl?: string;

    /** Additional availability notes */
    note?: string;
}

/** Manuscript identifier - where the physical manuscript is held */
export interface MsIdentifier {
    /** Country where manuscript is held */
    country?: string;

    /** City/settlement */
    settlement?: string;

    /** Institution/repository name */
    repository?: string;

    /** Collection name (if applicable) */
    collection?: string;

    /** Shelfmark/call number */
    idno?: string;

    /** Alternative identifiers */
    altIdentifiers: AltIdentifier[];

    /** Common name for the manuscript (e.g., "Codex Regius") */
    msName?: string;
}

/** Alternative identifier */
export interface AltIdentifier {
    /** Type of identifier (e.g., "former", "catalog") */
    idType: string;

    /** The identifier value */
    idno: string;
}

/** Summary of manuscript contents */
export interface MsContents {
    /** Brief summary of the manuscript contents */
    summary?: string;

    /** Primary language of the text (ISO 639 code) */
    textLang?: string;

    /** Additional language notes */
    textLangNote?: string;
}

/** Physical description of the manuscript */
export interface PhysDesc {
    /** Material (parchment, paper, etc.) */
    material?: string;

    /** Extent (number of leaves, dimensions) */
    extent?: string;

    /** Foliation description */
    foliation?: string;

    /** Layout description (columns, lines per page) */
    layout?: string;

    /** Description of scribal hands */
    hands: HandNote[];

    /** General condition */
    condition?: string;
}

/** Description of a scribal hand */
export interface HandNote {
    /** Hand identifier (e.g., "H1", "main") */
    id?: string;

    /** Script type (e.g., "carolingian", "gothic") */
    script?: string;

    /** Scope (e.g., "major", "minor", "sole") */
    scope?: string;

    /** Description */
    description?: string;
}

/** Historical information about the manuscript */
export interface History {
    /** When the manuscript was created */
    origDate?: DateRange;

    /** Where the manuscript was created */
    origPlace?: string;

    /** Provenance notes */
    provenance?: string;

    /** How the repository acquired it */
    acquisition?: string;
}

/** A date or date range */
export interface DateRange {
    /** Displayed date text (e.g., "ca. 1270-1280") */
    display?: string;

    /** ISO date for not-before (YYYY or YYYY-MM-DD) */
    notBefore?: string;

    /** ISO date for not-after */
    notAfter?: string;

    /** Single date if known exactly */
    when?: string;
}

/** Language declaration */
export interface Language {
    /** ISO 639 language code (e.g., "non" for Old Norse) */
    ident: string;

    /** Percentage of text in this language (optional) */
    usage?: number;

    /** Human-readable name */
    name?: string;
}

/** Create empty metadata with default values */
export function createEmptyMetadata(): Metadata {
    return {
        titleStmt: {
            title: undefined,
            subtitle: undefined,
            author: undefined,
            editors: [],
            respStmts: [],
        },
        publicationStmt: {
            publisher: undefined,
            pubPlace: undefined,
            date: undefined,
            distributor: undefined,
            availability: undefined,
            idno: undefined,
            idnoType: undefined,
        },
        msIdentifier: {
            country: undefined,
            settlement: undefined,
            repository: undefined,
            collection: undefined,
            idno: undefined,
            altIdentifiers: [],
            msName: undefined,
        },
        msContents: {
            summary: undefined,
            textLang: undefined,
            textLangNote: undefined,
        },
        physDesc: {
            material: undefined,
            extent: undefined,
            foliation: undefined,
            layout: undefined,
            hands: [],
            condition: undefined,
        },
        history: {
            origDate: undefined,
            origPlace: undefined,
            provenance: undefined,
            acquisition: undefined,
        },
        languages: [],
        encodingNotes: undefined,
    };
}

/** Check if metadata has any meaningful content */
export function isMetadataEmpty(metadata: Metadata): boolean {
    return (
        !metadata.titleStmt.title &&
        !metadata.titleStmt.author &&
        metadata.titleStmt.editors.length === 0 &&
        !metadata.msIdentifier.repository &&
        !metadata.msIdentifier.idno &&
        metadata.languages.length === 0
    );
}

/** Common Old Norse manuscript languages */
export const COMMON_LANGUAGES: Language[] = [
    { ident: "non", name: "Old Norse" },
    { ident: "is", name: "Icelandic" },
    { ident: "no", name: "Norwegian" },
    { ident: "da", name: "Danish" },
    { ident: "sv", name: "Swedish" },
    { ident: "lat", name: "Latin" },
    { ident: "gml", name: "Middle Low German" },
    { ident: "fro", name: "Old French" },
    { ident: "ang", name: "Old English" },
];

/** Common license options */
export const COMMON_LICENSES = [
    { id: "cc-by-4.0", name: "CC BY 4.0", url: "https://creativecommons.org/licenses/by/4.0/" },
    { id: "cc-by-sa-4.0", name: "CC BY-SA 4.0", url: "https://creativecommons.org/licenses/by-sa/4.0/" },
    { id: "cc-by-nc-4.0", name: "CC BY-NC 4.0", url: "https://creativecommons.org/licenses/by-nc/4.0/" },
    { id: "cc0", name: "CC0 (Public Domain)", url: "https://creativecommons.org/publicdomain/zero/1.0/" },
    { id: "other", name: "Other", url: "" },
];
