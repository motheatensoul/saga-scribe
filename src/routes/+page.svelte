<script lang="ts">
    import { onMount, tick } from "svelte";
    import { Splitpanes, Pane } from "svelte-splitpanes";
    import { open } from "@tauri-apps/plugin-dialog";
    import { saveAs } from "$lib/utils/saveDialog";
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
    import BusyOverlay from "$lib/components/BusyOverlay.svelte";
    import { editor } from "$lib/stores/editor";
    import { templateStore } from "$lib/stores/template";
    import { entityStore } from "$lib/stores/entities";
    import { wordNormalizedForm, wordsWithDiplomaticForm } from "$lib/utils/wordNorm";
    import { settings } from "$lib/stores/settings";
    import { stylesheetStore } from "$lib/stores/stylesheets";
    import { errorStore, errorCounts } from "$lib/stores/errors";
    import { validationStore } from "$lib/stores/validation";
    import * as metadataStore from "$lib/stores/metadata.svelte";
    import { importedStore } from "$lib/stores/imported.svelte";
    import { compileSession } from "$lib/stores/compileSession";
    import { renumberWord } from "$lib/stores/renumber";
    import { preservationStore } from "$lib/stores/preservation.svelte";
    import type { Metadata } from "$lib/types/metadata";
    import { isMetadataEmpty } from "$lib/types/metadata";
    import {
        compileDsl,
        saveProject,
        openProject,
        exportTei,
        exportHtml,
        exportPdf,
        cancelPdfExport,
        pdfExportSupported,
        openFile,
        importFile,
        exportInflections,
        generateTeiHeader,
        validateXml,
    } from "$lib/tauri";
    import type { ImportedDocument } from "$lib/tauri";
    import { loadAppResources } from "$lib/startup";
    import { importedLemmas } from "$lib/utils/importedLemmas";
    import { generateStandaloneHtml } from "$lib/utils/htmlExport";
    import { printToPdf, withPrintStyles } from "$lib/utils/pdfExport";
    import {
        type XsltStage,
        createXsltProcessor,
        loadStylesheetText,
        runXsltTransform,
    } from "$lib/utils/xslt";
    import {
        inflectionStore,
        sessionLemmaStore,
        annotationStore,
        lemmaMappings,
        annotationHistory,
    } from "$lib/stores/dictionary";

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
    /** The selected word's normalized form, as compiled */
    let selectedWordNormalized = $derived(
        showLemmatizer && selectedWordIndex >= 0
            ? wordNormalizedForm(previewContent, selectedWordIndex, $entityStore.entities)
            : null,
    );
    // For span selections (shift-click extends)
    let spanEndWordIndex = $state<number | null>(null);
    let compileTimeout: ReturnType<typeof setTimeout>;
    let entitiesJson = $derived(
        Object.keys($entityStore.entities).length > 0
            ? JSON.stringify({
                  version: "1.0",
                  name: "SagaScribe",
                  entities: $entityStore.entities,
              })
            : null,
    );
    const defaultStylesheetPath = "/xsl/simple.xsl";
    let activeStylesheetPath = $derived(
        $stylesheetStore.find(
            (item) => item.id === $settings.activeStylesheetId,
        )?.path ?? defaultStylesheetPath,
    );
    let normalizerJson = $state<string | null>(null);
    let entityMappingsJson = $state<string | null>(null);
    let isImporting = $state(false);
    let showMetadataEditor = $state(false);
    let currentMetadata = $state<Metadata | undefined>(undefined);

    onMount(async () => {
        ({ normalizerJson, entityMappingsJson } = await loadAppResources());
    });

    // Compile without updating UI - returns the result
    async function compileOnly(content: string): Promise<string | null> {
        const template = $templateStore.active;
        if (!template) return null;

        const entityOptions = {
            entitiesJson: entitiesJson ?? undefined,
            normalizerJson: normalizerJson ?? undefined,
            entityMappingsJson: entityMappingsJson ?? undefined,
            customMappings: $entityStore.customMappings,
        };
        // Lemmas and annotations are numbered by the previous compile, and
        // renumbered after each one (compileSession), so they are read only
        // when it's this compile's turn
        // Empty lemmas too: removed ones (taken out of imported files) and
        // words with only a normalized form
        const lemmaMappingsJson = () => {
            const writes = annotationStore.getLemmaWrites();
            return Object.keys(writes).length > 0 ? JSON.stringify(writes) : undefined;
        };

        if (importedStore.isImportedMode) {
            return importedStore.compile(content, () => ({
                ...entityOptions,
                lemmaMappingsJson: lemmaMappingsJson(),
            }));
        }

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

        const result = await compileSession.run(content, (guidance) => {
            const annotationSet = annotationStore.getSet();
            return compileDsl(
                content,
                header,
                template.footer,
                {
                    wordWrap: template.wordWrap,
                    autoLineNumbers: template.autoLineNumbers,
                    multiLevel: template.multiLevel,
                    wrapPages: template.wrapPages,
                    ...entityOptions,
                    lemmaMappingsJson: lemmaMappingsJson(),
                    annotationsJson:
                        annotationSet.annotations.length > 0 ? JSON.stringify(annotationSet) : undefined,
                },
                guidance,
            );
        });
        return result.xml;
    }

    // Words inserted or deleted: everything stored by word number moves along
    compileSession.onRenumbering = (map) => {
        annotationStore.renumber(map);
        const move = (index: number) => renumberWord(map, index) ?? -1;
        if (selectedWordIndex >= 0) selectedWordIndex = move(selectedWordIndex);
        if (spanEndWordIndex !== null) {
            const end = move(spanEndWordIndex);
            spanEndWordIndex = end >= 0 ? end : null;
        }
    };

    /** Idle time after the last compile before the output is validated. */
    const VALIDATION_DELAY_MS = 1000;
    let validationTimeout: ReturnType<typeof setTimeout>;
    let validationRun = 0;

    /**
     * Validates compiled XML once editing pauses. Validating a large document
     * takes hundreds of milliseconds, so it never holds up the preview, and a
     * result is discarded if newer output arrived meanwhile.
     */
    function scheduleValidation(xml: string) {
        clearTimeout(validationTimeout);
        const run = ++validationRun;
        validationTimeout = setTimeout(async () => {
            const schemaId = $templateStore.active?.validationSchemaId || "tei-p5";
            try {
                const validationResult = await validateXml(xml, schemaId);
                if (run === validationRun) validationStore.setResult(validationResult);
            } catch (validationError) {
                if (run !== validationRun) return;
                console.warn("Validation failed:", validationError);
                // Don't block on validation errors - just clear the validation state
                validationStore.setResult(null);
            }
        }, VALIDATION_DELAY_MS);
    }

    /** Compiles and schedules validation; returns whether compilation succeeded. */
    async function doCompile(content: string): Promise<boolean> {
        try {
            const result = await compileOnly(content);
            if (result === null) return false;
            previewContent = result;
            scheduleValidation(result);
            return true;
        } catch (e) {
            previewContent = `Error: ${e}`;
            // Drop any pending validation of the previous output
            clearTimeout(validationTimeout);
            validationRun++;
            validationStore.setResult(null);
            return false;
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

    /** Compiles now, instead of the pending preview compile; returns whether it succeeded */
    function compileNow(content = $editor.content): Promise<boolean> {
        clearTimeout(compileTimeout);
        return doCompile(content);
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
        _element: HTMLElement,
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
            spanEndWordIndex = null;
            showLemmatizer = true;
        }
    }

    function handleLemmatizerClose() {
        showLemmatizer = false;
        selectedWordFacsimile = null;
        selectedWordDiplomatic = null;
        selectedWordIndex = -1;
        spanEndWordIndex = null;
    }

    let warnedAnnotationsNotExported = false;

    /**
     * Imported-mode output is spliced from the original XML: confirmed lemmas
     * are written into it, other annotations (notes, semantic, paleographic)
     * would add markup the file doesn't have, so they stay in the project.
     * Say so once per session.
     */
    function warnIfAnnotationsNotExported() {
        if (!importedStore.isImportedMode || warnedAnnotationsNotExported) return;
        warnedAnnotationsNotExported = true;
        errorStore.warning(
            "Annotations",
            "On an imported TEI file, lemmas are written into the file; notes and other annotations are kept in the project only.",
        );
    }

    /** A lemma or annotation changed: shows it in the output right away */
    async function handleLemmatizerSave() {
        // Wait for Svelte reactivity to propagate store updates
        await tick();
        compileNow();
    }

    async function handleAnnotationSave() {
        warnIfAnnotationsNotExported();
        await handleLemmatizerSave();
    }

    const findMatchingWords = (diplomatic: string) => wordsWithDiplomaticForm(previewContent, diplomatic);

    /**
     * Undoes or redoes the last annotation change (lemmas and the others),
     * without recording it in the history again.
     */
    function stepAnnotationHistory(direction: "Undo" | "Redo") {
        const action = direction === "Undo" ? annotationHistory.undo() : annotationHistory.redo();
        if (!action) return;

        // The annotation the step leaves in place, if any
        const result =
            action.type === "add"
                ? direction === "Undo" ? null : action.annotation
                : action.type === "remove"
                  ? direction === "Undo" ? action.annotation : null
                  : direction === "Undo" ? (action.previousAnnotation ?? null) : action.annotation;
        if (result) {
            sessionLemmaStore.add(result, false);
        } else {
            sessionLemmaStore.remove(action.annotation.id, false);
        }
        compileNow();

        const { target } = action.annotation;
        const wordIndex = target.type === "span" ? target.startWord : target.wordIndex;
        errorStore.info(direction, `${direction === "Undo" ? "Undid" : "Redid"} annotation for word #${wordIndex}`);
    }

    const handleLemmaUndo = () => stepAnnotationHistory("Undo");
    const handleLemmaRedo = () => stepAnnotationHistory("Redo");

    /**
     * Enters imported mode on `document` and the original file's parts, or
     * leaves it if there's no document; returns whether it's imported mode.
     */
    function setImportedDocument(
        document: ImportedDocument | undefined,
        originalBodyXml: string | undefined,
        preamble = "",
        postamble = "",
    ): boolean {
        if (!document || !originalBodyXml) {
            importedStore.reset();
            preservationStore.clear();
            return false;
        }
        importedStore.load({
            segments: document.segments,
            spans: document.spans,
            originalBodyXml,
            originalPreamble: preamble,
            originalPostamble: postamble,
            isMenota: document.is_menota ?? false,
            dslRules: document.dsl_rules ?? 0,
        });
        preservationStore.setSections({ preamble, postamble });
        return true;
    }

    // Project file handling
    async function handleOpenProject() {
        const path = await open({
            filters: [
                { name: "Saga Scribe Project", extensions: ["teis"] },
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

                setImportedDocument(
                    project.imported_document,
                    project.original_body_xml,
                    project.original_preamble,
                    project.original_postamble,
                );

                await compileNow(project.source);

                errorStore.info("Project", `Opened project from ${pathStr}`);
            } else {
                // Open plain DSL file (backwards compatibility)
                const file = await openFile(pathStr);
                editor.setFile(file.path, file.content);
                editorComponent?.setContent(file.content);

                // Clear history and annotations for new file
                annotationHistory.clear();
                sessionLemmaStore.clear();
                importedStore.reset();
                preservationStore.clear();

                await compileNow(file.content);

                errorStore.info("File", `Opened DSL file from ${pathStr}`);
            }
        } catch (e) {
            errorStore.error("Open", `Failed to open: ${e}`);
        }
    }

    async function handleSaveProject() {
        const template = requireTemplate("Save", "saving");
        if (!template) return;

        // Get save path (use existing or prompt for new)
        let path = $editor.filePath;
        if (!path || !path.endsWith(".teis")) {
            path = await saveAs({ name: "Saga Scribe Project", extensions: ["teis"] }, {
                defaultPath: pathWithExtension("teis"),
            });
        }
        if (!path) return;

        try {
            // Ensure we have fresh compiled output; never write a failed compile
            if (!(await compileNow())) {
                errorStore.error(
                    "Save",
                    "Project not saved: the document could not be compiled. Fix the errors and save again.",
                );
                return;
            }

            // Serialize session confirmations (lemma mappings for backward compat)
            const confirmationsJson = JSON.stringify($lemmaMappings.mappings);

            // Serialize full annotation set (new in v1.2)
            const annotationsJson = JSON.stringify(sessionLemmaStore.getSet());

            // Save project archive
            const metadataJson = currentMetadata
                ? JSON.stringify(currentMetadata)
                : undefined;
            const segmentsJson = importedStore.isImportedMode
                ? importedStore.buildManifestJson()
                : undefined;
            const originalBodyXml = importedStore.isImportedMode
                ? importedStore.originalBodyXml
                : undefined;
            const originalPreamble = importedStore.isImportedMode
                ? importedStore.originalPreamble
                : undefined;
            const originalPostamble = importedStore.isImportedMode
                ? importedStore.originalPostamble
                : undefined;
            await saveProject(
                path,
                $editor.content,
                previewContent,
                confirmationsJson,
                template.id,
                metadataJson,
                annotationsJson,
                segmentsJson,
                originalBodyXml,
                originalPreamble,
                originalPostamble,
            );

            editor.setFile(path, $editor.content);
            errorStore.info("Project", `Saved project to ${path}`);
        } catch (e) {
            errorStore.error("Save", `Failed to save project: ${e}`);
        }
    }

    /** The active template; logs why `action` can't go ahead without one */
    function requireTemplate(category: string, action: string) {
        const template = $templateStore.active;
        if (!template) {
            errorStore.warning(category, `Please select a template before ${action}`);
        }
        return template;
    }

    /** The open file's path with `extension` instead of its own, as a default for saving */
    function pathWithExtension(extension: string): string | undefined {
        return $editor.filePath?.replace(/\.[^.]+$/, `.${extension}`);
    }

    /** The open file's name without its extension, as an export's title */
    function fileStem(): string | undefined {
        return $editor.filePath ? $editor.filePath.split("/").pop()?.replace(/\.[^.]+$/, "") : "export";
    }

    async function handleExportXml() {
        if (!requireTemplate("Export", "exporting")) return;

        const path = await saveAs({ name: "TEI-XML", extensions: ["xml"] }, {
            defaultPath: pathWithExtension("xml"),
        });
        if (!path) return;

        try {
            // Ensure we have fresh compiled output; never export a failed compile
            if (!(await compileNow())) {
                errorStore.error(
                    "Export",
                    "Not exported: the document could not be compiled. Fix the errors and export again.",
                );
                return;
            }

            await exportTei(path, previewContent);
            errorStore.info("Export", `Exported TEI-XML to ${path}`);
        } catch (e) {
            errorStore.error("Export", `Failed to export: ${e}`);
        }
    }

    async function handleExportDictionary() {
        const path = await saveAs({ name: "JSON", extensions: ["json"] }, {
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

    /** Label of the running HTML/PDF export step; non-null shows the progress overlay. */
    let exportProgress = $state<string | null>(null);
    let exportController: AbortController | null = null;

    const EXPORT_STAGE_LABELS: Record<XsltStage, string> = {
        preparing: "Preparing document…",
        parsing: "Parsing XML…",
        transforming: "Applying stylesheet…",
        sanitizing: "Cleaning up output…",
    };

    /**
     * Runs an export behind the progress overlay. The work yields between steps,
     * so the UI stays painted and the user can cancel.
     */
    async function withExportProgress(
        what: string,
        task: (signal: AbortSignal) => Promise<void>,
    ) {
        const controller = new AbortController();
        exportController = controller;
        exportProgress = "Starting…";
        try {
            await task(controller.signal);
        } catch (e) {
            if (controller.signal.aborted) {
                errorStore.info("Export", `${what} export cancelled`);
            } else {
                errorStore.error("Export", `Failed to export ${what}: ${e}`);
            }
        } finally {
            exportProgress = null;
            exportController = null;
        }
    }

    function cancelExport() {
        exportController?.abort();
    }

    /** Compiles the document and renders it to sanitized HTML with the active stylesheet. */
    async function renderExportHtml(signal: AbortSignal): Promise<string> {
        exportProgress = "Compiling…";
        // Never export a failed compile
        if (!(await compileNow())) {
            throw new Error(
                "the document could not be compiled. Fix the errors and try again.",
            );
        }
        signal.throwIfAborted();

        exportProgress = "Loading stylesheet…";
        const processor = createXsltProcessor(
            await loadStylesheetText(activeStylesheetPath),
        );
        return runXsltTransform(processor, previewContent, $entityStore.entities, {
            signal,
            onStage: (stage) => (exportProgress = EXPORT_STAGE_LABELS[stage]),
        });
    }

    async function handleExportHtml() {
        if (!requireTemplate("Export", "exporting")) return;

        const path = await saveAs({ name: "HTML", extensions: ["html", "htm"] }, {
            defaultPath: pathWithExtension("html"),
        });
        if (!path) return;

        await withExportProgress("HTML", async (signal) => {
            const htmlBody = await renderExportHtml(signal);

            // Generate standalone HTML with embedded styles
            const fileName = fileStem();
            const fullHtml = generateStandaloneHtml(htmlBody, { title: fileName });
            signal.throwIfAborted();

            exportProgress = "Writing file…";
            await exportHtml(path, fullHtml);
            errorStore.info("Export", `Exported HTML to ${path}`);
        });
    }

    async function handleExportPdf() {
        if (!requireTemplate("Export", "exporting")) return;

        // Where the backend writes PDFs itself, ask where to save first;
        // otherwise the system print dialog offers "Save as PDF".
        const direct = await pdfExportSupported().catch(() => false);
        let path: string | null = null;
        if (direct) {
            path = await saveAs({ name: "PDF", extensions: ["pdf"] }, {
                defaultPath: pathWithExtension("pdf"),
            });
            if (!path) return;
        }

        await withExportProgress("PDF", async (signal) => {
            const htmlBody = await renderExportHtml(signal);

            // Generate standalone HTML
            const fileName = fileStem();
            const fullHtml = generateStandaloneHtml(htmlBody, {
                title: fileName,
                pageBreakStyle: "print-break",
            });
            signal.throwIfAborted();

            if (path) {
                exportProgress = "Laying out pages…";
                // The typesetter is large, so it's only loaded for PDF export
                const { paginateForPrint } = await import("$lib/utils/paginate");
                const pages = await paginateForPrint(withPrintStyles(fullHtml, "A4"), signal);

                // Printed in a hidden window; this one stays responsive
                exportProgress = "Writing PDF…";
                const stop = () => void cancelPdfExport();
                signal.addEventListener("abort", stop);
                try {
                    await exportPdf(pages, path);
                } finally {
                    signal.removeEventListener("abort", stop);
                }
                signal.throwIfAborted();
                errorStore.info("Export", `Exported PDF to ${path}`);
            } else {
                exportProgress = "Laying out pages…";
                await printToPdf(fullHtml, { pageSize: "A4" });
                errorStore.info(
                    "Export",
                    "Print dialog opened - select 'Save as PDF' to export",
                );
            }
        });
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

            const imported = result.isImportedMode ? result.importedDocument : undefined;
            if (
                setImportedDocument(
                    imported,
                    result.originalBodyXml,
                    result.originalPreamble,
                    result.originalPostamble,
                )
            ) {
                const lemmas = importedLemmas(imported!.segments, imported!.is_menota ?? false);
                if (lemmas.missingLemmas > 0) {
                    errorStore.warning("Import", `Missing lemma for ${lemmas.missingLemmas} word(s); left empty.`);
                }
                if (lemmas.missingNormalized > 0) {
                    errorStore.warning("Import", `Missing normalized form for ${lemmas.missingNormalized} word(s).`);
                }
                if (Object.keys(lemmas.confirmations).length > 0) {
                    annotationStore.loadLegacyConfirmations(lemmas.confirmations);
                }
                for (const [wordform, form] of lemmas.inflections) {
                    inflectionStore.addMapping(wordform, form);
                }
            }

            editorComponent?.setContent(result.dsl);

            // Cancel the compile that setContent just triggered via onchange
            clearTimeout(compileTimeout);

            editor.setFile(null, result.dsl);

            // Set metadata if present in imported file
            if (result.metadata) {
                currentMetadata = result.metadata;
                metadataStore.setMetadata(result.metadata);
            }

            isImporting = false;

            // Let the editor render before compiling
            await tick();
            await new Promise((resolve) => setTimeout(resolve, 16));

            const compiled = await compileOnly(result.dsl);

            if (result.metadata) {
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
            isImporting = false;
        }
    }

    // Keyboard shortcuts
    function handleKeydown(event: KeyboardEvent) {
        // Escape: Close any open modal (a11y requirement)
        if (event.key === "Escape") {
            if (showLemmatizer) {
                showLemmatizer = false;
                return;
            }
            if (showEntityBrowser) {
                showEntityBrowser = false;
                return;
            }
            if (showErrorPanel) {
                showErrorPanel = false;
                return;
            }
            if (showValidationPanel) {
                showValidationPanel = false;
                return;
            }
            if (showTemplateManager) {
                showTemplateManager = false;
                return;
            }
            if (showMetadataEditor) {
                showMetadataEditor = false;
                return;
            }
            // Note: showSettings and showHelp are handled by their own components
            return;
        }

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
                        <div class="flex items-center gap-2 px-2">
                            <span class="text-md xl:text-lg font-bold">
                                DSL Editor
                            </span>
                            {#if importedStore.isImportedMode}
                                <span class="badge badge-outline badge-sm text-xs">
                                    Imported
                                </span>
                            {/if}
                        </div>
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
                        onchanges={(changes) => compileSession.record(changes)}
                        onchange={handleEditorChange}
                    />
                </div>
            </Pane>
            <Pane minSize={20}>
                <Preview
                    content={previewContent}
                    xslPath={activeStylesheetPath}
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
                        {#key selectedWordIndex}
                            <Lemmatizer
                                facsimile={selectedWordFacsimile || ""}
                                diplomatic={selectedWordDiplomatic}
                                normalized={selectedWordNormalized}
                                wordIndex={selectedWordIndex}
                                onclose={handleLemmatizerClose}
                                onsave={handleLemmatizerSave}
                            />
                        {/key}
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

    {#if exportProgress !== null}
        <BusyOverlay title="Exporting…" step={exportProgress} oncancel={cancelExport} />
    {/if}

    {#if isImporting}
        <BusyOverlay title="Importing…" />
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
