<script lang="ts">
    import { onMount, tick } from "svelte";
    import { Splitpanes, Pane } from "svelte-splitpanes";
    import Editor from "$lib/components/Editor.svelte";
    import Preview from "$lib/components/Preview.svelte";
    import Toolbar from "$lib/components/Toolbar.svelte";
    import TemplateEditor from "$lib/components/TemplateEditor.svelte";
    import MetadataEditor from "$lib/components/MetadataEditor.svelte";
    import EntityBrowser from "$lib/components/EntityBrowser.svelte";
    import Lemmatizer from "$lib/components/Lemmatizer.svelte";
    import AnnotationPanel from "$lib/components/AnnotationPanel.svelte";
    import ErrorPanel from "$lib/components/ErrorPanel.svelte";
    import ValidationPanel from "$lib/components/ValidationPanel.svelte";
    import SettingsDialog from "$lib/components/SettingsDialog.svelte";
    import HelpDialog from "$lib/components/HelpDialog.svelte";
    import StatisticsPanel from "$lib/components/StatisticsPanel.svelte";
    import { editor } from "$lib/stores/editor.svelte";
    import { templateStore } from "$lib/stores/template.svelte";
    import { entityStore } from "$lib/stores/entities.svelte";
    import { settings } from "$lib/stores/settings.svelte";
    import { errorStore } from "$lib/stores/errors.svelte";
    import { validationStore } from "$lib/stores/validation.svelte";
    import { metadataStore } from "$lib/stores/metadata.svelte";
    import type { Metadata } from "$lib/types/metadata";
    import { isMetadataEmpty } from "$lib/types/metadata";
    import {
        listTemplates,
        compileDsl,
        loadEntities,
        loadTextFile,
        loadCustomMappings,
        loadCustomEntities,
        loadOnpHeadwords,
        loadInflections,
        generateTeiHeader,
    } from "$lib/tauri";
    import {
        dictionaryStore,
        inflectionStore,
    } from "$lib/stores/dictionary.svelte";
    import {
        annotationStore,
        annotationHistory,
    } from "$lib/stores/annotations.svelte";
    import { createFileOperations } from "$lib/composables/useFileOperations.svelte";
    import { createShortcuts } from "$lib/composables/useShortcuts.svelte";

    import { resolveResource } from "@tauri-apps/api/path";


    //Icon imports
    import {
        BookDashed,
        ScrollText,
        MessageCircleWarning,
        FileCheck,
        FileText,
        Undo,
        Redo,
        Search,
        BarChart2,
    } from "@lucide/svelte";

    let editorComponent: Editor | null = $state<Editor | null>(null);
    let previewContent = $state("");
    let showTemplateManager = $state(false);
    let showEntityBrowser = $state(false);
    let showErrorPanel = $state(false);
    let showValidationPanel = $state(false);
    let showStatisticsPanel = $state(false);
    let showLemmatizer = $state(false);
    let wordPanelTab = $state<"lemmatize" | "annotate">("lemmatize");
    let showSettings = $state(false);
    let showHelp = $state(false);
    let selectedWordFacsimile = $state<string | null>(null);
    let selectedWordDiplomatic = $state<string | null>(null);
    let selectedWordIndex = $state<number>(-1);
    let selectedWordElement = $state<HTMLElement | null>(null);
    // For span selections (shift-click extends)
    let spanEndWordIndex = $state<number | null>(null);
    let compileTimeout: ReturnType<typeof setTimeout>;
    let entitiesJson = $derived(
        Object.keys(entityStore.entities).length > 0
            ? JSON.stringify({
                  version: "1.0",
                  name: "SagaScribe",
                  entities: entityStore.entities,
              })
            : null,
    );
    let normalizerJson = $state<string | null>(null);
    let entityMappingsJson = $state<string | null>(null);
    let isImporting = $state(false);
    let isMounting: boolean = $state(true);
    let showMetadataEditor = $state(false);
    let currentMetadata = $state<Metadata | undefined>(undefined);

    const fileOps = createFileOperations({
        doCompile,
        getPreviewContent: () => previewContent,
        setIsImporting: (val) => (isImporting = val),
        getEditorComponent: () => editorComponent,
    });

    function closeAllModals() {
        showLemmatizer = false;
        showEntityBrowser = false;
        showErrorPanel = false;
        showValidationPanel = false;
        showTemplateManager = false;
        showMetadataEditor = false;
        showStatisticsPanel = false;
    }

    const shortcuts = createShortcuts({
        fileOps: {
            handleSaveProject: fileOps.handleSaveProject,
            handleOpenProject: fileOps.handleOpenProject,
        },
        undoRedoOps: {
            handleLemmaUndo: () => handleLemmaUndo(),
            handleLemmaRedo: () => handleLemmaRedo(),
        },
        uiState: {
            closeAllModals,
            toggleHelp: () => (showHelp = !showHelp),
        },
    });

    onMount(async () => {
        errorStore.info("App", "Application starting...");

        // Load settings first
        try {
            errorStore.info("Settings", "Loading settings...");
            await settings.load();
            errorStore.info("Settings", "Settings loaded");
        } catch (e) {
            errorStore.error("Settings", "Failed to load settings", String(e));
        }

        // Load templates
        try {
            errorStore.info("Templates", "Loading templates...");
            const templates = await listTemplates();
            templateStore.setTemplates(templates);

            // Try to restore the previously active template
            await settings.load();
            const savedTemplateId = settings.activeTemplateId;
            let activeTemplate = savedTemplateId
                ? templates.find((t) => t.id === savedTemplateId)
                : null;

            // Fall back to first template if saved one not found
            if (!activeTemplate && templates.length > 0) {
                activeTemplate = templates[0];
            }

            if (activeTemplate) {
                templateStore.setActive(activeTemplate);
            }

            errorStore.info(
                "Templates",
                `Loaded ${templates.length} templates`,
            );
        } catch (e) {
            errorStore.error(
                "Templates",
                "Failed to load templates",
                String(e),
            );
        }

        // Load default MENOTA entities
        const resourcePath = await resolveResource("entities/menota.json");
        const devPath = resourcePath.replace(
            /src-tauri\/target\/[^/]+\/entities\/menota\.json$/,
            "static/entities/menota.json",
        );

        const entityPaths = [resourcePath, devPath];
        let entities = null;
        let loadedFrom = "";

        for (const path of entityPaths) {
            try {
                errorStore.info("Entities", `Trying to load from: ${path}`);
                entities = await loadEntities(path);
                loadedFrom = path;
                break;
            } catch (e) {
                errorStore.warning(
                    "Entities",
                    `Failed to load from ${path}`,
                    String(e),
                );
            }
        }

        if (entities) {
            const entityCount = Object.keys(entities).length;
            errorStore.info(
                "Entities",
                `Loaded ${entityCount} entities from ${loadedFrom}`,
            );
            entityStore.setBuiltinEntities(entities);
            
            try {
                const customEntities = await loadCustomEntities();
                const customCount = Object.keys(customEntities).length;
                if (customCount > 0) {
                    errorStore.info(
                        "Entities",
                        `Loaded ${customCount} custom entities`,
                    );
                    entityStore.setCustomEntities(customEntities);
                }
            } catch (e) {
                errorStore.warning(
                    "Entities",
                    "Failed to load custom entities",
                    String(e),
                );
            }

            const baseMappingsResourcePath = await resolveResource(
                "normalizer/entity-base-letters.json",
            );
            const baseMappingsDevPath = baseMappingsResourcePath.replace(
                /src-tauri\/target\/[^/]+\/normalizer\/entity-base-letters\.json$/,
                "static/normalizer/entity-base-letters.json",
            );
            const baseMappingsPaths = [
                baseMappingsResourcePath,
                baseMappingsDevPath,
            ];

            for (const path of baseMappingsPaths) {
                try {
                    errorStore.info(
                        "Entities",
                        `Trying to load base mappings from: ${path}`,
                    );
                    const baseMappingsText = await loadTextFile(path);
                    entityMappingsJson = baseMappingsText;
                    const baseMappingsData = JSON.parse(baseMappingsText);
                    const mappings = baseMappingsData.mappings || {};
                    entityStore.setBaseMappings(mappings);
                    break;
                } catch (e) {
                    errorStore.warning(
                        "Entities",
                        `Failed to load base mappings from ${path}`,
                        String(e),
                    );
                }
            }

            try {
                const customMappings = await loadCustomMappings();
                const customCount = Object.keys(customMappings).length;
                if (customCount > 0) {
                    errorStore.info(
                        "Entities",
                        `Loaded ${customCount} custom entity mappings`,
                    );
                    entityStore.setCustomMappings(customMappings);
                }
            } catch (e) {
                errorStore.warning(
                    "Entities",
                    "Failed to load custom mappings",
                    String(e),
                );
            }
        } else {
            errorStore.error(
                "Entities",
                "Failed to load entities from any path",
            );
            entityStore.setError("Could not find entity definitions file");
        }

        const normalizerResourcePath = await resolveResource(
            "normalizer/menota-levels.json",
        );
        const normalizerDevPath = normalizerResourcePath.replace(
            /src-tauri\/target\/[^/]+\/normalizer\/menota-levels\.json$/,
            "static/normalizer/menota-levels.json",
        );
        const normalizerPaths = [normalizerResourcePath, normalizerDevPath];

        for (const path of normalizerPaths) {
            try {
                errorStore.info("Normalizer", `Trying to load from: ${path}`);
                normalizerJson = await loadTextFile(path);
                errorStore.info(
                    "Normalizer",
                    `Loaded normalizer dictionary from ${path}`,
                );
                break;
            } catch (e) {
                errorStore.warning(
                    "Normalizer",
                    `Failed to load from ${path}`,
                    String(e),
                );
            }
        }

        const onpResourcePath = await resolveResource(
            "dictionary/onp-headwords.json",
        );
        const onpDevPath = onpResourcePath.replace(
            /src-tauri\/target\/[^/]+\/dictionary\/onp-headwords\.json$/,
            "static/dictionary/onp-headwords.json",
        );
        const onpPaths = [onpResourcePath, onpDevPath];

        dictionaryStore.setLoading();
        for (const path of onpPaths) {
            try {
                errorStore.info(
                    "Dictionary",
                    `Trying to load ONP headwords from: ${path}`,
                );
                const count = await loadOnpHeadwords(path);
                dictionaryStore.setLoaded(count);
                break;
            } catch (e) {
                errorStore.warning(
                    "Dictionary",
                    `Failed to load from ${path}`,
                    String(e),
                );
            }
        }

        try {
            const store = await loadInflections();
            const count = Object.keys(store.forms).length;
            if (count > 0) {
                inflectionStore.setMappings(store.forms);
            }
        } catch (e) {
            errorStore.warning(
                "Dictionary",
                "Failed to load inflection mappings",
                String(e),
            );
        }

        isMounting = false;
        errorStore.info("App", "Application ready");
    });

    async function compileOnly(content: string): Promise<string | null> {
        const template = templateStore.active;
        if (!template) return null;

        const compileLemmaMappings = annotationStore.lemmaMappings;
        const annotationSet = annotationStore.set;
        const hasAnnotations = annotationSet?.annotations.length > 0;

        const meta = metadataStore.metadata;
        let header = template.header;
        if (meta && !isMetadataEmpty(meta)) {
            try {
                header = await generateTeiHeader(
                    JSON.stringify(meta),
                    template.multiLevel,
                );
            } catch (e) {
                console.warn("Failed to generate TEI header from metadata, using template header:", e);
            }
        }

        const options = {
            wordWrap: template.wordWrap,
            autoLineNumbers: template.autoLineNumbers,
            multiLevel: template.multiLevel,
            wrapPages: template.wrapPages,
            entitiesJson: entitiesJson ?? undefined,
            normalizerJson: normalizerJson ?? undefined,
            entityMappingsJson: entityMappingsJson ?? undefined,
            customMappings: entityStore.customMappings,
            lemmaMappingsJson:
                Object.keys(compileLemmaMappings).length > 0
                    ? JSON.stringify(compileLemmaMappings)
                    : undefined,
            annotationsJson: hasAnnotations
                ? JSON.stringify(annotationSet)
                : undefined,
        };

        return await compileDsl(
            content,
            header,
            template.footer,
            options,
        );
    }

    // Auto-save effect
    $effect(() => {
        if (!settings.autoSave) return;

        const interval = setInterval(() => {
            // Only auto-save if we have a file path and there are unsaved changes
            if (editor.filePath && editor.isDirty) {
                fileOps.handleSaveProject(true);
            }
        }, 30000); // Check every 30 seconds

        return () => clearInterval(interval);
    });

    async function doCompile(content: string) {
        try {
            const result = await compileOnly(content);
            if (result !== null) {
                // Debug: Check what we're setting
                console.log("[doCompile] Result length:", result.length);
                console.log("[doCompile] First 100 chars:", result.substring(0, 100));
                console.log("[doCompile] Starts with '<':", result.trimStart().startsWith('<'));
                previewContent = result;
            } else {
                console.log("[doCompile] compileOnly returned null (no template active?)");
            }
        } catch (e) {
            console.error("[doCompile] Compilation error:", e);
            // Don't set plain text as previewContent - it breaks XML parsing
            // Use errorStore to display the error instead
            errorStore.error("Compiler", `Compilation failed: ${e}`);
        }
    }

    async function updatePreview(content: string) {
        if (!settings.autoPreview) return;
        if (compileTimeout) clearTimeout(compileTimeout);

        compileTimeout = setTimeout(
            () => {
                doCompile(editor.content);
            },
            settings.previewDelay,
        );
    }

    function handleEditorChange(content: string) {
        updatePreview(content);
    }

    function handleEntityInsert(text: string) {
        editorComponent?.insertText(text);
        showEntityBrowser = false;
    }

    function handleWordClick(
        facsimile: string,
        diplomatic: string,
        wordIndex: number,
        element: HTMLElement,
        isSpanExtend?: boolean,
    ) {
        if (isSpanExtend && showLemmatizer && selectedWordIndex >= 0) {
            spanEndWordIndex = wordIndex;
        } else {
            selectedWordFacsimile = facsimile;
            selectedWordDiplomatic = diplomatic;
            selectedWordIndex = wordIndex;
            selectedWordElement = element;
            spanEndWordIndex = null;
            showLemmatizer = true;
        }
    }

    function handleLemmatizerClose() {
        showLemmatizer = false;
        selectedWordFacsimile = null;
        selectedWordDiplomatic = null;
        selectedWordIndex = -1;
        selectedWordElement = null;
        spanEndWordIndex = null;
    }

    async function handleLemmatizerSave() {
        await tick();
        if (compileTimeout) clearTimeout(compileTimeout);
        doCompile(editor.content);
    }

    async function handleAnnotationSave() {
        await tick();
        if (compileTimeout) clearTimeout(compileTimeout);
        doCompile(editor.content);
    }

    function findMatchingWords(targetDiplomatic: string): number[] {

        if (!previewContent) return [];
        const indices: number[] = [];
        const parser = new DOMParser();
        try {
            const doc = parser.parseFromString(previewContent, "text/xml");
            if (doc.querySelector("parsererror")) return [];
            const words = doc.querySelectorAll("w");
            let wordIndex = 0;
            for (const word of words) {
                const diplEl = word.querySelector("dipl");
                let diplomatic = diplEl?.textContent?.trim() || word.textContent?.trim() || "";
                if (diplomatic === targetDiplomatic) {
                    indices.push(wordIndex);
                }
                wordIndex++;
            }
        } catch {
            return [];
        }
        return indices;
    }

    function handleLemmaUndo() {
        const action = annotationHistory.undo();
        if (!action) return;
        if (action.type === "add") {
            annotationStore.remove(action.annotation.id, false);
        } else if (action.type === "remove") {
            annotationStore.add(action.annotation, false);
        } else if (action.type === "update") {
            if (action.previousAnnotation) {
                annotationStore.add(action.previousAnnotation, false);
            } else {
                annotationStore.remove(action.annotation.id, false);
            }
        }
        if (compileTimeout) clearTimeout(compileTimeout);
        doCompile(editor.content);
    }

    function handleLemmaRedo() {
        const action = annotationHistory.redo();
        if (!action) return;
        if (action.type === "add") {
            annotationStore.add(action.annotation, false);
        } else if (action.type === "remove") {
            annotationStore.remove(action.annotation.id, false);
        } else if (action.type === "update") {
            annotationStore.add(action.annotation, false);
        }
        if (compileTimeout) clearTimeout(compileTimeout);
        doCompile(editor.content);
    }

</script>

<svelte:window onkeydown={shortcuts.handleKeydown} />

<div class="flex flex-col h-screen overflow-hidden bg-base-100">
    <Toolbar
        onopen={fileOps.handleOpenProject}
        onimport={fileOps.handleImport}
        onsave={fileOps.handleSaveProject}
        onexportxml={fileOps.handleExportXml}
        onexportdict={fileOps.handleExportDictionary}
        onexporthtml={fileOps.handleExportHtml}
        onexportpdf={fileOps.handleExportPdf}
        onundo={handleLemmaUndo}
        onredo={handleLemmaRedo}
        onsettings={() => (showSettings = true)}
        onhelp={() => (showHelp = true)}
    />

    <div class="flex-1 overflow-hidden">
        <Splitpanes theme="themed">
            <Pane minSize={20}>
                <div class="flex flex-col h-full">
                    <div class="flex justify-between items-center px-4 py-2 bg-base-200 border-b border-base-300 font-medium text-sm">
                        <span class="text-md xl:text-lg font-bold px-2">DSL Editor</span>
                        <div class="flex gap-1">
                            <button class="btn btn-ghost btn-xs xl:btn-sm" title="Undo (Ctrl+Z)" onclick={() => editorComponent?.triggerUndo()}><Undo class="size-3/4" /></button>
                            <button class="btn btn-ghost btn-xs xl:btn-sm" title="Redo (Ctrl+Y)" onclick={() => editorComponent?.triggerRedo()}><Redo class="size-3/4" /></button>
                            <button class="btn btn-ghost btn-xs xl:btn-sm" title="Search and Replace (Ctrl+Shift+F)" onclick={() => editorComponent?.triggerSearch()}><Search class="size-3/4" /></button>
                            <div class="divider divider-horizontal mx-0 h-4 self-center"></div>
                            <button class="btn btn-ghost btn-xs xl:btn-sm text-xs xl:text-sm" title="Insert entity" onclick={() => (showEntityBrowser = true)}>ꝥ</button>
                            <button class="btn btn-ghost btn-xs xl:btn-sm" title="Manage templates" onclick={() => (showTemplateManager = true)}><BookDashed class="size-3/4" /></button>
                            <button class="btn btn-ghost btn-xs xl:btn-sm" title="Edit manuscript metadata" onclick={() => (showMetadataEditor = true)}><FileText class="size-3/4" /></button>
                            <button class="btn btn-ghost btn-xs xl:btn-sm" title="Project Statistics" onclick={() => (showStatisticsPanel = true)}><BarChart2 class="size-3/4" /></button>
                            <button class="btn btn-ghost btn-xs xl:btn-sm" title="Validate XML" onclick={() => (showValidationPanel = true)}><FileCheck class="size-3/4" /></button>
                            <button class="btn btn-ghost btn-xs xl:btn-sm" class:text-error={errorStore.counts.error > 0} title="View logs" onclick={() => (showErrorPanel = true)}>
                                {#if errorStore.counts.error > 0}
                                    <MessageCircleWarning size="14" color="var(--color-error)" />
                                {:else}
                                    <ScrollText class="size-3/4" color="var(--color-success)" />
                                {/if}
                            </button>
                        </div>
                    </div>
                    <Editor bind:this={editorComponent} onchange={handleEditorChange} />
                </div>
            </Pane>
            <Pane minSize={20}>
                <Preview content={previewContent} onwordclick={handleWordClick} />
            </Pane>
        </Splitpanes>
    </div>

    <TemplateEditor bind:isopen={showTemplateManager} />

    <MetadataEditor
        bind:isopen={showMetadataEditor}
        metadata={metadataStore.metadata}
        onSave={(meta) => {
            metadataStore.setMetadata(meta);
        }}
    />

    {#if showEntityBrowser}
        <div class="modal modal-open">
            <div class="modal-backdrop" role="none" onclick={() => (showEntityBrowser = false)}></div>
            <div class="modal-box max-w-4xl">
                <EntityBrowser oninsert={handleEntityInsert} onclose={() => (showEntityBrowser = false)} />
            </div>
        </div>
    {/if}

    {#if showErrorPanel}
        <div class="modal modal-open">
            <div class="modal-backdrop" role="none" onclick={() => (showErrorPanel = false)}></div>
            <div class="modal-box max-w-3xl">
                <ErrorPanel onclose={() => (showErrorPanel = false)} />
            </div>
        </div>
    {/if}

    {#if showValidationPanel}
        <div class="modal modal-open">
            <div class="modal-backdrop" role="none" onclick={() => (showValidationPanel = false)}></div>
            <div class="modal-box max-w-3xl">
                <ValidationPanel xmlContent={previewContent} onclose={() => (showValidationPanel = false)} />
            </div>
        </div>
    {/if}

    {#if showStatisticsPanel}
        <div class="modal modal-open">
            <div class="modal-backdrop" role="none" onclick={() => (showStatisticsPanel = false)}></div>
            <div class="modal-box max-w-2xl p-0 overflow-hidden">
                <StatisticsPanel previewContent={previewContent} onclose={() => (showStatisticsPanel = false)} />
            </div>
        </div>
    {/if}

    {#if showLemmatizer && selectedWordDiplomatic && selectedWordIndex >= 0}
        <div class="modal modal-open">
            <div class="modal-backdrop" role="none" onclick={handleLemmatizerClose}></div>
            <div class="modal-box max-w-2xl p-0">
                <div class="tabs tabs-boxed bg-base-200 rounded-none">
                    <button type="button" class="tab" class:tab-active={wordPanelTab === "lemmatize"} onclick={() => wordPanelTab = "lemmatize"}>Lemmatize</button>
                    <button type="button" class="tab" class:tab-active={wordPanelTab === "annotate"} onclick={() => wordPanelTab = "annotate"}>Annotate</button>
                </div>
                <div class="p-4">
                    <div class:hidden={wordPanelTab !== "lemmatize"}>
                        <Lemmatizer facsimile={selectedWordFacsimile || ""} diplomatic={selectedWordDiplomatic} wordIndex={selectedWordIndex} onclose={handleLemmatizerClose} onsave={handleLemmatizerSave} />
                    </div>
                    <div class:hidden={wordPanelTab !== "annotate"}>
                        <AnnotationPanel facsimile={selectedWordFacsimile || ""} diplomatic={selectedWordDiplomatic} wordIndex={selectedWordIndex} spanEndIndex={spanEndWordIndex} onclose={handleLemmatizerClose} onsave={handleAnnotationSave} onFindMatchingWords={findMatchingWords} />
                    </div>
                </div>
            </div>
        </div>
    {/if}

    {#if isImporting}
        <div class="modal modal-open">
            <div class="modal-backdrop bg-base-100/50"></div>
            <div class="modal-box bg-transparent shadow-none shadow-transparent border-none flex flex-col items-center justify-center overflow-hidden">
                <svg class="spinner-svg" viewBox="0 0 50 50" width="48" height="48">
                    <circle cx="25" cy="25" r="20" fill="none" stroke="currentColor" stroke-width="4" stroke-linecap="round" stroke-dasharray="90, 150" stroke-dashoffset="0" class="text-primary">
                        <animateTransform attributeName="transform" type="rotate" from="0 25 25" to="360 25 25" dur="1s" repeatCount="indefinite" />
                    </circle>
                </svg>
                <p class="mt-4 font-bold text-lg text-base-content">Importing…</p>
            </div>
        </div>
    {/if}

    <SettingsDialog bind:isopen={showSettings} />
    <HelpDialog bind:isopen={showHelp} />
</div>

<style>
    :global(.splitpanes.themed) { height: 100%; }
    :global(.splitpanes.themed .splitpanes__pane) { background-color: transparent; }
    :global(.splitpanes.themed .splitpanes__splitter) { background-color: var(--color-base-300); position: relative; flex-shrink: 0; }
    :global(.splitpanes.themed .splitpanes__splitter:hover) { background-color: var(--color-primary); }
    :global(.splitpanes.themed.splitpanes--vertical > .splitpanes__splitter) { width: 6px; cursor: col-resize; }
    :global(.splitpanes.themed.splitpanes--horizontal > .splitpanes__splitter) { height: 6px; cursor: row-resize; }
</style>
