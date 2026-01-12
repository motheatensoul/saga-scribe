<script lang="ts">
    import { onMount, tick } from "svelte";
    import { Splitpanes, Pane } from "svelte-splitpanes";
    import { open, save } from "@tauri-apps/plugin-dialog";
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
    import { editor } from "$lib/stores/editor";
    import { templateStore } from "$lib/stores/template";
    import { entityStore } from "$lib/stores/entities";
    import { settings } from "$lib/stores/settings";
    import { errorStore, errorCounts } from "$lib/stores/errors";
    import * as metadataStore from "$lib/stores/metadata.svelte";
    import type { Metadata } from "$lib/types/metadata";
    import { isMetadataEmpty } from "$lib/types/metadata";
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
        exportHtml,
        openFile,
        importFile,
        exportInflections,
        generateTeiHeader,
    } from "$lib/tauri";
    import { generateStandaloneHtml } from "$lib/utils/htmlExport";
    import { printToPdf } from "$lib/utils/pdfExport";
    import {
        dictionaryStore,
        inflectionStore,
        sessionLemmaStore,
        annotationStore,
        lemmaMappings,
        annotationHistory,
        canUndo,
        canRedo,
    } from "$lib/stores/dictionary";
    import { resolveResource, appDataDir } from "@tauri-apps/api/path";

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
    } from "@lucide/svelte";

    let editorComponent: Editor | null = $state<Editor | null>(null);
    let previewContent = $state("");
    let showTemplateManager = $state(false);
    let showEntityBrowser = $state(false);
    let showErrorPanel = $state(false);
    let showValidationPanel = $state(false);
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
    let entitiesJson = $state<string | null>(null);
    let normalizerJson = $state<string | null>(null);
    let entityMappingsJson = $state<string | null>(null);
    let isImporting = $state(false);
    let isMounting: boolean = $state(true);
    let showMetadataEditor = $state(false);
    let currentMetadata = $state<Metadata | undefined>(undefined);

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

        isMounting = false;
        errorStore.info("App", "Application ready");
    });

    // Compile without updating UI - returns the result
    async function compileOnly(content: string): Promise<string | null> {
        const template = $templateStore.active;
        if (!template) return null;

        // Use session store for lemma mappings - keyed by word INDEX
        // $lemmaMappings already provides the right format
        const compileLemmaMappings = $lemmaMappings.mappings;

        // Get full annotation set for non-lemma annotations
        const annotationSet = annotationStore.getSet();
        const hasAnnotations = annotationSet.annotations.length > 0;

        // Use dynamic metadata header if metadata exists, otherwise use template header
        const currentMetadata = metadataStore.getMetadata();
        let header = template.header;
        if (currentMetadata && !isMetadataEmpty(currentMetadata)) {
            try {
                header = await generateTeiHeader(
                    JSON.stringify(currentMetadata),
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
            customMappings: $entityStore.customMappings,
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

    async function doCompile(content: string) {
        try {
            const result = await compileOnly(content);
            if (result !== null) {
                previewContent = result;
            }
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
        isSpanExtend?: boolean,
    ) {
        if (isSpanExtend && showLemmatizer && selectedWordIndex >= 0) {
            // Shift-click extends selection to create a span
            spanEndWordIndex = wordIndex;
        } else {
            // Normal click - start new selection
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

    async function handleLemmatizerSave(
        wordIndex: number,
        lemma?: string,
        msa?: string,
    ) {
        console.log("handleLemmatizerSave called", { wordIndex, lemma, msa });
        // Wait for Svelte reactivity to propagate store updates
        await tick();
        console.log(
            "Current session lemma mappings:",
            $lemmaMappings.mappings,
        );
        // Trigger immediate recompile to update TEI output with new lemma
        // Cancel any pending auto-preview and compile directly
        clearTimeout(compileTimeout);
        doCompile($editor.content);
    }

    /**
     * Called when annotations are added or removed in the AnnotationPanel.
     * Triggers recompile to update the XML output.
     */
    async function handleAnnotationSave() {
        // Wait for Svelte reactivity to propagate store updates
        await tick();
        // Trigger immediate recompile to update TEI output
        clearTimeout(compileTimeout);
        doCompile($editor.content);
    }

    /**
     * Find all word indices matching a diplomatic form in the preview XML.
     * Used for bulk annotation feature.
     */
    function findMatchingWords(targetDiplomatic: string): number[] {
        if (!previewContent) return [];

        const indices: number[] = [];
        const parser = new DOMParser();

        try {
            const doc = parser.parseFromString(previewContent, "text/xml");
            const parseError = doc.querySelector("parsererror");
            if (parseError) return [];

            // Find all <w> elements
            const words = doc.querySelectorAll("w");
            let wordIndex = 0;

            for (const word of words) {
                // Get diplomatic form - check me:dipl first, then fallback to text content
                const diplEl = word.querySelector("dipl");
                let diplomatic = diplEl?.textContent?.trim();

                if (!diplomatic) {
                    // Fallback: use text content of the word (for single-level mode)
                    diplomatic = word.textContent?.trim() || "";
                }

                if (diplomatic === targetDiplomatic) {
                    indices.push(wordIndex);
                }
                wordIndex++;
            }
        } catch {
            // Parse error - return empty
            return [];
        }

        return indices;
    }

    // Undo/redo for annotations (lemmatization and other types)
    function handleLemmaUndo() {
        const action = annotationHistory.undo();
        if (!action) return;

        // Revert the action (without pushing to history again)
        if (action.type === "add") {
            // Undo an add: remove the annotation
            sessionLemmaStore.remove(action.annotation.id, false);
        } else if (action.type === "remove") {
            // Undo a remove: add back the annotation
            sessionLemmaStore.add(action.annotation, false);
        } else if (action.type === "update") {
            // Undo an update: restore the previous annotation
            if (action.previousAnnotation) {
                sessionLemmaStore.add(action.previousAnnotation, false);
            } else {
                sessionLemmaStore.remove(action.annotation.id, false);
            }
        }

        // Recompile to reflect changes
        clearTimeout(compileTimeout);
        doCompile($editor.content);

        // Get word index for feedback message
        const wordIndex =
            action.annotation.target.type === "word"
                ? action.annotation.target.wordIndex
                : action.annotation.target.type === "char"
                  ? action.annotation.target.wordIndex
                  : action.annotation.target.startWord;
        errorStore.info("Undo", `Undid annotation for word #${wordIndex}`);
    }

    function handleLemmaRedo() {
        const action = annotationHistory.redo();
        if (!action) return;

        // Reapply the action (without pushing to history again)
        if (action.type === "add") {
            // Redo an add: add the annotation
            sessionLemmaStore.add(action.annotation, false);
        } else if (action.type === "remove") {
            // Redo a remove: remove the annotation
            sessionLemmaStore.remove(action.annotation.id, false);
        } else if (action.type === "update") {
            // Redo an update: apply the new annotation
            sessionLemmaStore.add(action.annotation, false);
        }

        // Recompile to reflect changes
        clearTimeout(compileTimeout);
        doCompile($editor.content);

        // Get word index for feedback message
        const wordIndex =
            action.annotation.target.type === "word"
                ? action.annotation.target.wordIndex
                : action.annotation.target.type === "char"
                  ? action.annotation.target.wordIndex
                  : action.annotation.target.startWord;
        errorStore.info("Redo", `Redid annotation for word #${wordIndex}`);
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

                // Clear annotation history and load annotations
                annotationHistory.clear();
                if (project.annotations) {
                    // New format: load full annotation set
                    sessionLemmaStore.loadSet(project.annotations);
                } else {
                    // Legacy format: convert confirmations to annotations
                    sessionLemmaStore.loadLegacyConfirmations(project.confirmations);
                }

                // Restore template if possible
                const templates = $templateStore.templates;
                const template = templates.find(
                    (t) => t.id === project.manifest.template_id,
                );
                if (template) {
                    templateStore.setActive(template);
                }

                // Restore metadata if present
                if (project.metadata) {
                    currentMetadata = project.metadata;
                    metadataStore.setMetadata(project.metadata);
                } else {
                    currentMetadata = undefined;
                    metadataStore.resetMetadata();
                }

                // Cancel any pending auto-preview compile and trigger recompile
                clearTimeout(compileTimeout);
                await doCompile(project.source);

                errorStore.info("Project", `Opened project from ${pathStr}`);
            } else {
                // Open plain DSL file (backwards compatibility)
                const file = await openFile(pathStr);
                editor.setFile(file.path, file.content);
                editorComponent?.setContent(file.content);

                // Clear history and annotations for new file
                annotationHistory.clear();
                sessionLemmaStore.clear();

                // Cancel any pending auto-preview compile
                clearTimeout(compileTimeout);
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
            clearTimeout(compileTimeout);
            await doCompile($editor.content);

            // Serialize session confirmations (lemma mappings for backward compat)
            const confirmationsJson = JSON.stringify($lemmaMappings.mappings);

            // Serialize full annotation set (new in v1.2)
            const annotationsJson = JSON.stringify(sessionLemmaStore.getSet());

            // Save project archive
            const metadataJson = currentMetadata
                ? JSON.stringify(currentMetadata)
                : undefined;
            await saveProject(
                path,
                $editor.content,
                previewContent,
                confirmationsJson,
                template.id,
                metadataJson,
                annotationsJson,
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
            clearTimeout(compileTimeout);
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

    /**
     * Apply XSLT transformation to XML content
     * Extracted from XsltRenderer.svelte for reuse in exports
     */
    async function applyXsltTransform(xmlContent: string): Promise<string> {
        // Load XSL stylesheet
        const response = await fetch("/xsl/simple.xsl");
        if (!response.ok) {
            throw new Error(`Failed to load stylesheet: ${response.statusText}`);
        }
        const xslText = await response.text();

        const parser = new DOMParser();
        const xslDoc = parser.parseFromString(xslText, "application/xml");

        const parseError = xslDoc.querySelector("parsererror");
        if (parseError) {
            throw new Error(`XSL parse error: ${parseError.textContent}`);
        }

        const processor = new XSLTProcessor();
        processor.importStylesheet(xslDoc);

        // Replace entity references with placeholders to avoid XML parsing errors
        let processedXml = xmlContent;
        const entityPattern = /&([a-zA-Z][a-zA-Z0-9]*);/g;
        const entityMap = new Map<string, string>();
        let entityCounter = 0;

        processedXml = processedXml.replace(entityPattern, (match, name) => {
            // Skip standard XML entities
            if (["lt", "gt", "amp", "quot", "apos"].includes(name)) {
                return match;
            }
            const placeholder = `__ENTITY_${entityCounter}__`;
            entityMap.set(placeholder, name);
            entityCounter++;
            return placeholder;
        });

        const xmlDoc = parser.parseFromString(processedXml, "application/xml");

        const xmlParseError = xmlDoc.querySelector("parsererror");
        if (xmlParseError) {
            throw new Error(`XML parse error: ${xmlParseError.textContent}`);
        }

        // Apply XSLT transformation
        const resultDoc = processor.transformToDocument(xmlDoc);

        if (!resultDoc || !resultDoc.documentElement) {
            throw new Error("XSLT transformation produced no output");
        }

        // Get the HTML content
        let html = resultDoc.documentElement.outerHTML;

        // Resolve entity placeholders to actual glyphs
        const entities = $entityStore.entities;
        for (const [placeholder, entityName] of entityMap) {
            const entity = entities[entityName];
            const glyph = entity?.char || `[${entityName}]`;
            html = html.replaceAll(placeholder, glyph);
        }

        return html;
    }

    async function handleExportHtml() {
        const template = $templateStore.active;
        if (!template) {
            errorStore.warning(
                "Export",
                "Please select a template before exporting",
            );
            return;
        }

        const path = await save({
            filters: [{ name: "HTML", extensions: ["html", "htm"] }],
            defaultPath: $editor.filePath
                ? $editor.filePath.replace(/\.[^.]+$/, ".html")
                : undefined,
        });
        if (!path) return;

        try {
            // Ensure we have fresh compiled output
            clearTimeout(compileTimeout);
            await doCompile($editor.content);

            // Apply XSLT transformation
            const htmlBody = await applyXsltTransform(previewContent);

            // Generate standalone HTML with embedded styles
            const fileName = $editor.filePath
                ? $editor.filePath.split("/").pop()?.replace(/\.[^.]+$/, "")
                : "export";
            const fullHtml = generateStandaloneHtml(htmlBody, { title: fileName });

            await exportHtml(path, fullHtml);
            errorStore.info("Export", `Exported HTML to ${path}`);
        } catch (e) {
            errorStore.error("Export", `Failed to export HTML: ${e}`);
        }
    }

    async function handleExportPdf() {
        const template = $templateStore.active;
        if (!template) {
            errorStore.warning(
                "Export",
                "Please select a template before exporting",
            );
            return;
        }

        try {
            // Ensure we have fresh compiled output
            clearTimeout(compileTimeout);
            await doCompile($editor.content);

            // Apply XSLT transformation
            const htmlBody = await applyXsltTransform(previewContent);

            // Generate standalone HTML
            const fileName = $editor.filePath
                ? $editor.filePath.split("/").pop()?.replace(/\.[^.]+$/, "")
                : "export";
            const fullHtml = generateStandaloneHtml(htmlBody, {
                title: fileName,
                pageBreakStyle: "print-break",
            });

            // Open print dialog (user can "Save as PDF")
            await printToPdf(fullHtml, { pageSize: "A4" });

            errorStore.info(
                "Export",
                "Print dialog opened - select 'Save as PDF' to export",
            );
        } catch (e) {
            errorStore.error("Export", `Failed to prepare PDF: ${e}`);
        }
    }

    async function handleImport() {
        const path = await open({
            filters: [
                {
                    name: "All Supported Formats",
                    extensions: ["xml", "tei", "txt"],
                },
                { name: "TEI/XML", extensions: ["xml", "tei"] },
                { name: "Text File", extensions: ["txt"] },
            ],
        });
        if (!path) return;

        const pathStr = path as string;
        isImporting = true;

        // Yield to let browser paint spinner before starting work
        await tick();
        await new Promise((resolve) => setTimeout(resolve, 16));

        try {
            const result = await importFile(pathStr);

            // Clear history and annotations
            annotationHistory.clear();
            sessionLemmaStore.clear();
            clearTimeout(compileTimeout);

            const compiled = await compileOnly(result.dsl);

            editorComponent?.setContent(result.dsl);

            // Cancel the compile that setContent just triggered via onchange
            clearTimeout(compileTimeout);

            editor.setFile(null, result.dsl);

            // Set metadata if present in imported file
            if (result.metadata) {
                currentMetadata = result.metadata;
                metadataStore.setMetadata(result.metadata);
                errorStore.info("Import", `Imported content and metadata from ${pathStr}`);
            } else {
                errorStore.info("Import", `Imported content from ${pathStr}`);
            }

            if (compiled !== null) {
                previewContent = compiled;
            }
        } catch (e) {
            errorStore.error("Import", `Failed to import: ${e}`);
        } finally {
            // Wait for RenderedText async parsing to complete before hiding spinner
            await new Promise((resolve) => setTimeout(resolve, 500));
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
        onexporthtml={handleExportHtml}
        onexportpdf={handleExportPdf}
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
                        <span class="text-md xl:text-lg font-bold px-2"
                            >DSL Editor</span
                        >
                        <div class="flex gap-1">
                            <button
                                class="btn btn-ghost btn-xs xl:btn-sm"
                                title="Undo (Ctrl+Z)"
                                onclick={() =>
                                    editorComponent?.triggerUndo()}
                            >
                                <Undo class="size-3/4" />
                            </button>
                            <button
                                class="btn btn-ghost btn-xs xl:btn-sm"
                                title="Redo (Ctrl+Y)"
                                onclick={() =>
                                    editorComponent?.triggerRedo()}
                            >
                                <Redo class="size-3/4" />
                            </button>
                            <button
                                class="btn btn-ghost btn-xs xl:btn-sm"
                                title="Search and Replace (Ctrl+Shift+F)"
                                onclick={() =>
                                    editorComponent?.triggerSearch()}
                            >
                                <Search class="size-3/4" />
                            </button>
                            <div
                                class="divider divider-horizontal mx-0 h-4 self-center"
                            ></div>
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
                                title="Edit manuscript metadata"
                                onclick={() => (showMetadataEditor = true)}
                            >
                                <FileText class="size-3/4" />
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
                                    <MessageCircleWarning
                                        size="14"
                                        color="var(--color-error)"
                                    />
                                {:else}
                                    <ScrollText
                                        class="size-3/4"
                                        color="var(--color-success)"
                                    />
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

    <MetadataEditor
        bind:isopen={showMetadataEditor}
        bind:metadata={currentMetadata}
        onSave={(meta) => {
            metadataStore.updateMetadata(meta);
        }}
    />

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
            <div class="modal-box max-w-2xl p-0">
                <!-- Tabs -->
                <div class="tabs tabs-boxed bg-base-200 rounded-none">
                    <button
                        type="button"
                        class="tab"
                        class:tab-active={wordPanelTab === "lemmatize"}
                        onclick={() => wordPanelTab = "lemmatize"}
                    >
                        Lemmatize
                    </button>
                    <button
                        type="button"
                        class="tab"
                        class:tab-active={wordPanelTab === "annotate"}
                        onclick={() => wordPanelTab = "annotate"}
                    >
                        Annotate
                    </button>
                </div>
                <!-- Tab content -->
                <div class="p-4">
                    <div class:hidden={wordPanelTab !== "lemmatize"}>
                        <Lemmatizer
                            facsimile={selectedWordFacsimile || ""}
                            diplomatic={selectedWordDiplomatic}
                            wordIndex={selectedWordIndex}
                            onclose={handleLemmatizerClose}
                            onsave={handleLemmatizerSave}
                        />
                    </div>
                    <div class:hidden={wordPanelTab !== "annotate"}>
                        <AnnotationPanel
                            facsimile={selectedWordFacsimile || ""}
                            diplomatic={selectedWordDiplomatic}
                            wordIndex={selectedWordIndex}
                            spanEndIndex={spanEndWordIndex}
                            onclose={handleLemmatizerClose}
                            onsave={handleAnnotationSave}
                            onFindMatchingWords={findMatchingWords}
                        />
                    </div>
                </div>
            </div>
        </div>
    {/if}

    {#if isImporting}
        <div class="modal modal-open">
            <div class="modal-backdrop bg-base-100/50"></div>
            <div
                class="modal-box bg-transparent shadow-none shadow-transparent border-none flex flex-col items-center justify-center overflow-hidden"
            >
                <!-- SVG spinner with explicit animation -->
                <svg
                    class="spinner-svg"
                    viewBox="0 0 50 50"
                    width="48"
                    height="48"
                >
                    <circle
                        cx="25"
                        cy="25"
                        r="20"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="4"
                        stroke-linecap="round"
                        stroke-dasharray="90, 150"
                        stroke-dashoffset="0"
                        class="text-primary"
                    >
                        <animateTransform
                            attributeName="transform"
                            type="rotate"
                            from="0 25 25"
                            to="360 25 25"
                            dur="1s"
                            repeatCount="indefinite"
                        />
                    </circle>
                </svg>
                <p class="mt-4 font-bold text-lg text-base-content">
                    Importing…
                </p>
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
