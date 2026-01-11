<script lang="ts">
    import { onMount } from "svelte";
    import { Splitpanes, Pane } from "svelte-splitpanes";
    import { open, save } from "@tauri-apps/plugin-dialog";
    import Editor from "$lib/components/Editor.svelte";
    import Preview from "$lib/components/Preview.svelte";
    import Toolbar from "$lib/components/Toolbar.svelte";
    import TemplateEditor from "$lib/components/TemplateEditor.svelte";
    import EntityBrowser from "$lib/components/EntityBrowser.svelte";
    import Lemmatizer from "$lib/components/Lemmatizer.svelte";
    import ErrorPanel from "$lib/components/ErrorPanel.svelte";
    import ValidationPanel from "$lib/components/ValidationPanel.svelte";
    import SettingsDialog from "$lib/components/SettingsDialog.svelte";
    import HelpDialog from "$lib/components/HelpDialog.svelte";
    import { editor } from "$lib/stores/editor";
    import { templateStore } from "$lib/stores/template";
    import { entityStore } from "$lib/stores/entities";
    import { settings } from "$lib/stores/settings";
    import { errorStore, errorCounts } from "$lib/stores/errors";
    import {
        listTemplates,
        compileDsl,
        loadEntities,
        loadTextFile,
        loadCustomMappings,
        loadOnpHeadwords,
        loadInflections,
        saveProject,
        openProject,
        exportTei,
        openFile,
        importFile,
        exportInflections,
    } from "$lib/tauri";
    import {
        dictionaryStore,
        inflectionStore,
        sessionLemmaStore,
    } from "$lib/stores/dictionary";
    import {
        lemmatizationHistory,
        canUndo,
        canRedo,
    } from "$lib/stores/lemmatizationHistory";
    import { resolveResource, appDataDir } from "@tauri-apps/api/path";
    
    //Icon imports
    import { BookDashed, ScrollText, MessageCircleWarning, FileCheck, Undo, Redo, Search  } from '@lucide/svelte';
    

    let editorComponent: Editor;
    let previewContent = $state("");
    let showTemplateManager = $state(false);
    let showEntityBrowser = $state(false);
    let showErrorPanel = $state(false);
    let showValidationPanel = $state(false);
    let showLemmatizer = $state(false);
    let showSettings = $state(false);
    let showHelp = $state(false);
    let selectedWordFacsimile = $state<string | null>(null);
    let selectedWordDiplomatic = $state<string | null>(null);
    let selectedWordIndex = $state<number>(-1);
    let selectedWordElement = $state<HTMLElement | null>(null);
    let compileTimeout: ReturnType<typeof setTimeout>;
    let entitiesJson = $state<string | null>(null);
    let normalizerJson = $state<string | null>(null);
    let entityMappingsJson = $state<string | null>(null);
    let isImporting = $state(false);

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
            const savedTemplateId = $settings.activeTemplateId;
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
        // Try resource path (production), then derive static folder from resource path (development)
        const resourcePath = await resolveResource("entities/menota.json");

        // For development, the resource path is like: .../src-tauri/target/debug/entities/menota.json
        // We need: .../static/entities/menota.json
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
            entityStore.setEntities(entities);
            entitiesJson = JSON.stringify({
                version: "1.0",
                name: "MENOTA",
                entities,
            });

            // Load base entity mappings (diplomatic normalization defaults)
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
                    entityMappingsJson = baseMappingsText; // Store raw JSON for compiler
                    const baseMappingsData = JSON.parse(baseMappingsText);
                    const mappings = baseMappingsData.mappings || {};
                    const mappingsCount = Object.keys(mappings).length;
                    errorStore.info(
                        "Entities",
                        `Loaded ${mappingsCount} base entity mappings from ${path}`,
                    );
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

            // Load custom entity mappings (user overrides)
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

        // Load normalizer dictionary for multi-level transcription
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

        if (!normalizerJson) {
            errorStore.warning(
                "Normalizer",
                "Could not load normalizer dictionary - multi-level output may not work correctly",
            );
        }

        // Load ONP dictionary for lemmatization
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
                errorStore.info(
                    "Dictionary",
                    `Loaded ${count} ONP headwords from ${path}`,
                );
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

        // Load user inflection mappings
        try {
            const store = await loadInflections();
            const count = Object.keys(store.forms).length;
            if (count > 0) {
                errorStore.info(
                    "Dictionary",
                    `Loaded ${count} user inflection mappings`,
                );
                inflectionStore.setMappings(store.forms);
            }
        } catch (e) {
            errorStore.warning(
                "Dictionary",
                "Failed to load inflection mappings",
                String(e),
            );
        }

        errorStore.info("App", "Application ready");
    });

    async function doCompile(content: string) {
        const template = $templateStore.active;
        if (!template) return;

        try {
            // Use session store for lemma mappings - keyed by word INDEX
            // Only confirmed word instances get lemma attributes
            const lemmaMappings: Record<
                number,
                { lemma: string; msa: string; normalized?: string }
            > = {};
            for (const [indexStr, mapping] of Object.entries(
                $sessionLemmaStore.mappings,
            )) {
                const index = parseInt(indexStr, 10);
                lemmaMappings[index] = {
                    lemma: mapping.lemma,
                    msa: mapping.msa,
                    normalized: mapping.normalized,
                };
            }

            console.log(
                "Compiling with session lemma mappings (by index):",
                lemmaMappings,
            );

            previewContent = await compileDsl(
                content,
                template.header,
                template.footer,
                {
                    wordWrap: template.wordWrap,
                    autoLineNumbers: template.autoLineNumbers,
                    multiLevel: template.multiLevel,
                    wrapPages: template.wrapPages,
                    entitiesJson: entitiesJson ?? undefined,
                    normalizerJson: normalizerJson ?? undefined,
                    entityMappingsJson: entityMappingsJson ?? undefined,
                    customMappings: $entityStore.customMappings,
                    lemmaMappingsJson:
                        Object.keys(lemmaMappings).length > 0
                            ? JSON.stringify(lemmaMappings)
                            : undefined,
                },
            );
        } catch (e) {
            previewContent = `Error: ${e}`;
        }
    }

    async function updatePreview(content: string) {
        if (!$settings.autoPreview) return;

        clearTimeout(compileTimeout);
        compileTimeout = setTimeout(
            () => doCompile(content),
            $settings.previewDelay,
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
    ) {
        selectedWordFacsimile = facsimile;
        selectedWordDiplomatic = diplomatic;
        selectedWordIndex = wordIndex;
        selectedWordElement = element;
        showLemmatizer = true;
    }

    function handleLemmatizerClose() {
        showLemmatizer = false;
        selectedWordFacsimile = null;
        selectedWordDiplomatic = null;
        selectedWordIndex = -1;
        selectedWordElement = null;
    }

    function handleLemmatizerSave(
        wordIndex: number,
        lemma?: string,
        msa?: string,
    ) {
        console.log("handleLemmatizerSave called", { wordIndex, lemma, msa });
        console.log(
            "Current session lemma mappings:",
            $sessionLemmaStore.mappings,
        );
        // Trigger immediate recompile to update TEI output with new lemma
        // Use doCompile directly to bypass autoPreview check and delay
        doCompile($editor.content);
    }

    // Undo/redo for lemmatization
    function handleLemmaUndo() {
        const action = lemmatizationHistory.undo();
        if (!action) return;

        // Revert the action (without pushing to history again)
        if (action.type === "confirm") {
            // Undo a confirm: restore previous state
            if (action.previousMapping) {
                sessionLemmaStore.confirm(
                    action.wordIndex,
                    action.previousMapping,
                );
            } else {
                sessionLemmaStore.unconfirm(action.wordIndex);
            }
        } else {
            // Undo an unconfirm: restore the mapping
            if (action.previousMapping) {
                sessionLemmaStore.confirm(
                    action.wordIndex,
                    action.previousMapping,
                );
            }
        }

        // Recompile to reflect changes
        doCompile($editor.content);
        errorStore.info(
            "Undo",
            `Undid lemmatization for word #${action.wordIndex}`,
        );
    }

    function handleLemmaRedo() {
        const action = lemmatizationHistory.redo();
        if (!action) return;

        // Reapply the action (without pushing to history again)
        if (action.type === "confirm") {
            // Redo a confirm: apply the mapping
            if (action.mapping) {
                sessionLemmaStore.confirm(action.wordIndex, action.mapping);
            }
        } else {
            // Redo an unconfirm: remove the mapping
            sessionLemmaStore.unconfirm(action.wordIndex);
        }

        // Recompile to reflect changes
        doCompile($editor.content);
        errorStore.info(
            "Redo",
            `Redid lemmatization for word #${action.wordIndex}`,
        );
    }

    // Project file handling
    async function handleOpenProject() {
        const path = await open({
            filters: [
                { name: "TEI Scribe Project", extensions: ["teis"] },
                { name: "DSL Source", extensions: ["dsl", "txt"] },
            ],
        });
        if (!path) return;

        const pathStr = path as string;

        try {
            if (pathStr.endsWith(".teis")) {
                // Open project archive
                const project = await openProject(pathStr);

                // Restore DSL source to editor
                editor.setFile(pathStr, project.source);
                editorComponent?.setContent(project.source);

                // Clear lemmatization history and session confirmations
                lemmatizationHistory.clear();
                sessionLemmaStore.clear();
                for (const [indexStr, mapping] of Object.entries(
                    project.confirmations,
                )) {
                    const index = parseInt(indexStr, 10);
                    sessionLemmaStore.confirm(index, mapping);
                }

                // Restore template if possible
                const templates = $templateStore.templates;
                const template = templates.find(
                    (t) => t.id === project.manifest.template_id,
                );
                if (template) {
                    templateStore.setActive(template);
                }

                // Trigger recompile
                await doCompile(project.source);

                errorStore.info("Project", `Opened project from ${pathStr}`);
            } else {
                // Open plain DSL file (backwards compatibility)
                const file = await openFile(pathStr);
                editor.setFile(file.path, file.content);
                editorComponent?.setContent(file.content);

                // Clear history and session confirmations for new file
                lemmatizationHistory.clear();
                sessionLemmaStore.clear();

                await doCompile(file.content);

                errorStore.info("File", `Opened DSL file from ${pathStr}`);
            }
        } catch (e) {
            errorStore.error("Open", `Failed to open: ${e}`);
        }
    }

    async function handleSaveProject() {
        const template = $templateStore.active;
        if (!template) {
            errorStore.warning(
                "Save",
                "Please select a template before saving",
            );
            return;
        }

        // Get save path (use existing or prompt for new)
        let path = $editor.filePath;
        if (!path || !path.endsWith(".teis")) {
            path = await save({
                filters: [{ name: "TEI Scribe Project", extensions: ["teis"] }],
                defaultPath: path
                    ? path.replace(/\.[^.]+$/, ".teis")
                    : undefined,
            });
        }
        if (!path) return;

        try {
            // Ensure we have fresh compiled output
            await doCompile($editor.content);

            // Serialize session confirmations
            const confirmationsJson = JSON.stringify(
                $sessionLemmaStore.mappings,
            );

            // Save project archive
            await saveProject(
                path,
                $editor.content,
                previewContent,
                confirmationsJson,
                template.id,
            );

            editor.setFile(path, $editor.content);
            errorStore.info("Project", `Saved project to ${path}`);
        } catch (e) {
            errorStore.error("Save", `Failed to save project: ${e}`);
        }
    }

    async function handleExportXml() {
        const template = $templateStore.active;
        if (!template) {
            errorStore.warning(
                "Export",
                "Please select a template before exporting",
            );
            return;
        }

        const path = await save({
            filters: [{ name: "TEI-XML", extensions: ["xml"] }],
            defaultPath: $editor.filePath
                ? $editor.filePath.replace(/\.[^.]+$/, ".xml")
                : undefined,
        });
        if (!path) return;

        try {
            // Ensure we have fresh compiled output
            await doCompile($editor.content);

            await exportTei(path, previewContent);
            errorStore.info("Export", `Exported TEI-XML to ${path}`);
        } catch (e) {
            errorStore.error("Export", `Failed to export: ${e}`);
        }
    }

    async function handleExportDictionary() {
        const path = await save({
            filters: [{ name: "JSON", extensions: ["json"] }],
            defaultPath: "inflections.json",
        });
        if (!path) return;

        try {
            const count = await exportInflections(path);
            errorStore.info(
                "Export",
                `Exported ${count} inflection entries to ${path}`,
            );
        } catch (e) {
            errorStore.error("Export", `Failed to export dictionary: ${e}`);
        }
    }

    async function handleImport() {
        const path = await open({
            filters: [
                { name: "All Supported Formats", extensions: ["xml", "tei", "txt"] },
                { name: "TEI/XML", extensions: ["xml", "tei"] },
                { name: "Text File", extensions: ["txt"] },
            ],
        });
        if (!path) return;

        isImporting = true;
        
        // Give the UI a moment to render the spinner
        await new Promise(resolve => setTimeout(resolve, 50));

        try {
            const pathStr = path as string;
            const content = await importFile(pathStr);

            // Set content and clear file path to treat as new unsaved project
            // (avoids overwriting the original source file with DSL)
            editorComponent?.setContent(content);
            editor.setFile(null, content);

            // Clear history and session confirmations
            lemmatizationHistory.clear();
            sessionLemmaStore.clear();

            await doCompile(content);
            errorStore.info("Import", `Imported content from ${pathStr}`);
        } catch (e) {
            errorStore.error("Import", `Failed to import: ${e}`);
        } finally {
            isImporting = false;
        }
    }

    // Keyboard shortcuts
    function handleKeydown(event: KeyboardEvent) {
        // F1: Open help
        if (event.key === "F1") {
            event.preventDefault();
            showHelp = true;
            return;
        }

        if (event.ctrlKey || event.metaKey) {
            if (event.key === "s") {
                event.preventDefault();
                handleSaveProject();
            } else if (event.key === "o") {
                event.preventDefault();
                handleOpenProject();
            } else if (event.key === "?" || event.key === "/") {
                // Ctrl+? or Ctrl+/: Open help
                event.preventDefault();
                showHelp = true;
            } else if (
                event.shiftKey &&
                (event.key === "z" || event.key === "Z")
            ) {
                // Ctrl+Shift+Z: Undo lemmatization
                event.preventDefault();
                handleLemmaUndo();
            } else if (
                event.shiftKey &&
                (event.key === "y" || event.key === "Y")
            ) {
                // Ctrl+Shift+Y: Redo lemmatization
                event.preventDefault();
                handleLemmaRedo();
            }
        }
    }
</script>

<svelte:window on:keydown={handleKeydown} />

<div class="flex flex-col h-screen overflow-hidden bg-base-100">
    <Toolbar
        onopen={handleOpenProject}
        onimport={handleImport}
        onsave={handleSaveProject}
        onexportxml={handleExportXml}
        onexportdict={handleExportDictionary}
        onundo={handleLemmaUndo}
        onredo={handleLemmaRedo}
        onsettings={() => (showSettings = true)}
        onhelp={() => (showHelp = true)}
    />

    <div class="flex-1 overflow-hidden">
        <Splitpanes theme="themed">
            <Pane minSize={20}>
                <div class="flex flex-col h-full">
                    <div
                        class="flex justify-between items-center px-4 py-2 bg-base-200 border-b border-base-300 font-medium text-sm"
                    >
                        <span class="text-md xl:text-lg font-bold px-2">DSL Editor</span>
                        <div class="flex gap-1">
                            <button
                                class="btn btn-ghost btn-xs xl:btn-sm"
                                title="Undo (Ctrl+Z)"
                                onclick={() => editorComponent?.triggerUndo()}
                            >
                                <Undo class="size-3/4" />
                            </button>
                            <button
                                class="btn btn-ghost btn-xs xl:btn-sm"
                                title="Redo (Ctrl+Y)"
                                onclick={() => editorComponent?.triggerRedo()}
                            >
                                <Redo class="size-3/4" />
                            </button>
                            <button
                                class="btn btn-ghost btn-xs xl:btn-sm"
                                title="Search and Replace (Ctrl+Shift+F)"
                                onclick={() => editorComponent?.triggerSearch()}
                            >
                                <Search class="size-3/4" />
                            </button>
                            <div class="divider divider-horizontal mx-0 h-4 self-center"></div>
                            <button
                                class="btn btn-ghost btn-xs xl:btn-sm text-xs xl:text-sm"
                                title="Insert entity"
                                onclick={() => (showEntityBrowser = true)}
                            >
                                ꝥ
                            </button>
                            <button
                                class="btn btn-ghost btn-xs xl:btn-sm"
                                title="Manage templates"
                                onclick={() => (showTemplateManager = true)}
                            >
                                <BookDashed class="size-3/4" />
                            </button>
                            <button
                                class="btn btn-ghost btn-xs xl:btn-sm"
                                title="Validate XML"
                                onclick={() => (showValidationPanel = true)}
                            >
                                <FileCheck class="size-3/4" />
                            </button>
                            <button
                                class="btn btn-ghost btn-xs xl:btn-sm"
                                class:text-error={$errorCounts.error > 0}
                                title="View logs"
                                onclick={() => (showErrorPanel = true)}
                            >
                                {#if $errorCounts.error > 0}
                                    <MessageCircleWarning size="14" color="var(--color-error)"/>
                                {:else}
                                    <ScrollText class="size-3/4" color="var(--color-success)"/>
                                {/if}
                            </button>
                        </div>
                    </div>
                    <Editor
                        bind:this={editorComponent}
                        onchange={handleEditorChange}
                    />
                </div>
            </Pane>
            <Pane minSize={20}>
                <Preview
                    content={previewContent}
                    onwordclick={handleWordClick}
                />
            </Pane>
        </Splitpanes>
    </div>

    <TemplateEditor bind:isopen={showTemplateManager} />

    {#if showEntityBrowser}
        <div class="modal modal-open">
            <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
            <div
                class="modal-backdrop"
                role="none"
                onclick={() => (showEntityBrowser = false)}
            ></div>
            <div class="modal-box max-w-4xl">
                <EntityBrowser
                    oninsert={handleEntityInsert}
                    onclose={() => (showEntityBrowser = false)}
                />
            </div>
        </div>
    {/if}

    {#if showErrorPanel}
        <div class="modal modal-open">
            <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
            <div
                class="modal-backdrop"
                role="none"
                onclick={() => (showErrorPanel = false)}
            ></div>
            <div class="modal-box max-w-3xl">
                <ErrorPanel onclose={() => (showErrorPanel = false)} />
            </div>
        </div>
    {/if}

    {#if showValidationPanel}
        <div class="modal modal-open">
            <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
            <div
                class="modal-backdrop"
                role="none"
                onclick={() => (showValidationPanel = false)}
            ></div>
            <div class="modal-box max-w-3xl">
                <ValidationPanel
                    xmlContent={previewContent}
                    onclose={() => (showValidationPanel = false)}
                />
            </div>
        </div>
    {/if}

    {#if showLemmatizer && selectedWordDiplomatic && selectedWordIndex >= 0}
        <div class="modal modal-open">
            <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
            <div
                class="modal-backdrop"
                role="none"
                onclick={handleLemmatizerClose}
            ></div>
            <div class="modal-box max-w-2xl">
                <Lemmatizer
                    facsimile={selectedWordFacsimile || ""}
                    diplomatic={selectedWordDiplomatic}
                    wordIndex={selectedWordIndex}
                    onclose={handleLemmatizerClose}
                    onsave={handleLemmatizerSave}
                />
            </div>
        </div>
    {/if}

    {#if isImporting}
        <div class="modal modal-open">
            <div class="modal-backdrop bg-base-100/50"></div>
            <div class="modal-box bg-transparent shadow-none shadow-transparent border-none flex flex-col items-center justify-center overflow-hidden">
                <span class="loading loading-spinner loading-lg text-primary"></span>
                <p class="mt-4 font-bold text-lg text-base-content">Importing...</p>
            </div>
        </div>
    {/if}

    <SettingsDialog bind:isopen={showSettings} />
    <HelpDialog bind:isopen={showHelp} />
</div>

<style>
    /* Splitpanes with custom theme */
    :global(.splitpanes.themed) {
        height: 100%;
    }

    :global(.splitpanes.themed .splitpanes__pane) {
        background-color: transparent;
    }

    :global(.splitpanes.themed .splitpanes__splitter) {
        background-color: var(--color-base-300);
        position: relative;
        flex-shrink: 0;
    }

    :global(.splitpanes.themed .splitpanes__splitter:hover) {
        background-color: var(--color-primary);
    }

    :global(.splitpanes.themed.splitpanes--vertical > .splitpanes__splitter) {
        width: 6px;
        cursor: col-resize;
    }

    :global(.splitpanes.themed.splitpanes--horizontal > .splitpanes__splitter) {
        height: 6px;
        cursor: row-resize;
    }
</style>
