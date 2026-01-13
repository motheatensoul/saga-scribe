<script lang="ts">
    import { X as CloseButton, ChevronDown, ChevronRight, Plus, Trash2 } from "@lucide/svelte";
    import type {
        Metadata,
        Person,
        RespStmt,
        HandNote,
        Language,
        AltIdentifier,
    } from "$lib/types/metadata";
    import {
        createEmptyMetadata,
        COMMON_LANGUAGES,
        COMMON_LICENSES,
    } from "$lib/types/metadata";

    let {
        isopen = $bindable(false),
        metadata = $bindable<Metadata | undefined>(undefined),
        onSave,
    }: {
        isopen?: boolean;
        metadata?: Metadata;
        onSave?: (metadata: Metadata) => void;
    } = $props();

    // Local copy for editing
    let editData = $state<Metadata>(createEmptyMetadata());
    let isSaving = $state(false);

    // Collapsible sections
    let sections = $state({
        title: true,
        publication: false,
        manuscript: true,
        contents: false,
        physical: false,
        history: false,
        languages: false,
    });

    // Initialize edit data when metadata changes or modal opens
    $effect(() => {
        if (isopen) {
            if (metadata) {
                // Deep clone to avoid mutating original
                editData = JSON.parse(JSON.stringify(metadata));
            } else {
                editData = createEmptyMetadata();
            }
        }
    });

    function handleClose() {
        isopen = false;
    }

    function handleBackdropClick(e: MouseEvent) {
        if (e.target === e.currentTarget) {
            handleClose();
        }
    }

    function handleKeydown(e: KeyboardEvent) {
        if (e.key === "Escape") {
            handleClose();
        }
    }

    function handleSave() {
        isSaving = true;
        try {
            metadata = editData;
            onSave?.(editData);
            isopen = false;
        } finally {
            isSaving = false;
        }
    }

    function toggleSection(section: keyof typeof sections) {
        sections[section] = !sections[section];
    }

    // Array helpers
    function addEditor() {
        editData.titleStmt.editors = [
            ...editData.titleStmt.editors,
            { name: "", identifier: undefined, identifierType: undefined },
        ];
    }

    function removeEditor(index: number) {
        editData.titleStmt.editors = editData.titleStmt.editors.filter(
            (_, i) => i !== index,
        );
    }

    function addRespStmt() {
        editData.titleStmt.respStmts = [
            ...editData.titleStmt.respStmts,
            { resp: "", name: "" },
        ];
    }

    function removeRespStmt(index: number) {
        editData.titleStmt.respStmts = editData.titleStmt.respStmts.filter(
            (_, i) => i !== index,
        );
    }

    function addHand() {
        editData.physDesc.hands = [
            ...editData.physDesc.hands,
            { id: "", script: undefined, scope: undefined, description: undefined },
        ];
    }

    function removeHand(index: number) {
        editData.physDesc.hands = editData.physDesc.hands.filter(
            (_, i) => i !== index,
        );
    }

    function addLanguage() {
        editData.languages = [
            ...editData.languages,
            { ident: "", usage: undefined, name: undefined },
        ];
    }

    function removeLanguage(index: number) {
        editData.languages = editData.languages.filter((_, i) => i !== index);
    }

    function addAltIdentifier() {
        editData.msIdentifier.altIdentifiers = [
            ...editData.msIdentifier.altIdentifiers,
            { idType: "", idno: "" },
        ];
    }

    function removeAltIdentifier(index: number) {
        editData.msIdentifier.altIdentifiers =
            editData.msIdentifier.altIdentifiers.filter((_, i) => i !== index);
    }

    function selectLicense(licenseId: string) {
        const license = COMMON_LICENSES.find((l) => l.id === licenseId);
        if (license && license.id !== "other") {
            if (!editData.publicationStmt.availability) {
                editData.publicationStmt.availability = {};
            }
            editData.publicationStmt.availability.license = license.name;
            editData.publicationStmt.availability.licenseUrl = license.url;
        }
    }

    function selectCommonLanguage(index: number, ident: string) {
        const lang = COMMON_LANGUAGES.find((l) => l.ident === ident);
        if (lang) {
            editData.languages[index].ident = lang.ident;
            editData.languages[index].name = lang.name;
        }
    }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if isopen}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
        onclick={handleBackdropClick}
        role="dialog"
        aria-modal="true"
        aria-labelledby="metadata-editor-title"
        tabindex="-1"
    >
        <div
            class="bg-base-100 rounded-lg shadow-xl w-full max-w-4xl mx-4 max-h-[90vh] flex flex-col"
        >
            <!-- Header -->
            <div
                class="flex items-center justify-between p-4 border-b border-base-300"
            >
                <h2 id="metadata-editor-title" class="text-xl font-bold">
                    Manuscript Metadata
                </h2>
                <button
                    class="btn btn-ghost btn-sm btn-circle"
                    onclick={handleClose}
                    aria-label="Close"
                >
                    <CloseButton size="18" strokeWidth="3" />
                </button>
            </div>

            <!-- Content -->
            <div class="p-4 overflow-y-auto flex-1 space-y-2">
                <!-- Title Statement Section -->
                <div class="collapse collapse-arrow bg-base-200">
                    <input
                        type="checkbox"
                        checked={sections.title}
                        onchange={() => toggleSection("title")}
                    />
                    <div class="collapse-title font-medium">
                        Title & Responsibility
                    </div>
                    <div class="collapse-content space-y-4">
                        <!-- Title -->
                        <div class="form-control">
                            <label class="label" for="meta-title">
                                <span class="label-text font-medium"
                                    >Title *</span
                                >
                            </label>
                            <input
                                id="meta-title"
                                type="text"
                                class="input input-bordered"
                                bind:value={editData.titleStmt.title}
                                placeholder="Njáls saga"
                            />
                        </div>

                        <!-- Subtitle -->
                        <div class="form-control">
                            <label class="label" for="meta-subtitle">
                                <span class="label-text">Subtitle</span>
                            </label>
                            <input
                                id="meta-subtitle"
                                type="text"
                                class="input input-bordered"
                                bind:value={editData.titleStmt.subtitle}
                                placeholder="A diplomatic transcription"
                            />
                        </div>

                        <!-- Author -->
                        <div class="form-control">
                            <label class="label" for="meta-author">
                                <span class="label-text">Original Author</span>
                                <span class="label-text-alt"
                                    >If known</span
                                >
                            </label>
                            <input
                                id="meta-author"
                                type="text"
                                class="input input-bordered"
                                bind:value={editData.titleStmt.author}
                                placeholder="Anonymous"
                            />
                        </div>

                        <!-- Editors -->
                        <div class="form-control">
                            <div class="flex items-center justify-between">
                                <span class="label-text font-medium"
                                    >Editors</span
                                >
                                <button
                                    class="btn btn-ghost btn-xs"
                                    onclick={addEditor}
                                >
                                    <Plus size="14" /> Add Editor
                                </button>
                            </div>
                            {#each editData.titleStmt.editors as editor, i}
                                <div
                                    class="flex items-center gap-2 mt-2 p-2 bg-base-300 rounded"
                                >
                                    <input
                                        type="text"
                                        class="input input-bordered input-sm flex-1"
                                        bind:value={editor.name}
                                        placeholder="Editor name"
                                    />
                                    <input
                                        type="text"
                                        class="input input-bordered input-sm w-32"
                                        bind:value={editor.identifierType}
                                        placeholder="ORCID"
                                    />
                                    <input
                                        type="text"
                                        class="input input-bordered input-sm w-40"
                                        bind:value={editor.identifier}
                                        placeholder="0000-0000-0000-0000"
                                    />
                                    <button
                                        class="btn btn-ghost btn-sm btn-circle text-error"
                                        onclick={() => removeEditor(i)}
                                    >
                                        <Trash2 size="14" />
                                    </button>
                                </div>
                            {/each}
                        </div>

                        <!-- Responsibility Statements -->
                        <div class="form-control">
                            <div class="flex items-center justify-between">
                                <span class="label-text font-medium"
                                    >Other Contributors</span
                                >
                                <button
                                    class="btn btn-ghost btn-xs"
                                    onclick={addRespStmt}
                                >
                                    <Plus size="14" /> Add
                                </button>
                            </div>
                            {#each editData.titleStmt.respStmts as resp, i}
                                <div
                                    class="flex items-center gap-2 mt-2 p-2 bg-base-300 rounded"
                                >
                                    <select
                                        class="select select-bordered select-sm w-40"
                                        bind:value={resp.resp}
                                    >
                                        <option value="">Role...</option>
                                        <option value="Transcription"
                                            >Transcription</option
                                        >
                                        <option value="Encoding">Encoding</option>
                                        <option value="Proofreading"
                                            >Proofreading</option
                                        >
                                        <option value="Translation"
                                            >Translation</option
                                        >
                                        <option value="Annotation"
                                            >Annotation</option
                                        >
                                    </select>
                                    <input
                                        type="text"
                                        class="input input-bordered input-sm flex-1"
                                        bind:value={resp.name}
                                        placeholder="Name"
                                    />
                                    <button
                                        class="btn btn-ghost btn-sm btn-circle text-error"
                                        onclick={() => removeRespStmt(i)}
                                    >
                                        <Trash2 size="14" />
                                    </button>
                                </div>
                            {/each}
                        </div>
                    </div>
                </div>

                <!-- Publication Section -->
                <div class="collapse collapse-arrow bg-base-200">
                    <input
                        type="checkbox"
                        checked={sections.publication}
                        onchange={() => toggleSection("publication")}
                    />
                    <div class="collapse-title font-medium">
                        Publication Information
                    </div>
                    <div class="collapse-content space-y-4">
                        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                            <div class="form-control">
                                <label class="label" for="meta-publisher">
                                    <span class="label-text">Publisher</span>
                                </label>
                                <input
                                    id="meta-publisher"
                                    type="text"
                                    class="input input-bordered"
                                    bind:value={editData.publicationStmt
                                        .publisher}
                                    placeholder="University of..."
                                />
                            </div>
                            <div class="form-control">
                                <label class="label" for="meta-pubplace">
                                    <span class="label-text"
                                        >Place of Publication</span
                                    >
                                </label>
                                <input
                                    id="meta-pubplace"
                                    type="text"
                                    class="input input-bordered"
                                    bind:value={editData.publicationStmt
                                        .pubPlace}
                                    placeholder="Reykjavík"
                                />
                            </div>
                        </div>

                        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                            <div class="form-control">
                                <label class="label" for="meta-pubdate">
                                    <span class="label-text"
                                        >Publication Date</span
                                    >
                                </label>
                                <input
                                    id="meta-pubdate"
                                    type="text"
                                    class="input input-bordered"
                                    bind:value={editData.publicationStmt.date}
                                    placeholder="2024"
                                />
                            </div>
                            <div class="form-control">
                                <label class="label" for="meta-idno">
                                    <span class="label-text"
                                        >Digital Identifier</span
                                    >
                                </label>
                                <div class="flex gap-2">
                                    <select
                                        class="select select-bordered w-24"
                                        bind:value={editData.publicationStmt
                                            .idnoType}
                                    >
                                        <option value="">Type</option>
                                        <option value="DOI">DOI</option>
                                        <option value="URN">URN</option>
                                        <option value="URL">URL</option>
                                    </select>
                                    <input
                                        id="meta-idno"
                                        type="text"
                                        class="input input-bordered flex-1"
                                        bind:value={editData.publicationStmt
                                            .idno}
                                        placeholder="10.1234/example"
                                    />
                                </div>
                            </div>
                        </div>

                        <!-- License -->
                        <div class="form-control" role="group" aria-labelledby="license-label">
                            <div class="label">
                                <span id="license-label" class="label-text font-medium"
                                    >License</span
                                >
                            </div>
                            <div class="flex flex-wrap gap-2 mb-2">
                                {#each COMMON_LICENSES as license}
                                    <button
                                        class="btn btn-sm {editData
                                            .publicationStmt.availability
                                            ?.license === license.name
                                            ? 'btn-primary'
                                            : 'btn-outline'}"
                                        onclick={() =>
                                            selectLicense(license.id)}
                                    >
                                        {license.name}
                                    </button>
                                {/each}
                            </div>
                            {#if editData.publicationStmt.availability}
                            <input
                                type="text"
                                class="input input-bordered input-sm"
                                bind:value={editData.publicationStmt.availability
                                    .licenseUrl}
                                placeholder="License URL"
                            />
                            {/if}
                        </div>
                    </div>
                </div>

                <!-- Manuscript Identifier Section -->
                <div class="collapse collapse-arrow bg-base-200">
                    <input
                        type="checkbox"
                        checked={sections.manuscript}
                        onchange={() => toggleSection("manuscript")}
                    />
                    <div class="collapse-title font-medium">
                        Manuscript Identifier
                    </div>
                    <div class="collapse-content space-y-4">
                        <!-- Common name -->
                        <div class="form-control">
                            <label class="label" for="meta-msname">
                                <span class="label-text font-medium"
                                    >Common Name</span
                                >
                                <span class="label-text-alt"
                                    >e.g., "Codex Regius"</span
                                >
                            </label>
                            <input
                                id="meta-msname"
                                type="text"
                                class="input input-bordered"
                                bind:value={editData.msIdentifier.msName}
                                placeholder="Möðruvallabók"
                            />
                        </div>

                        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                            <div class="form-control">
                                <label class="label" for="meta-country">
                                    <span class="label-text">Country</span>
                                </label>
                                <input
                                    id="meta-country"
                                    type="text"
                                    class="input input-bordered"
                                    bind:value={editData.msIdentifier.country}
                                    placeholder="Iceland"
                                />
                            </div>
                            <div class="form-control">
                                <label class="label" for="meta-settlement">
                                    <span class="label-text">City</span>
                                </label>
                                <input
                                    id="meta-settlement"
                                    type="text"
                                    class="input input-bordered"
                                    bind:value={editData.msIdentifier.settlement}
                                    placeholder="Reykjavík"
                                />
                            </div>
                        </div>

                        <div class="form-control">
                            <label class="label" for="meta-repository">
                                <span class="label-text font-medium"
                                    >Repository</span
                                >
                            </label>
                            <input
                                id="meta-repository"
                                type="text"
                                class="input input-bordered"
                                bind:value={editData.msIdentifier.repository}
                                placeholder="Stofnun Árna Magnússonar í íslenskum fræðum"
                            />
                        </div>

                        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                            <div class="form-control">
                                <label class="label" for="meta-collection">
                                    <span class="label-text">Collection</span>
                                </label>
                                <input
                                    id="meta-collection"
                                    type="text"
                                    class="input input-bordered"
                                    bind:value={editData.msIdentifier.collection}
                                    placeholder="GKS"
                                />
                            </div>
                            <div class="form-control">
                                <label class="label" for="meta-shelfmark">
                                    <span class="label-text font-medium"
                                        >Shelfmark</span
                                    >
                                </label>
                                <input
                                    id="meta-shelfmark"
                                    type="text"
                                    class="input input-bordered"
                                    bind:value={editData.msIdentifier.idno}
                                    placeholder="AM 132 fol."
                                />
                            </div>
                        </div>

                        <!-- Alt Identifiers -->
                        <div class="form-control">
                            <div class="flex items-center justify-between">
                                <span class="label-text"
                                    >Alternative Identifiers</span
                                >
                                <button
                                    class="btn btn-ghost btn-xs"
                                    onclick={addAltIdentifier}
                                >
                                    <Plus size="14" /> Add
                                </button>
                            </div>
                            {#each editData.msIdentifier.altIdentifiers as alt, i}
                                <div
                                    class="flex items-center gap-2 mt-2 p-2 bg-base-300 rounded"
                                >
                                    <select
                                        class="select select-bordered select-sm w-32"
                                        bind:value={alt.idType}
                                    >
                                        <option value="">Type...</option>
                                        <option value="former">Former</option>
                                        <option value="catalog">Catalog</option>
                                        <option value="handrit.is"
                                            >handrit.is</option
                                        >
                                    </select>
                                    <input
                                        type="text"
                                        class="input input-bordered input-sm flex-1"
                                        bind:value={alt.idno}
                                        placeholder="Identifier"
                                    />
                                    <button
                                        class="btn btn-ghost btn-sm btn-circle text-error"
                                        onclick={() => removeAltIdentifier(i)}
                                    >
                                        <Trash2 size="14" />
                                    </button>
                                </div>
                            {/each}
                        </div>
                    </div>
                </div>

                <!-- Contents Section -->
                <div class="collapse collapse-arrow bg-base-200">
                    <input
                        type="checkbox"
                        checked={sections.contents}
                        onchange={() => toggleSection("contents")}
                    />
                    <div class="collapse-title font-medium">
                        Manuscript Contents
                    </div>
                    <div class="collapse-content space-y-4">
                        <div class="form-control">
                            <label class="label" for="meta-summary">
                                <span class="label-text font-medium"
                                    >Summary</span
                                >
                            </label>
                            <textarea
                                id="meta-summary"
                                class="textarea textarea-bordered h-24"
                                bind:value={editData.msContents.summary}
                                placeholder="Brief description of the manuscript contents..."
                            ></textarea>
                        </div>

                        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                            <div class="form-control">
                                <label class="label" for="meta-textlang">
                                    <span class="label-text"
                                        >Primary Language</span
                                    >
                                </label>
                                <select
                                    id="meta-textlang"
                                    class="select select-bordered"
                                    bind:value={editData.msContents.textLang}
                                >
                                    <option value="">Select...</option>
                                    {#each COMMON_LANGUAGES as lang}
                                        <option value={lang.ident}
                                            >{lang.name} ({lang.ident})</option
                                        >
                                    {/each}
                                </select>
                            </div>
                            <div class="form-control">
                                <label class="label" for="meta-textlangnote">
                                    <span class="label-text">Language Note</span
                                    >
                                </label>
                                <input
                                    id="meta-textlangnote"
                                    type="text"
                                    class="input input-bordered"
                                    bind:value={editData.msContents.textLangNote}
                                    placeholder="Old Norse with Latin glosses"
                                />
                            </div>
                        </div>
                    </div>
                </div>

                <!-- Physical Description Section -->
                <div class="collapse collapse-arrow bg-base-200">
                    <input
                        type="checkbox"
                        checked={sections.physical}
                        onchange={() => toggleSection("physical")}
                    />
                    <div class="collapse-title font-medium">
                        Physical Description
                    </div>
                    <div class="collapse-content space-y-4">
                        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                            <div class="form-control">
                                <label class="label" for="meta-material">
                                    <span class="label-text">Material</span>
                                </label>
                                <select
                                    id="meta-material"
                                    class="select select-bordered"
                                    bind:value={editData.physDesc.material}
                                >
                                    <option value="">Select...</option>
                                    <option value="parchment">Parchment</option>
                                    <option value="paper">Paper</option>
                                    <option value="mixed"
                                        >Mixed (parchment and paper)</option
                                    >
                                </select>
                            </div>
                            <div class="form-control">
                                <label class="label" for="meta-extent">
                                    <span class="label-text">Extent</span>
                                </label>
                                <input
                                    id="meta-extent"
                                    type="text"
                                    class="input input-bordered"
                                    bind:value={editData.physDesc.extent}
                                    placeholder="ii + 202 + ii leaves"
                                />
                            </div>
                        </div>

                        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                            <div class="form-control">
                                <label class="label" for="meta-foliation">
                                    <span class="label-text">Foliation</span>
                                </label>
                                <input
                                    id="meta-foliation"
                                    type="text"
                                    class="input input-bordered"
                                    bind:value={editData.physDesc.foliation}
                                    placeholder="Modern foliation in pencil"
                                />
                            </div>
                            <div class="form-control">
                                <label class="label" for="meta-layout">
                                    <span class="label-text">Layout</span>
                                </label>
                                <input
                                    id="meta-layout"
                                    type="text"
                                    class="input input-bordered"
                                    bind:value={editData.physDesc.layout}
                                    placeholder="2 columns, 40-45 lines"
                                />
                            </div>
                        </div>

                        <div class="form-control">
                            <label class="label" for="meta-condition">
                                <span class="label-text">Condition</span>
                            </label>
                            <input
                                id="meta-condition"
                                type="text"
                                class="input input-bordered"
                                bind:value={editData.physDesc.condition}
                                placeholder="Good, some water damage on ff. 1-5"
                            />
                        </div>

                        <!-- Hands -->
                        <div class="form-control">
                            <div class="flex items-center justify-between">
                                <span class="label-text font-medium"
                                    >Scribal Hands</span
                                >
                                <button
                                    class="btn btn-ghost btn-xs"
                                    onclick={addHand}
                                >
                                    <Plus size="14" /> Add Hand
                                </button>
                            </div>
                            {#each editData.physDesc.hands as hand, i}
                                <div
                                    class="mt-2 p-3 bg-base-300 rounded space-y-2"
                                >
                                    <div class="flex items-center gap-2">
                                        <input
                                            type="text"
                                            class="input input-bordered input-sm w-20"
                                            bind:value={hand.id}
                                            placeholder="H1"
                                        />
                                        <select
                                            class="select select-bordered select-sm w-32"
                                            bind:value={hand.script}
                                        >
                                            <option value="">Script...</option>
                                            <option value="gothic">Gothic</option
                                            >
                                            <option value="carolingian"
                                                >Carolingian</option
                                            >
                                            <option value="cursiva"
                                                >Cursiva</option
                                            >
                                            <option value="textualis"
                                                >Textualis</option
                                            >
                                        </select>
                                        <select
                                            class="select select-bordered select-sm w-28"
                                            bind:value={hand.scope}
                                        >
                                            <option value="">Scope...</option>
                                            <option value="sole">Sole</option>
                                            <option value="major">Major</option>
                                            <option value="minor">Minor</option>
                                        </select>
                                        <button
                                            class="btn btn-ghost btn-sm btn-circle text-error ml-auto"
                                            onclick={() => removeHand(i)}
                                        >
                                            <Trash2 size="14" />
                                        </button>
                                    </div>
                                    <input
                                        type="text"
                                        class="input input-bordered input-sm w-full"
                                        bind:value={hand.description}
                                        placeholder="Description of hand..."
                                    />
                                </div>
                            {/each}
                        </div>
                    </div>
                </div>

                <!-- History Section -->
                <div class="collapse collapse-arrow bg-base-200">
                    <input
                        type="checkbox"
                        checked={sections.history}
                        onchange={() => toggleSection("history")}
                    />
                    <div class="collapse-title font-medium">History</div>
                    <div class="collapse-content space-y-4">
                        <!-- Origin Date -->
                        <div class="form-control" role="group" aria-labelledby="origin-date-label">
                            <div class="label">
                                <span id="origin-date-label" class="label-text font-medium"
                                    >Date of Origin</span
                                >
                            </div>
                            <div class="grid grid-cols-1 md:grid-cols-3 gap-2">
                                <input
                                    type="text"
                                    class="input input-bordered"
                                    value={editData.history.origDate?.display ?? ""}
                                    oninput={(e) => {
                                        if (!editData.history.origDate) editData.history.origDate = {};
                                        editData.history.origDate.display = e.currentTarget.value || undefined;
                                    }}
                                    placeholder="ca. 1270-1280"
                                />
                                <input
                                    type="text"
                                    class="input input-bordered"
                                    value={editData.history.origDate?.notBefore ?? ""}
                                    oninput={(e) => {
                                        if (!editData.history.origDate) editData.history.origDate = {};
                                        editData.history.origDate.notBefore = e.currentTarget.value || undefined;
                                    }}
                                    placeholder="Not before (1270)"
                                />
                                <input
                                    type="text"
                                    class="input input-bordered"
                                    value={editData.history.origDate?.notAfter ?? ""}
                                    oninput={(e) => {
                                        if (!editData.history.origDate) editData.history.origDate = {};
                                        editData.history.origDate.notAfter = e.currentTarget.value || undefined;
                                    }}
                                    placeholder="Not after (1280)"
                                />
                            </div>
                        </div>

                        <div class="form-control">
                            <label class="label" for="meta-origplace">
                                <span class="label-text">Place of Origin</span>
                            </label>
                            <input
                                id="meta-origplace"
                                type="text"
                                class="input input-bordered"
                                bind:value={editData.history.origPlace}
                                placeholder="Northern Iceland"
                            />
                        </div>

                        <div class="form-control">
                            <label class="label" for="meta-provenance">
                                <span class="label-text">Provenance</span>
                            </label>
                            <textarea
                                id="meta-provenance"
                                class="textarea textarea-bordered h-20"
                                bind:value={editData.history.provenance}
                                placeholder="History of ownership..."
                            ></textarea>
                        </div>

                        <div class="form-control">
                            <label class="label" for="meta-acquisition">
                                <span class="label-text">Acquisition</span>
                            </label>
                            <input
                                id="meta-acquisition"
                                type="text"
                                class="input input-bordered"
                                bind:value={editData.history.acquisition}
                                placeholder="How the repository acquired the manuscript"
                            />
                        </div>
                    </div>
                </div>

                <!-- Languages Section -->
                <div class="collapse collapse-arrow bg-base-200">
                    <input
                        type="checkbox"
                        checked={sections.languages}
                        onchange={() => toggleSection("languages")}
                    />
                    <div class="collapse-title font-medium">
                        Languages Used
                    </div>
                    <div class="collapse-content space-y-4">
                        <div class="flex items-center justify-between">
                            <span class="text-sm text-base-content/70"
                                >List all languages used in the transcription</span
                            >
                            <button
                                class="btn btn-ghost btn-xs"
                                onclick={addLanguage}
                            >
                                <Plus size="14" /> Add Language
                            </button>
                        </div>
                        {#each editData.languages as lang, i}
                            <div
                                class="flex items-center gap-2 p-2 bg-base-300 rounded"
                            >
                                <select
                                    class="select select-bordered select-sm w-48"
                                    value={lang.ident}
                                    onchange={(e) =>
                                        selectCommonLanguage(
                                            i,
                                            e.currentTarget.value,
                                        )}
                                >
                                    <option value="">Select language...</option>
                                    {#each COMMON_LANGUAGES as l}
                                        <option value={l.ident}
                                            >{l.name} ({l.ident})</option
                                        >
                                    {/each}
                                </select>
                                <input
                                    type="number"
                                    class="input input-bordered input-sm w-20"
                                    bind:value={lang.usage}
                                    placeholder="%"
                                    min="0"
                                    max="100"
                                />
                                <span class="text-sm text-base-content/70">%</span
                                >
                                <button
                                    class="btn btn-ghost btn-sm btn-circle text-error ml-auto"
                                    onclick={() => removeLanguage(i)}
                                >
                                    <Trash2 size="14" />
                                </button>
                            </div>
                        {/each}
                    </div>
                </div>

                <!-- Encoding Notes -->
                <div class="form-control mt-4">
                    <label class="label" for="meta-encoding-notes">
                        <span class="label-text font-medium">Encoding Notes</span>
                        <span class="label-text-alt"
                            >Editorial principles, normalization, etc.</span
                        >
                    </label>
                    <textarea
                        id="meta-encoding-notes"
                        class="textarea textarea-bordered h-20"
                        bind:value={editData.encodingNotes}
                        placeholder="Transcription follows Menota handbook v3.0..."
                    ></textarea>
                </div>
            </div>

            <!-- Footer -->
            <div
                class="flex items-center justify-end gap-2 p-4 border-t border-base-300"
            >
                <button class="btn btn-ghost" onclick={handleClose}>
                    Cancel
                </button>
                <button
                    class="btn btn-primary"
                    onclick={handleSave}
                    disabled={isSaving}
                >
                    {#if isSaving}
                        <span class="loading loading-spinner loading-sm"></span>
                    {/if}
                    Save Metadata
                </button>
            </div>
        </div>
    </div>
{/if}
