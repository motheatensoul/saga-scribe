<script lang="ts">
    import { inflectionStore, sessionLemmaStore, lemmaMappings } from '$lib/stores/dictionary';
    import { dictionaryStore } from '$lib/stores/dictionary';
    import {
        lookupLemma,
        searchLemmaPrefix,
        fetchOnpFullEntry,
        addInflection,
        removeInflection,
        lookupInflection,
        type OnpEntry,
        type OnpFullEntry,
        type InflectedForm,
    } from '$lib/tauri';
    import { X as CloseButton } from "@lucide/svelte";

    let {
        facsimile,
        diplomatic,
        wordIndex,
        onclose,
        onsave,
    }: {
        facsimile: string;   // Facsimile-level form (for display/storage)
        diplomatic: string;  // Diplomatic-level form (for lookup)
        wordIndex: number;
        onclose?: () => void;
        onsave?: (wordIndex: number, lemma: string, msa: string) => void;
    } = $props();

    // MENOTA word classes
    const wordClasses = [
        { code: 'xNC', label: 'Noun (common)' },
        { code: 'xNP', label: 'Noun (proper)' },
        { code: 'xAJ', label: 'Adjective' },
        { code: 'xPE', label: 'Pronoun (personal)' },
        { code: 'xPR', label: 'Pronoun (reflexive)' },
        { code: 'xPQ', label: 'Pronoun (interrogative)' },
        { code: 'xPI', label: 'Pronoun (indefinite)' },
        { code: 'xDD', label: 'Determiner (demonstrative)' },
        { code: 'xDQ', label: 'Determiner (quantifier)' },
        { code: 'xDP', label: 'Determiner (possessive)' },
        { code: 'xVB', label: 'Verb' },
        { code: 'xAV', label: 'Adverb' },
        { code: 'xAQ', label: 'Adverb (interrogative)' },
        { code: 'xAP', label: 'Preposition' },
        { code: 'xCC', label: 'Conjunction (coordinating)' },
        { code: 'xCS', label: 'Conjunction (subordinating)' },
        { code: 'xIT', label: 'Interjection' },
        { code: 'xIM', label: 'Infinitive marker' },
        { code: 'xUA', label: 'Unassigned' },
    ];

    // Morphological categories
    const cases = [
        { code: 'cN', label: 'Nominative' },
        { code: 'cG', label: 'Genitive' },
        { code: 'cD', label: 'Dative' },
        { code: 'cA', label: 'Accusative' },
    ];

    const numbers = [
        { code: 'nS', label: 'Singular' },
        { code: 'nD', label: 'Dual' },
        { code: 'nP', label: 'Plural' },
    ];

    const genders = [
        { code: 'gM', label: 'Masculine' },
        { code: 'gF', label: 'Feminine' },
        { code: 'gN', label: 'Neuter' },
    ];

    const species = [
        { code: 'sI', label: 'Indefinite' },
        { code: 'sD', label: 'Definite' },
    ];

    const grades = [
        { code: 'rP', label: 'Positive' },
        { code: 'rC', label: 'Comparative' },
        { code: 'rS', label: 'Superlative' },
    ];

    const persons = [
        { code: 'p1', label: '1st person' },
        { code: 'p2', label: '2nd person' },
        { code: 'p3', label: '3rd person' },
    ];

    const tenses = [
        { code: 'tPS', label: 'Present' },
        { code: 'tPT', label: 'Preterite' },
    ];

    const moods = [
        { code: 'mIN', label: 'Indicative' },
        { code: 'mSU', label: 'Subjunctive' },
        { code: 'mIP', label: 'Imperative' },
    ];

    const voices = [
        { code: 'vA', label: 'Active' },
        { code: 'vR', label: 'Reflexive (middle)' },
    ];

    const finitenessOptions = [
        { code: 'fF', label: 'Finite' },
        { code: 'fP', label: 'Participle' },
        { code: 'fS', label: 'Supine' },
        { code: 'fI', label: 'Infinitive' },
    ];

    // Search state - use diplomatic form for searching
    // svelte-ignore state_referenced_locally 
    let searchQuery = $state(diplomatic);
    let searchResults = $state<OnpEntry[]>([]);
    let isSearching = $state(false);
    let searchTimeout: ReturnType<typeof setTimeout>;

    // Selected entry state
    let selectedEntry = $state<OnpEntry | null>(null);
    let fullEntry = $state<OnpFullEntry | null>(null);
    let isLoadingFull = $state(false);

    // Morphological selections
    let wordClass = $state('');
    let caseVal = $state('');
    let numberVal = $state('');
    let genderVal = $state('');
    let speciesVal = $state('');
    let gradeVal = $state('');
    let personVal = $state('');
    let tenseVal = $state('');
    let moodVal = $state('');
    let voiceVal = $state('');
    let finitenessVal = $state('');

    // Existing mappings for this word
    let existingMappings = $state<InflectedForm[]>([]);

    // Currently selected existing mapping (for highlighting)
    let selectedMappingId = $state<string | null>(null);

    // Normalized form for <me:norm> level
    let normalizedForm = $state('');

    // Derived: which fields to show based on word class
    let showCase = $derived(
        ['xNC', 'xNP', 'xAJ', 'xPE', 'xPR', 'xPQ', 'xPI', 'xDD', 'xDQ', 'xDP'].includes(wordClass) ||
        (wordClass === 'xVB' && finitenessVal === 'fP')
    );
    let showNumber = $derived(
        ['xNC', 'xNP', 'xAJ', 'xPE', 'xPR', 'xPQ', 'xPI', 'xDD', 'xDQ', 'xDP'].includes(wordClass) ||
        (wordClass === 'xVB' && ['fF', 'fP'].includes(finitenessVal))
    );
    let showGender = $derived(
        ['xNC', 'xNP', 'xAJ', 'xPE', 'xPR', 'xPQ', 'xPI', 'xDD', 'xDQ', 'xDP'].includes(wordClass) ||
        (wordClass === 'xVB' && finitenessVal === 'fP')
    );
    let showSpecies = $derived(
        ['xNC', 'xNP', 'xAJ'].includes(wordClass)
    );
    let showGrade = $derived(
        ['xAJ', 'xAV'].includes(wordClass)
    );
    let showPerson = $derived(
        ['xPE', 'xPR'].includes(wordClass) ||
        (wordClass === 'xVB' && finitenessVal === 'fF')
    );
    let showFiniteness = $derived(wordClass === 'xVB');
    let showTense = $derived(
        wordClass === 'xVB' && ['fF', 'fP'].includes(finitenessVal)
    );
    let showMood = $derived(
        wordClass === 'xVB' && finitenessVal === 'fF'
    );
    let showVoice = $derived(
        wordClass === 'xVB' && ['fF', 'fS'].includes(finitenessVal)
    );

    // Build me:msa string from selections
    let msaString = $derived.by(() => {
        if (!wordClass) return '';

        const parts: string[] = [wordClass];

        // Order matters per MENOTA spec
        if (wordClass === 'xVB') {
            if (finitenessVal) parts.push(finitenessVal);
            if (showPerson && personVal) parts.push(personVal);
            if (showNumber && numberVal) parts.push(numberVal);
            if (showTense && tenseVal) parts.push(tenseVal);
            if (showMood && moodVal) parts.push(moodVal);
            if (showVoice && voiceVal) parts.push(voiceVal);
            // Participle-specific
            if (finitenessVal === 'fP') {
                if (caseVal) parts.push(caseVal);
                if (genderVal) parts.push(genderVal);
            }
        } else {
            // Nouns, adjectives, pronouns, determiners
            if (showCase && caseVal) parts.push(caseVal);
            if (showNumber && numberVal) parts.push(numberVal);
            if (showGender && genderVal) parts.push(genderVal);
            if (showGrade && gradeVal) parts.push(gradeVal);
            if (showSpecies && speciesVal) parts.push(speciesVal);
            if (showPerson && personVal) parts.push(personVal);
        }

        return parts.join(' ');
    });

    // Suggest normalized form based on orthographic rules
    function suggestNormalized(diplomatic: string): string {
        let result = diplomatic.toLowerCase();

        // Old Norse orthographic normalizations
        // Double vowels to acute accents
        result = result.replace(/aa/g, 'á');
        result = result.replace(/ee/g, 'é');
        result = result.replace(/ii/g, 'í');
        result = result.replace(/oo/g, 'ó');
        result = result.replace(/uu/g, 'ú');
        result = result.replace(/yy/g, 'ý');

        // w -> v (Old Norse doesn't use w)
        result = result.replace(/w/g, 'v');

        // Common medieval spellings
        result = result.replace(/th/g, 'þ');
        result = result.replace(/dh/g, 'ð');

        // Long s normalization (should already be done in diplomatic, but just in case)
        result = result.replace(/ſ/g, 's');

        // Normalize some common variants
        result = result.replace(/ø/g, 'ǫ');  // Can be either, but ǫ is more standard

        return result;
    }

    // Auto-suggest normalized form based on diplomatic level
    let suggestedNorm = $derived(suggestNormalized(diplomatic));

    // Check if this word instance already has a session confirmation
    let sessionConfirmation = $derived($lemmaMappings.mappings[wordIndex]);

    // Load existing mappings on mount
    $effect(() => {
        loadExistingMappings();
    });

    // If this word instance already has a session confirmation, pre-populate the form
    $effect(() => {
        if (sessionConfirmation && existingMappings.length > 0) {
            // Find the matching existing mapping and apply it
            const matching = existingMappings.find(
                m => m.lemma === sessionConfirmation.lemma && m.analysis === sessionConfirmation.msa
            );
            if (matching) {
                // Mark as selected without triggering onsave again
                selectedMappingId = `${matching.onp_id}-${matching.analysis}`;
            }
        }
    });

    async function loadExistingMappings() {
        try {
            // Look up by diplomatic form for consistent matching
            existingMappings = await lookupInflection(diplomatic);
        } catch (e) {
            console.error('Failed to load existing mappings:', e);
        }
    }

    // Search as user types
    $effect(() => {
        if (searchQuery.length >= 1) {
            clearTimeout(searchTimeout);
            searchTimeout = setTimeout(() => doSearch(), 200);
        } else {
            searchResults = [];
        }
    });

    async function doSearch() {
        if (!searchQuery.trim()) return;

        isSearching = true;
        try {
            let results = await lookupLemma(searchQuery);
            if (results.length === 0) {
                results = await searchLemmaPrefix(searchQuery, 20);
            }
            searchResults = results;
        } catch (e) {
            console.error('Search failed:', e);
            searchResults = [];
        } finally {
            isSearching = false;
        }
    }

    async function selectEntry(entry: OnpEntry) {
        // Clear selected mapping when selecting from search
        selectedMappingId = null;

        selectedEntry = entry;
        fullEntry = null;
        isLoadingFull = true;

        // Try to infer word class from ONP part of speech
        const pos = entry.part_of_speech?.[0] || '';
        if (pos === 'commonNoun') wordClass = 'xNC';
        else if (pos === 'properNoun') wordClass = 'xNP';
        else if (pos === 'verb') wordClass = 'xVB';
        else if (pos === 'adjective') wordClass = 'xAJ';
        else if (pos === 'adverb') wordClass = 'xAV';
        else if (pos === 'adposition') wordClass = 'xAP';
        else if (pos === 'coordinatingConjunction') wordClass = 'xCC';
        else if (pos === 'subordinatingConjunction') wordClass = 'xCS';
        else if (pos === 'interjection') wordClass = 'xIT';
        else wordClass = '';

        // Initialize normalized form with suggestion
        normalizedForm = suggestedNorm;

        try {
            fullEntry = await fetchOnpFullEntry(entry.id);
        } catch (e) {
            console.error('Failed to fetch full entry:', e);
        } finally {
            isLoadingFull = false;
        }
    }

    async function handleSave() {
        console.log('handleSave called', { selectedEntry, msaString, wordClass, normalizedForm, wordIndex });
        if (!selectedEntry || !msaString) {
            console.log('Save blocked: missing selectedEntry or msaString');
            return;
        }

        // Use normalized form if provided, otherwise use suggestion
        const normForm = normalizedForm.trim() || suggestedNorm;

        try {
            console.log('Saving inflection...', { facsimile, diplomatic, wordIndex, lemma: selectedEntry.lemma, msa: msaString, normalized: normForm });

            // Save to persistent inflection store (by diplomatic form for consistent lookup)
            await addInflection(
                diplomatic,  // Key by diplomatic form
                selectedEntry.id,
                selectedEntry.lemma,
                msaString,
                wordClass,
                facsimile,   // Store facsimile
                diplomatic,  // Store diplomatic
                normForm
            );

            inflectionStore.addMapping(diplomatic, {
                onp_id: selectedEntry.id,
                lemma: selectedEntry.lemma,
                analysis: msaString,
                part_of_speech: wordClass,
                facsimile,
                diplomatic,
                normalized: normForm,
            });

            // Save to session store (by word index) for this specific instance
            // Use history-aware method for undo/redo support
            sessionLemmaStore.confirmWithHistory(wordIndex, {
                lemma: selectedEntry.lemma,
                msa: msaString,
                normalized: normForm,
            });

            console.log('Calling onsave callback with wordIndex:', wordIndex);
            onsave?.(wordIndex, selectedEntry.lemma, msaString);
            console.log('Calling onclose callback...');
            onclose?.();
        } catch (e) {
            console.error('Failed to save inflection:', e);
        }
    }

    async function handleRemoveMapping(mapping: InflectedForm) {
        try {
            // Remove by diplomatic form (the lookup key)
            await removeInflection(diplomatic, mapping.onp_id, mapping.analysis);
            inflectionStore.removeMapping(diplomatic, mapping.onp_id, mapping.analysis);

            // Clear selection if this was the selected mapping
            const mappingKey = `${mapping.onp_id}-${mapping.analysis}`;
            if (selectedMappingId === mappingKey) {
                selectedMappingId = null;
            }

            await loadExistingMappings();
        } catch (e) {
            console.error('Failed to remove mapping:', e);
        }
    }

    // Apply an existing mapping to populate the form for editing
    function applyMapping(mapping: InflectedForm) {
        // Track which mapping is selected for highlighting
        selectedMappingId = `${mapping.onp_id}-${mapping.analysis}`;

        // Set selected entry with basic info from the mapping
        selectedEntry = {
            id: mapping.onp_id,
            lemma: mapping.lemma,
            part_of_speech: [mapping.part_of_speech],
            release: '',
            lemma_mod: '',
            language: 'non',
            formats: [],
            citations: 0,
        };

        // Set normalized form
        normalizedForm = mapping.normalized || suggestedNorm;

        // Parse the MSA string and populate all form fields
        const parsed = parseMsaString(mapping.analysis);

        // Apply all parsed values to form fields
        wordClass = parsed.wordClass;
        finitenessVal = parsed.finiteness;
        caseVal = parsed.caseVal;
        numberVal = parsed.numberVal;
        genderVal = parsed.genderVal;
        speciesVal = parsed.speciesVal;
        gradeVal = parsed.gradeVal;
        personVal = parsed.personVal;
        tenseVal = parsed.tenseVal;
        moodVal = parsed.moodVal;
        voiceVal = parsed.voiceVal;
    }

    // Parse an MSA string like "xNC cN nP gF" into individual field values
    // Returns an object with all parsed values instead of setting state directly
    function parseMsaString(msa: string): {
        wordClass: string;
        finiteness: string;
        caseVal: string;
        numberVal: string;
        genderVal: string;
        speciesVal: string;
        gradeVal: string;
        personVal: string;
        tenseVal: string;
        moodVal: string;
        voiceVal: string;
    } {
        const parts = msa.split(' ');
        const result = {
            wordClass: '',
            finiteness: '',
            caseVal: '',
            numberVal: '',
            genderVal: '',
            speciesVal: '',
            gradeVal: '',
            personVal: '',
            tenseVal: '',
            moodVal: '',
            voiceVal: '',
        };

        for (const part of parts) {
            // Word class codes
            if (wordClasses.some(w => w.code === part)) {
                result.wordClass = part;
            }
            // Finiteness (must come early for verb field visibility)
            else if (finitenessOptions.some(f => f.code === part)) {
                result.finiteness = part;
            }
            // Case
            else if (cases.some(c => c.code === part)) {
                result.caseVal = part;
            }
            // Number
            else if (numbers.some(n => n.code === part)) {
                result.numberVal = part;
            }
            // Gender
            else if (genders.some(g => g.code === part)) {
                result.genderVal = part;
            }
            // Species (definiteness)
            else if (species.some(s => s.code === part)) {
                result.speciesVal = part;
            }
            // Grade
            else if (grades.some(g => g.code === part)) {
                result.gradeVal = part;
            }
            // Person
            else if (persons.some(p => p.code === part)) {
                result.personVal = part;
            }
            // Tense
            else if (tenses.some(t => t.code === part)) {
                result.tenseVal = part;
            }
            // Mood
            else if (moods.some(m => m.code === part)) {
                result.moodVal = part;
            }
            // Voice
            else if (voices.some(v => v.code === part)) {
                result.voiceVal = part;
            }
        }

        return result;
    }

    function resetMorphology() {
        caseVal = '';
        numberVal = '';
        genderVal = '';
        speciesVal = '';
        gradeVal = '';
        personVal = '';
        tenseVal = '';
        moodVal = '';
        voiceVal = '';
        finitenessVal = '';
    }

    // Handle user manually changing word class - reset morphology fields
    function handleWordClassChange(e: Event) {
        const select = e.target as HTMLSelectElement;
        wordClass = select.value;
        resetMorphology();
    }
</script>

<div class="flex flex-col gap-4 max-h-[80vh] overflow-y-auto">
    <div class="flex items-center justify-between sticky top-0 bg-base-100 pb-2">
        <div>
            <h3 class="font-bold text-lg">Lemmatize: <span class="text-primary">{facsimile}</span></h3>
            <span class="text-xs opacity-60">Word #{wordIndex}</span>
            {#if facsimile !== diplomatic}
                <span class="text-xs opacity-60 ml-2">(dipl: {diplomatic})</span>
            {/if}
            {#if sessionConfirmation}
                <span class="badge badge-success badge-xs ml-2">confirmed</span>
            {/if}
        </div>
        <button type="button" class="btn btn-sm btn-ghost btn-circle" onclick={onclose} aria-label="Close">
            <CloseButton size={16} />
        </button>
    </div>

    {#if existingMappings.length > 0}
        <div class="bg-base-200 rounded-lg p-3">
            <h4 class="text-sm font-medium mb-2">Existing mappings (click to use)</h4>
            <div class="flex flex-col gap-2">
                {#each existingMappings as mapping}
                    {@const mappingKey = `${mapping.onp_id}-${mapping.analysis}`}
                    {@const isSelected = selectedMappingId === mappingKey}
                    <div
                        class="flex items-center gap-2 rounded p-2 transition-colors"
                        class:bg-primary={isSelected}
                        class:text-primary-content={isSelected}
                        class:bg-base-100={!isSelected}
                    >
                        <button
                            type="button"
                            class="flex-1 text-left rounded p-1 -m-1 transition-colors"
                            class:hover:bg-primary-focus={isSelected}
                            class:hover:bg-base-200={!isSelected}
                            onclick={() => applyMapping(mapping)}
                            title="Click to use this mapping"
                        >
                            <span class="font-medium">{mapping.lemma}</span>
                            <code
                                class="ml-2 text-xs px-1 rounded"
                                class:bg-primary-focus={isSelected}
                                class:bg-base-300={!isSelected}
                            >{mapping.analysis}</code>
                            {#if mapping.normalized}
                                <span class="ml-2 text-xs opacity-70">→ {mapping.normalized}</span>
                            {/if}
                        </button>
                        <button
                            type="button"
                            class="btn btn-ghost btn-xs"
                            class:text-primary-content={isSelected}
                            onclick={() => handleRemoveMapping(mapping)}
                            aria-label="Remove mapping"
                        >
                            ✕
                        </button>
                    </div>
                {/each}
            </div>
        </div>
    {/if}

    <div class="form-control">
        <label class="label" for="lemma-search">
            <span class="label-text">Search ONP dictionary</span>
            {#if !$dictionaryStore.loaded}
                <span class="label-text-alt text-warning">Dictionary not loaded</span>
            {/if}
        </label>
        <input
            id="lemma-search"
            type="text"
            class="input input-bordered"
            placeholder="Enter lemma..."
            bind:value={searchQuery}
            disabled={!$dictionaryStore.loaded}
        />
    </div>

    {#if isSearching}
        <div class="flex items-center justify-center py-4">
            <span class="loading loading-spinner loading-sm"></span>
            <span class="ml-2 text-sm opacity-70">Searching...</span>
        </div>
    {:else if searchResults.length > 0}
        <div class="max-h-40 overflow-y-auto border border-base-300 rounded-lg">
            <ul class="menu menu-sm p-0" role="listbox" aria-label="Search results">
                {#each searchResults as entry}
                    <li>
                        <button
                            type="button"
                            role="option"
                            aria-selected={selectedEntry?.id === entry.id}
                            class="flex justify-between"
                            class:active={selectedEntry?.id === entry.id}
                            onclick={() => selectEntry(entry)}
                        >
                            <span class="font-medium">{entry.lemma}</span>
                            <span class="badge badge-sm badge-ghost">
                                {entry.part_of_speech?.[0] || '?'}
                            </span>
                        </button>
                    </li>
                {/each}
            </ul>
        </div>
    {:else if searchQuery.length >= 1}
        <p class="text-sm opacity-70 text-center py-2">No results found</p>
    {/if}

    {#if selectedEntry}
        <div class="bg-base-200 rounded-lg p-4">
            <div class="flex items-start justify-between mb-3">
                <div>
                    <h4 class="font-bold text-lg">{selectedEntry.lemma}</h4>
                    <span class="badge badge-sm">{selectedEntry.part_of_speech?.[0] || '?'}</span>
                </div>
                <a
                    href={`https://onp.ku.dk/${selectedEntry.id}`}
                    target="_blank"
                    rel="noopener noreferrer"
                    class="btn btn-xs btn-ghost"
                    title="Open in ONP"
                >
                    ↗
                </a>
            </div>

            {#if isLoadingFull}
                <div class="flex items-center py-2">
                    <span class="loading loading-spinner loading-xs"></span>
                    <span class="ml-2 text-sm opacity-70">Loading definition...</span>
                </div>
            {:else if fullEntry && fullEntry.senses.length > 0}
                <div class="text-sm opacity-80 max-h-24 overflow-y-auto mb-3">
                    {#each fullEntry.senses.slice(0, 2) as sense}
                        <p class="mb-1">{sense.definition}</p>
                    {/each}
                </div>
            {/if}

            <!-- Word class selection -->
            <div class="form-control mb-3">
                <label class="label py-1" for="word-class">
                    <span class="label-text font-medium">Word class</span>
                </label>
                <select id="word-class" class="select select-bordered select-sm" value={wordClass} onchange={handleWordClassChange}>
                    <option value="">Select...</option>
                    {#each wordClasses as wc}
                        <option value={wc.code}>{wc.label}</option>
                    {/each}
                </select>
            </div>

            {#if wordClass}
                <div class="grid grid-cols-2 gap-2">
                    {#if showFiniteness}
                        <div class="form-control">
                            <label class="label py-0.5" for="msa-finiteness"><span class="label-text text-xs">Finiteness</span></label>
                            <select id="msa-finiteness" class="select select-bordered select-xs" bind:value={finitenessVal}>
                                <option value="">-</option>
                                {#each finitenessOptions as opt}
                                    <option value={opt.code}>{opt.label}</option>
                                {/each}
                            </select>
                        </div>
                    {/if}

                    {#if showCase}
                        <div class="form-control">
                            <label class="label py-0.5" for="msa-case"><span class="label-text text-xs">Case</span></label>
                            <select id="msa-case" class="select select-bordered select-xs" bind:value={caseVal}>
                                <option value="">-</option>
                                {#each cases as opt}
                                    <option value={opt.code}>{opt.label}</option>
                                {/each}
                            </select>
                        </div>
                    {/if}

                    {#if showNumber}
                        <div class="form-control">
                            <label class="label py-0.5" for="msa-number"><span class="label-text text-xs">Number</span></label>
                            <select id="msa-number" class="select select-bordered select-xs" bind:value={numberVal}>
                                <option value="">-</option>
                                {#each numbers as opt}
                                    <option value={opt.code}>{opt.label}</option>
                                {/each}
                            </select>
                        </div>
                    {/if}

                    {#if showGender}
                        <div class="form-control">
                            <label class="label py-0.5" for="msa-gender"><span class="label-text text-xs">Gender</span></label>
                            <select id="msa-gender" class="select select-bordered select-xs" bind:value={genderVal}>
                                <option value="">-</option>
                                {#each genders as opt}
                                    <option value={opt.code}>{opt.label}</option>
                                {/each}
                            </select>
                        </div>
                    {/if}

                    {#if showSpecies}
                        <div class="form-control">
                            <label class="label py-0.5" for="msa-species"><span class="label-text text-xs">Definiteness</span></label>
                            <select id="msa-species" class="select select-bordered select-xs" bind:value={speciesVal}>
                                <option value="">-</option>
                                {#each species as opt}
                                    <option value={opt.code}>{opt.label}</option>
                                {/each}
                            </select>
                        </div>
                    {/if}

                    {#if showGrade}
                        <div class="form-control">
                            <label class="label py-0.5" for="msa-grade"><span class="label-text text-xs">Grade</span></label>
                            <select id="msa-grade" class="select select-bordered select-xs" bind:value={gradeVal}>
                                <option value="">-</option>
                                {#each grades as opt}
                                    <option value={opt.code}>{opt.label}</option>
                                {/each}
                            </select>
                        </div>
                    {/if}

                    {#if showPerson}
                        <div class="form-control">
                            <label class="label py-0.5" for="msa-person"><span class="label-text text-xs">Person</span></label>
                            <select id="msa-person" class="select select-bordered select-xs" bind:value={personVal}>
                                <option value="">-</option>
                                {#each persons as opt}
                                    <option value={opt.code}>{opt.label}</option>
                                {/each}
                            </select>
                        </div>
                    {/if}

                    {#if showTense}
                        <div class="form-control">
                            <label class="label py-0.5" for="msa-tense"><span class="label-text text-xs">Tense</span></label>
                            <select id="msa-tense" class="select select-bordered select-xs" bind:value={tenseVal}>
                                <option value="">-</option>
                                {#each tenses as opt}
                                    <option value={opt.code}>{opt.label}</option>
                                {/each}
                            </select>
                        </div>
                    {/if}

                    {#if showMood}
                        <div class="form-control">
                            <label class="label py-0.5" for="msa-mood"><span class="label-text text-xs">Mood</span></label>
                            <select id="msa-mood" class="select select-bordered select-xs" bind:value={moodVal}>
                                <option value="">-</option>
                                {#each moods as opt}
                                    <option value={opt.code}>{opt.label}</option>
                                {/each}
                            </select>
                        </div>
                    {/if}

                    {#if showVoice}
                        <div class="form-control">
                            <label class="label py-0.5" for="msa-voice"><span class="label-text text-xs">Voice</span></label>
                            <select id="msa-voice" class="select select-bordered select-xs" bind:value={voiceVal}>
                                <option value="">-</option>
                                {#each voices as opt}
                                    <option value={opt.code}>{opt.label}</option>
                                {/each}
                            </select>
                        </div>
                    {/if}
                </div>

                {#if msaString}
                    <div class="mt-3 p-2 bg-base-300 rounded">
                        <span class="text-xs opacity-70">me:msa:</span>
                        <code class="ml-1 font-mono text-sm">{msaString}</code>
                    </div>

                    <!-- Normalized form input -->
                    <div class="form-control mt-3">
                        <label class="label py-1" for="normalized-form">
                            <span class="label-text font-medium">Normalized form (me:norm)</span>
                        </label>
                        <div class="flex gap-2">
                            <input
                                id="normalized-form"
                                type="text"
                                class="input input-bordered input-sm flex-1 font-mono"
                                bind:value={normalizedForm}
                                placeholder={suggestedNorm}
                            />
                            {#if normalizedForm !== suggestedNorm}
                                <button
                                    type="button"
                                    class="btn btn-ghost btn-sm"
                                    onclick={() => normalizedForm = suggestedNorm}
                                    title="Reset to suggestion"
                                >
                                    ↺
                                </button>
                            {/if}
                        </div>
                        <div class="text-xs opacity-60 mt-1">
                            Diplomatic: {diplomatic} → Suggested: {suggestedNorm}
                        </div>
                    </div>
                {/if}
            {/if}
        </div>
    {/if}

    <div class="modal-action sticky bottom-0 bg-base-100 pt-2">
        <button type="button" class="btn btn-ghost" onclick={onclose}>Cancel</button>
        <button
            type="button"
            class="btn btn-primary"
            onclick={handleSave}
            disabled={!selectedEntry || !msaString}
        >
            Save
        </button>
    </div>
</div>
