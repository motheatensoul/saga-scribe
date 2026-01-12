<script lang="ts">
    import { X as CloseIcon, Plus, Tag, MessageSquare, Eye, Link2, Edit3, Trash2, Copy } from "@lucide/svelte";
    import {
        annotationStore,
        annotationsForWord,
        type Annotation,
        type AnnotationType,
        type AnnotationValue,
    } from "$lib/stores/annotations";
    import { templateStore } from "$lib/stores/template";
    import {
        SEMANTIC_CATEGORIES,
        NOTE_CATEGORIES,
        type PaleographicType,
        type MenotaObservationType,
        type MenotaUnclearReason,
        type MenotaAddPlace,
        type MenotaAddType,
        type MenotaDelRend,
        type MenotaSuppliedReason,
        type MenotaCharType,
        MENOTA_UNCLEAR_REASONS,
        MENOTA_ADD_PLACES,
        MENOTA_DEL_RENDS,
        MENOTA_CHAR_TYPES,
    } from "$lib/types/annotations";

    let {
        wordIndex,
        facsimile,
        diplomatic,
        spanEndIndex = null,
        onclose,
        onsave,
        onFindMatchingWords,
    }: {
        wordIndex: number;
        facsimile: string;
        diplomatic: string;
        spanEndIndex?: number | null;
        onclose?: () => void;
        /** Callback when an annotation is added or removed - triggers recompile */
        onsave?: () => void;
        /** Optional callback to find all word indices matching a diplomatic form */
        onFindMatchingWords?: (diplomatic: string) => number[];
    } = $props();

    // Computed span info
    let isSpan = $derived(spanEndIndex !== null && spanEndIndex !== wordIndex);
    let spanStart = $derived(isSpan ? Math.min(wordIndex, spanEndIndex!) : wordIndex);
    let spanEnd = $derived(isSpan ? Math.max(wordIndex, spanEndIndex!) : wordIndex);

    // Get annotations for this word
    let wordAnnotations = $derived($annotationsForWord(wordIndex));

    // State for adding new annotations
    let showAddForm = $state(false);
    let newAnnotationType = $state<AnnotationType>("note");

    // State for note form
    let noteText = $state("");
    let noteCategory = $state<string>("");

    // State for semantic form
    let semanticCategory = $state("");
    let semanticSubcategory = $state("");
    let semanticLabel = $state("");

    // State for paleographic form
    let paleoType = $state<PaleographicType>("unclear");
    let paleoDescription = $state("");
    let paleoCertainty = $state<number>(0.8);

    // State for MENOTA paleographic form
    let menotaObsType = $state<MenotaObservationType>("unclear");
    let menotaUnclearReason = $state<MenotaUnclearReason | "">("");
    let menotaAddPlace = $state<MenotaAddPlace | "">("");
    let menotaAddType = $state<MenotaAddType | "">("");
    let menotaHand = $state("");
    let menotaDelRend = $state<MenotaDelRend | "">("");
    let menotaSuppliedReason = $state<MenotaSuppliedReason | "">("");
    let menotaResp = $state("");
    let menotaSource = $state("");
    let menotaCharType = $state<MenotaCharType | "">("");
    let menotaCharSize = $state<number | null>(null);

    // Derived: check which schema to use
    let isMenotaSchema = $derived($templateStore.active?.annotationSchemaId !== "tei-p5");

    // State for character-level selection
    let charSelectionStart = $state<number | null>(null);
    let charSelectionEnd = $state<number | null>(null);
    let isSelectingChars = $state(false);

    // State for bulk annotation
    let applyToAllMatching = $state(false);

    // Get matching word indices when bulk mode is available
    let matchingWordIndices = $derived(() => {
        if (!onFindMatchingWords || isSpan) return [];
        return onFindMatchingWords(diplomatic).filter(i => i !== wordIndex);
    });

    // Count of other matching words (excluding current)
    let matchingWordCount = $derived(matchingWordIndices().length);

    // Characters of the word for selection UI
    let wordChars = $derived(facsimile.split(''));

    // Handle character click for selection
    function handleCharClick(index: number) {
        if (!isSelectingChars) return;

        if (charSelectionStart === null) {
            charSelectionStart = index;
            charSelectionEnd = index;
        } else if (charSelectionEnd === null || index < charSelectionStart) {
            charSelectionStart = index;
            charSelectionEnd = index;
        } else {
            charSelectionEnd = index;
        }
    }

    // Check if a character is in the selection range
    function isCharSelected(index: number): boolean {
        if (charSelectionStart === null) return false;
        const end = charSelectionEnd ?? charSelectionStart;
        return index >= Math.min(charSelectionStart, end) &&
               index <= Math.max(charSelectionStart, end);
    }

    // Get selected character range as tuple
    function getCharRange(): [number, number] | null {
        if (charSelectionStart === null) return null;
        const end = charSelectionEnd ?? charSelectionStart;
        return [Math.min(charSelectionStart, end), Math.max(charSelectionStart, end)];
    }

    // Clear character selection
    function clearCharSelection() {
        charSelectionStart = null;
        charSelectionEnd = null;
        isSelectingChars = false;
    }

    // Get annotation type icon
    function getTypeIcon(type: AnnotationType) {
        switch (type) {
            case "lemma": return Tag;
            case "semantic": return Tag;
            case "note": return MessageSquare;
            case "paleographic": return Eye;
            case "reference": return Link2;
            default: return Edit3;
        }
    }

    // Get annotation type label
    function getTypeLabel(type: AnnotationType): string {
        switch (type) {
            case "lemma": return "Lemma";
            case "semantic": return "Semantic";
            case "note": return "Note";
            case "paleographic": return "Paleographic";
            case "syntax": return "Syntax";
            case "reference": return "Reference";
            case "custom": return "Custom";
        }
    }

    // Get target description for annotation
    function getTargetDescription(ann: Annotation): string | null {
        if (ann.target.type === "char") {
            const { charStart, charEnd } = ann.target;
            const chars = facsimile.slice(charStart, charEnd + 1);
            return `[${chars}]`;
        }
        if (ann.target.type === "span") {
            return `[#${ann.target.startWord}-#${ann.target.endWord}]`;
        }
        return null;
    }

    // Get annotation display text
    function getAnnotationSummary(ann: Annotation): string {
        const targetDesc = getTargetDescription(ann);
        const prefix = targetDesc ? `${targetDesc} ` : "";

        switch (ann.value.kind) {
            case "lemma":
                return `${ann.value.lemma} (${ann.value.msa})`;
            case "semantic":
                return `${ann.value.category}${ann.value.subcategory ? ` / ${ann.value.subcategory}` : ""}`;
            case "note":
                return ann.value.text.slice(0, 50) + (ann.value.text.length > 50 ? "..." : "");
            case "paleographic":
                return `${prefix}${ann.value.observationType}${ann.value.description ? `: ${ann.value.description.slice(0, 30)}` : ""}`;
            case "menota-paleographic":
                return `${prefix}MENOTA ${ann.value.observationType}${ann.value.description ? `: ${ann.value.description.slice(0, 30)}` : ""}`;
            case "syntax":
                return ann.value.function;
            case "reference":
                return ann.value.label || ann.value.target;
            case "custom":
                return ann.value.customType;
            default:
                return "Unknown";
        }
    }

    // Helper to get appropriate target based on selection
    function getTarget() {
        if (isSpan) {
            return { type: "span" as const, startWord: spanStart, endWord: spanEnd };
        }
        return { type: "word" as const, wordIndex };
    }

    // Get all word indices to apply annotation to (current + matching if bulk mode)
    function getTargetIndices(): number[] {
        const indices = [wordIndex];
        if (applyToAllMatching && !isSpan) {
            indices.push(...matchingWordIndices());
        }
        return indices;
    }

    // Add annotation handlers
    function handleAddNote() {
        if (!noteText.trim()) return;

        const targetIndices = getTargetIndices();
        for (const idx of targetIndices) {
            const annotation = {
                id: `note-${Date.now().toString(16)}-${idx}`,
                type: "note" as const,
                target: isSpan && idx === wordIndex
                    ? { type: "span" as const, startWord: spanStart, endWord: spanEnd }
                    : { type: "word" as const, wordIndex: idx },
                value: {
                    kind: "note" as const,
                    text: noteText,
                    category: noteCategory || undefined,
                },
            };
            annotationStore.add(annotation);
        }
        resetForm();
        onsave?.();
    }

    function handleAddSemantic() {
        if (!semanticCategory) return;

        const targetIndices = getTargetIndices();
        for (const idx of targetIndices) {
            const annotation = {
                id: `sem-${Date.now().toString(16)}-${idx}`,
                type: "semantic" as const,
                target: isSpan && idx === wordIndex
                    ? { type: "span" as const, startWord: spanStart, endWord: spanEnd }
                    : { type: "word" as const, wordIndex: idx },
                value: {
                    kind: "semantic" as const,
                    category: semanticCategory,
                    subcategory: semanticSubcategory || undefined,
                    label: semanticLabel || undefined,
                },
            };
            annotationStore.add(annotation);
        }
        resetForm();
        onsave?.();
    }

    function handleAddPaleographic() {
        const charRange = getCharRange();

        const targetIndices = getTargetIndices();
        for (const idx of targetIndices) {
            // Character-level targeting only applies to the current word, not bulk targets
            const target = charRange && idx === wordIndex
                ? { type: "char" as const, wordIndex: idx, charStart: charRange[0], charEnd: charRange[1] }
                : isSpan && idx === wordIndex
                    ? { type: "span" as const, startWord: spanStart, endWord: spanEnd }
                    : { type: "word" as const, wordIndex: idx };

            let value: AnnotationValue;

            if (isMenotaSchema) {
                value = {
                    kind: "menota-paleographic",
                    observationType: menotaObsType,
                    unclearReason: menotaUnclearReason || undefined,
                    addPlace: menotaAddPlace || undefined,
                    addType: menotaAddType || undefined,
                    hand: menotaHand || undefined,
                    delRend: menotaDelRend || undefined,
                    suppliedReason: menotaSuppliedReason || undefined,
                    resp: menotaResp || undefined,
                    source: menotaSource || undefined,
                    charType: menotaCharType || undefined,
                    charSize: menotaCharSize || undefined,
                    description: paleoDescription || undefined,
                    certainty: paleoCertainty,
                };
            } else {
                value = {
                    kind: "paleographic",
                    observationType: paleoType,
                    description: paleoDescription || undefined,
                    certainty: paleoCertainty,
                };
            }

            const annotation: Annotation = {
                id: `paleo-${Date.now().toString(16)}-${idx}`,
                type: "paleographic",
                target,
                value,
            };
            annotationStore.add(annotation);
        }
        resetForm();
        onsave?.();
    }

    function handleRemoveAnnotation(id: string) {
        annotationStore.remove(id);
        onsave?.();
    }

    function resetForm() {
        showAddForm = false;
        noteText = "";
        noteCategory = "";
        semanticCategory = "";
        semanticSubcategory = "";
        semanticLabel = "";
        paleoDescription = "";
        paleoCertainty = 0.8;
        
        // MENOTA resets
        menotaObsType = "unclear";
        menotaUnclearReason = "";
        menotaAddPlace = "";
        menotaAddType = "";
        menotaHand = "";
        menotaDelRend = "";
        menotaSuppliedReason = "";
        menotaResp = "";
        menotaSource = "";
        menotaCharType = "";
        menotaCharSize = null;

        clearCharSelection();
    }

    // Get subcategories for selected semantic category
    let availableSubcategories = $derived(() => {
        const cat = SEMANTIC_CATEGORIES.find(c => c.id === semanticCategory);
        return cat?.subcategories || [];
    });

    // Paleographic types
    const PALEO_TYPES: { value: PaleographicType; label: string }[] = [
        { value: "unclear", label: "Unclear reading" },
        { value: "damage", label: "Physical damage" },
        { value: "erasure", label: "Erasure" },
        { value: "letterform", label: "Unusual letterform" },
        { value: "abbreviation", label: "Abbreviation" },
        { value: "correction", label: "Scribal correction" },
        { value: "addition", label: "Addition" },
        { value: "decoration", label: "Decoration" },
        { value: "other", label: "Other" },
    ];
</script>

<div class="bg-base-100 text-base-content text-sm h-full flex flex-col max-w-md">
    <!-- Header -->
    <div class="flex justify-between items-center p-3 border-b border-base-300">
        <div>
            <h2 class="font-bold">Annotations</h2>
            <div class="text-xs opacity-70">
                {#if isSpan}
                    <span class="badge badge-sm badge-secondary mr-1">Span</span>
                    <span>Words #{spanStart} - #{spanEnd}</span>
                {:else}
                    <span class="font-mono">{facsimile}</span>
                    <span class="ml-1">(#{wordIndex})</span>
                {/if}
            </div>
            {#if isSpan}
                <p class="text-xs opacity-50 mt-1">Shift-click another word to adjust span</p>
            {/if}
        </div>
        <button
            type="button"
            class="btn btn-ghost btn-sm btn-circle"
            onclick={onclose}
            aria-label="Close"
        >
            <CloseIcon size={16} />
        </button>
    </div>

    <!-- Existing annotations -->
    <div class="flex-1 overflow-y-auto p-3">
        {#if wordAnnotations.length === 0}
            <p class="text-center opacity-60 py-4">No annotations for this word</p>
        {:else}
            <div class="flex flex-col gap-2">
                {#each wordAnnotations as ann}
                    {@const Icon = getTypeIcon(ann.type)}
                    <div class="card card-compact bg-base-200">
                        <div class="card-body p-2">
                            <div class="flex items-start gap-2">
                                <div class="badge badge-sm gap-1" class:badge-primary={ann.type === "lemma"} class:badge-secondary={ann.type === "semantic"} class:badge-accent={ann.type === "note"}>
                                    <Icon size={12} />
                                    {getTypeLabel(ann.type)}
                                </div>
                                <button
                                    type="button"
                                    class="btn btn-ghost btn-xs ml-auto"
                                    onclick={() => handleRemoveAnnotation(ann.id)}
                                    disabled={ann.type === "lemma"}
                                    title={ann.type === "lemma" ? "Remove via Lemmatizer" : "Remove annotation"}
                                >
                                    <Trash2 size={12} />
                                </button>
                            </div>
                            <p class="text-sm">{getAnnotationSummary(ann)}</p>
                            {#if ann.metadata?.note}
                                <p class="text-xs opacity-60">{ann.metadata.note}</p>
                            {/if}
                        </div>
                    </div>
                {/each}
            </div>
        {/if}
    </div>

    <!-- Add annotation section -->
    <div class="border-t border-base-300 p-3">
        {#if !showAddForm}
            <button
                type="button"
                class="btn btn-sm btn-outline w-full gap-2"
                onclick={() => showAddForm = true}
            >
                <Plus size={14} />
                Add Annotation
            </button>
        {:else}
            <div class="flex flex-col gap-3">
                <!-- Type selector -->
                <div class="form-control">
                    <label class="label py-1" for="ann-type">
                        <span class="label-text text-xs">Annotation Type</span>
                    </label>
                    <select
                        id="ann-type"
                        class="select select-bordered select-sm"
                        bind:value={newAnnotationType}
                    >
                        <option value="note">Note</option>
                        <option value="semantic">Semantic Category</option>
                        <option value="paleographic">Paleographic</option>
                    </select>
                </div>

                <!-- Bulk annotation option -->
                {#if onFindMatchingWords && !isSpan && matchingWordCount > 0}
                    <div class="form-control">
                        <label class="label cursor-pointer justify-start gap-2 py-1">
                            <input
                                type="checkbox"
                                class="checkbox checkbox-sm checkbox-primary"
                                bind:checked={applyToAllMatching}
                            />
                            <span class="label-text text-xs">
                                <Copy size={12} class="inline mr-1" />
                                Apply to all "{diplomatic}" ({matchingWordCount + 1} total)
                            </span>
                        </label>
                    </div>
                {/if}

                <!-- Note form -->
                {#if newAnnotationType === "note"}
                    <div class="form-control">
                        <label class="label py-1" for="note-cat">
                            <span class="label-text text-xs">Category (optional)</span>
                        </label>
                        <select id="note-cat" class="select select-bordered select-sm" bind:value={noteCategory}>
                            <option value="">None</option>
                            {#each NOTE_CATEGORIES as cat}
                                <option value={cat}>{cat}</option>
                            {/each}
                        </select>
                    </div>
                    <div class="form-control">
                        <label class="label py-1" for="note-text">
                            <span class="label-text text-xs">Note text</span>
                        </label>
                        <textarea
                            id="note-text"
                            class="textarea textarea-bordered textarea-sm h-20"
                            placeholder="Enter your note..."
                            bind:value={noteText}
                        ></textarea>
                    </div>
                    <button
                        type="button"
                        class="btn btn-sm btn-primary"
                        onclick={handleAddNote}
                        disabled={!noteText.trim()}
                    >
                        Add Note
                    </button>
                {/if}

                <!-- Semantic form -->
                {#if newAnnotationType === "semantic"}
                    <div class="form-control">
                        <label class="label py-1" for="sem-cat">
                            <span class="label-text text-xs">Category</span>
                        </label>
                        <select id="sem-cat" class="select select-bordered select-sm" bind:value={semanticCategory}>
                            <option value="">Select...</option>
                            {#each SEMANTIC_CATEGORIES as cat}
                                <option value={cat.id}>{cat.label}</option>
                            {/each}
                        </select>
                    </div>
                    {#if availableSubcategories().length > 0}
                        <div class="form-control">
                            <label class="label py-1" for="sem-subcat">
                                <span class="label-text text-xs">Subcategory</span>
                            </label>
                            <select id="sem-subcat" class="select select-bordered select-sm" bind:value={semanticSubcategory}>
                                <option value="">None</option>
                                {#each availableSubcategories() as sub}
                                    <option value={sub}>{sub}</option>
                                {/each}
                            </select>
                        </div>
                    {/if}
                    <div class="form-control">
                        <label class="label py-1" for="sem-label">
                            <span class="label-text text-xs">Label (e.g., name)</span>
                        </label>
                        <input
                            id="sem-label"
                            type="text"
                            class="input input-bordered input-sm"
                            placeholder="e.g., Gunnarr"
                            bind:value={semanticLabel}
                        />
                    </div>
                    <button
                        type="button"
                        class="btn btn-sm btn-primary"
                        onclick={handleAddSemantic}
                        disabled={!semanticCategory}
                    >
                        Add Semantic
                    </button>
                {/if}

                <!-- Paleographic form -->
                {#if newAnnotationType === "paleographic"}
                    <!-- Character selection for targeting specific characters -->
                    <div class="form-control">
                        <div class="flex items-center justify-between py-1">
                            <span class="label-text text-xs">Target Characters</span>
                            <label class="label cursor-pointer gap-2 py-0">
                                <span class="label-text text-xs">Select chars</span>
                                <input
                                    type="checkbox"
                                    class="toggle toggle-xs"
                                    bind:checked={isSelectingChars}
                                    onchange={() => { if (!isSelectingChars) clearCharSelection(); }}
                                />
                            </label>
                        </div>
                        <div class="flex flex-wrap gap-0.5 p-2 bg-base-200 rounded font-serif text-lg min-h-[2.5rem]">
                            {#each wordChars as char, i}
                                <button
                                    type="button"
                                    class="char-select px-1 rounded transition-colors"
                                    class:bg-primary={isCharSelected(i)}
                                    class:text-primary-content={isCharSelected(i)}
                                    class:hover:bg-base-300={isSelectingChars && !isCharSelected(i)}
                                    class:cursor-pointer={isSelectingChars}
                                    class:cursor-default={!isSelectingChars}
                                    disabled={!isSelectingChars}
                                    onclick={() => handleCharClick(i)}
                                >
                                    {char}
                                </button>
                            {/each}
                        </div>
                        {#if getCharRange()}
                            <p class="text-xs opacity-70 mt-1">
                                Selected: "{facsimile.slice(getCharRange()![0], getCharRange()![1] + 1)}" (chars {getCharRange()![0]}-{getCharRange()![1]})
                            </p>
                        {:else if isSelectingChars}
                            <p class="text-xs opacity-50 mt-1">Click characters to select range</p>
                        {:else}
                            <p class="text-xs opacity-50 mt-1">Applies to whole word</p>
                        {/if}
                    </div>
                    {#if isMenotaSchema}
                        <!-- MENOTA Form -->
                        <div class="form-control">
                            <label class="label py-1" for="menota-type">
                                <span class="label-text text-xs">Observation Type</span>
                            </label>
                            <select id="menota-type" class="select select-bordered select-sm" bind:value={menotaObsType}>
                                <option value="unclear">Unclear Reading</option>
                                <option value="addition">Addition</option>
                                <option value="deletion">Deletion</option>
                                <option value="supplied">Supplied Text</option>
                                <option value="character">Character Feature</option>
                            </select>
                        </div>

                        {#if menotaObsType === "unclear"}
                            <div class="form-control">
                                <label class="label py-1" for="menota-unclear">
                                    <span class="label-text text-xs">Reason</span>
                                </label>
                                <select id="menota-unclear" class="select select-bordered select-sm" bind:value={menotaUnclearReason}>
                                    <option value="">Select reason...</option>
                                    {#each MENOTA_UNCLEAR_REASONS as r}
                                        <option value={r.id}>{r.label}</option>
                                    {/each}
                                </select>
                            </div>
                        {:else if menotaObsType === "addition"}
                            <div class="form-control">
                                <label class="label py-1" for="menota-add-place">
                                    <span class="label-text text-xs">Placement</span>
                                </label>
                                <select id="menota-add-place" class="select select-bordered select-sm" bind:value={menotaAddPlace}>
                                    <option value="">Select placement...</option>
                                    {#each MENOTA_ADD_PLACES as p}
                                        <option value={p.id}>{p.label}</option>
                                    {/each}
                                </select>
                            </div>
                            <div class="form-control">
                                <label class="label py-1" for="menota-add-type">
                                    <span class="label-text text-xs">Type</span>
                                </label>
                                <select id="menota-add-type" class="select select-bordered select-sm" bind:value={menotaAddType}>
                                    <option value="">Select type...</option>
                                    <option value="supplement">Supplement</option>
                                    <option value="gloss">Gloss</option>
                                    <option value="correction">Correction</option>
                                </select>
                            </div>
                            <div class="form-control">
                                <label class="label py-1" for="menota-hand">
                                    <span class="label-text text-xs">Hand ID</span>
                                </label>
                                <input id="menota-hand" type="text" class="input input-bordered input-sm" bind:value={menotaHand} placeholder="e.g. hand2" />
                            </div>
                        {:else if menotaObsType === "deletion"}
                            <div class="form-control">
                                <label class="label py-1" for="menota-del-rend">
                                    <span class="label-text text-xs">Rendering</span>
                                </label>
                                <select id="menota-del-rend" class="select select-bordered select-sm" bind:value={menotaDelRend}>
                                    <option value="">Select rendering...</option>
                                    {#each MENOTA_DEL_RENDS as r}
                                        <option value={r.id}>{r.label}</option>
                                    {/each}
                                </select>
                            </div>
                            <div class="form-control">
                                <label class="label py-1" for="menota-del-hand">
                                    <span class="label-text text-xs">Hand ID</span>
                                </label>
                                <input id="menota-del-hand" type="text" class="input input-bordered input-sm" bind:value={menotaHand} placeholder="e.g. hand2" />
                            </div>
                        {:else if menotaObsType === "supplied"}
                            <div class="form-control">
                                <label class="label py-1" for="menota-supplied-reason">
                                    <span class="label-text text-xs">Reason</span>
                                </label>
                                <select id="menota-supplied-reason" class="select select-bordered select-sm" bind:value={menotaSuppliedReason}>
                                    <option value="">Select reason...</option>
                                    <option value="omitted">Omitted</option>
                                    <option value="damage">Damage</option>
                                    <option value="illegible">Illegible</option>
                                    <option value="restoration">Restoration</option>
                                    <option value="emendation">Emendation</option>
                                </select>
                            </div>
                            <div class="form-control">
                                <label class="label py-1" for="menota-resp">
                                    <span class="label-text text-xs">Responsibility</span>
                                </label>
                                <input id="menota-resp" type="text" class="input input-bordered input-sm" bind:value={menotaResp} placeholder="Editor ID" />
                            </div>
                        {:else if menotaObsType === "character"}
                            <div class="form-control">
                                <label class="label py-1" for="menota-char-type">
                                    <span class="label-text text-xs">Character Type</span>
                                </label>
                                <select id="menota-char-type" class="select select-bordered select-sm" bind:value={menotaCharType}>
                                    <option value="">Select type...</option>
                                    {#each MENOTA_CHAR_TYPES as t}
                                        <option value={t.id}>{t.label}</option>
                                    {/each}
                                </select>
                            </div>
                        {/if}
                    {:else}
                        <!-- Generic Form -->
                        <div class="form-control">
                            <label class="label py-1" for="paleo-type">
                                <span class="label-text text-xs">Observation Type</span>
                            </label>
                            <select id="paleo-type" class="select select-bordered select-sm" bind:value={paleoType}>
                                {#each PALEO_TYPES as pt}
                                    <option value={pt.value}>{pt.label}</option>
                                {/each}
                            </select>
                        </div>
                    {/if}
                    <div class="form-control">
                        <label class="label py-1" for="paleo-desc">
                            <span class="label-text text-xs">Description</span>
                        </label>
                        <textarea
                            id="paleo-desc"
                            class="textarea textarea-bordered textarea-sm h-16"
                            placeholder="Describe the observation..."
                            bind:value={paleoDescription}
                        ></textarea>
                    </div>
                    <div class="form-control">
                        <label class="label py-1" for="paleo-cert">
                            <span class="label-text text-xs">Certainty: {Math.round(paleoCertainty * 100)}%</span>
                        </label>
                        <input
                            id="paleo-cert"
                            type="range"
                            min="0"
                            max="1"
                            step="0.1"
                            class="range range-xs"
                            bind:value={paleoCertainty}
                        />
                    </div>
                    <button
                        type="button"
                        class="btn btn-sm btn-primary"
                        onclick={handleAddPaleographic}
                    >
                        Add Paleographic
                    </button>
                {/if}

                <button
                    type="button"
                    class="btn btn-sm btn-ghost"
                    onclick={resetForm}
                >
                    Cancel
                </button>
            </div>
        {/if}
    </div>
</div>
