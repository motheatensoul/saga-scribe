<script lang="ts">
    import { editor, fileName } from "$lib/stores/editor";
    import { templateStore } from "$lib/stores/template";
    import { settings } from "$lib/stores/settings";
    import { errorCounts } from "$lib/stores/errors";
    // Lemma and annotation actions are recorded in the annotation history
    import { canUndoAnnotation as canUndo, canRedoAnnotation as canRedo } from "$lib/stores/annotations";

    //Icons
    import {
        Settings as SettingsIcon,
        BadgeQuestionMark as HelpIcon,
        Save as SaveIcon,
        FolderOpen as OpenIcon,
        FileUp as ImportIcon,
        Undo as UndoIcon,
        Redo as RedoIcon,
        ChevronDown,
        BookDashed,
        FileText,
        FileCheck,
        ScrollText,
        MessageCircleWarning,
    } from "@lucide/svelte";

    let {
        onopen,
        onimport,
        onsave,
        onexportxml,
        onexportdict,
        onexporthtml,
        onexportpdf,
        ontemplates,
        onmetadata,
        onvalidate,
        onundo,
        onredo,
        onlogs,
        onsettings,
        onhelp,
    }: {
        onopen?: () => void; // Parent handles opening (shows dialog, loads project)
        onimport?: () => void; // Parent handles importing (shows dialog, converts content)
        onsave?: () => void; // Parent handles saving (shows dialog if needed, saves project)
        onexportxml?: () => void; // Parent handles XML export
        onexportdict?: () => void; // Parent handles dictionary export
        onexporthtml?: () => void; // Parent handles HTML export
        onexportpdf?: () => void; // Parent handles PDF export
        ontemplates?: () => void; // Open the template manager
        onmetadata?: () => void; // Open the metadata editor
        onvalidate?: () => void; // Open the validation panel
        onundo?: () => void; // Undo lemmatization
        onredo?: () => void; // Redo lemmatization
        onlogs?: () => void; // Open the log
        onsettings?: () => void; // Open settings dialog
        onhelp?: () => void; // Open help dialog
    } = $props();

    // Ghost buttons take the toolbar's colours: daisyUI's own are meant for base backgrounds
    const ghost =
        "btn btn-ghost btn-sm xl:btn-md text-neutral-content hover:text-neutral-content hover:bg-neutral-content/15 disabled:text-neutral-content/40 disabled:bg-transparent";

    let exportMenu: HTMLDetailsElement;
    let exportOpen = $state(false);

    /** Closes the menu before the export opens its dialog. */
    function runExport(action?: () => void) {
        exportOpen = false;
        action?.();
    }

    function closeExportOnOutsideClick(event: MouseEvent) {
        if (exportOpen && !exportMenu.contains(event.target as Node)) {
            exportOpen = false;
        }
    }

    function closeExportOnEscape(event: KeyboardEvent) {
        if (exportOpen && event.key === "Escape") {
            exportOpen = false;
            exportMenu.querySelector("summary")?.focus();
        }
    }

    async function handleTemplateChange(e: Event) {
        const select = e.target as HTMLSelectElement;
        const templates = $templateStore.templates;
        const template = templates.find((t) => t.id === select.value);
        if (template) {
            templateStore.setActive(template);
            settings.update({ activeTemplateId: template.id });
        }
    }
</script>

<svelte:window onclick={closeExportOnOutsideClick} onkeydown={closeExportOnEscape} />

<!--
    Groups, left to right: file, document, lemmas, then (at the right) application.
    See "Toolbar and headers" in docs/UI_STANDARDS.md. A narrow window shows icons
    without their labels (which stay as accessible names), then wraps whole groups.
-->
<div
    class="navbar bg-neutral text-neutral-content px-4 min-h-12 flex-wrap gap-x-4 gap-y-1"
>
    <div class="flex items-center gap-2" role="group" aria-label="File">
        <button
            class="btn btn-primary hover:btn-secondary text-primary-content hover:text-secondary-content btn-sm xl:btn-md"
            onclick={onopen}
            title="Open project (Ctrl+O)"
        >
            <OpenIcon size="18" aria-hidden="true" /><span class="sr-only lg:not-sr-only">Open</span>
        </button>
        <button
            class="btn btn-primary hover:btn-secondary text-primary-content hover:text-secondary-content btn-sm xl:btn-md"
            onclick={onimport}
            title="Import from other format (XML/TEI/TXT)"
        >
            <ImportIcon size="18" aria-hidden="true" /><span class="sr-only lg:not-sr-only">Import</span>
        </button>
        <button
            class="btn btn-primary hover:btn-secondary text-primary-content hover:text-secondary-content btn-sm xl:btn-md"
            onclick={onsave}
            title="Save project (Ctrl+S)"
        >
            <SaveIcon size="18" aria-hidden="true" /><span class="sr-only lg:not-sr-only">Save</span>
        </button>
        <!-- Opened and closed explicitly: a focus-based dropdown gets stuck when a
             native file dialog takes focus away and hands it back -->
        <details class="dropdown" bind:this={exportMenu} bind:open={exportOpen}>
            <summary
                class="{ghost} list-none"
                title="Export options"
            >
                Export
                <ChevronDown size="16" aria-hidden="true" />
            </summary>
            <ul class="dropdown-content z-1 menu p-2 shadow bg-base-100 text-base-content rounded-box w-52">
                <li><button onclick={() => runExport(onexportxml)}>TEI-XML</button></li>
                <li><button onclick={() => runExport(onexporthtml)}>HTML</button></li>
                <li><button onclick={() => runExport(onexportpdf)}>PDF</button></li>
                <li class="border-t border-base-300 mt-1 pt-1"><button onclick={() => runExport(onexportdict)}>Dictionary JSON</button></li>
            </ul>
        </details>
    </div>

    <div class="flex items-center gap-1" role="group" aria-label="Document">
        <label for="toolbar-template" class="text-xs xl:text-sm font-bold mr-1 sr-only xl:not-sr-only">
            Template:
        </label>
        <select
            id="toolbar-template"
            class="select select-sm select-bordered bg-neutral-focus text-primary text-xs lg:text-sm font-bold max-w-48"
            onchange={handleTemplateChange}
            value={$templateStore.active?.id ?? ""}
        >
            {#each $templateStore.templates as template}
                <option value={template.id}>{template.name}</option>
            {/each}
        </select>
        <button
            class="{ghost} btn-square"
            onclick={ontemplates}
            title="Manage templates"
            aria-label="Manage templates"
        >
            <BookDashed size="18" aria-hidden="true" />
        </button>
        <button
            class={ghost}
            onclick={onmetadata}
            title="Edit manuscript metadata"
        >
            <FileText size="18" aria-hidden="true" /><span class="sr-only 2xl:not-sr-only">Metadata</span>
        </button>
        <button
            class={ghost}
            onclick={onvalidate}
            title="Validate XML"
        >
            <FileCheck size="18" aria-hidden="true" /><span class="sr-only 2xl:not-sr-only">Validate</span>
        </button>
    </div>

    <div class="flex items-center gap-1" role="group" aria-label="Lemmatization">
        <button
            class={ghost}
            onclick={onundo}
            disabled={!$canUndo}
            title="Undo lemmatization (Ctrl+Shift+Z)"
        >
            <UndoIcon size="18" aria-hidden="true" /><span class="sr-only xl:not-sr-only">Lemma Undo</span>
        </button>
        <button
            class={ghost}
            onclick={onredo}
            disabled={!$canRedo}
            title="Redo lemmatization (Ctrl+Shift+Y)"
        >
            <RedoIcon size="18" aria-hidden="true" /><span class="sr-only xl:not-sr-only">Lemma Redo</span>
        </button>
    </div>

    <div class="ml-auto flex items-center gap-1 min-w-0" role="group" aria-label="Application">
        <button
            class="{ghost} btn-circle"
            onclick={onlogs}
            title="View logs"
            aria-label={$errorCounts.error > 0 ? "View logs (contains errors)" : "View logs"}
        >
            {#if $errorCounts.error > 0}
                <MessageCircleWarning size="18" aria-hidden="true" />
            {:else}
                <ScrollText size="18" aria-hidden="true" />
            {/if}
        </button>
        <button
            class="{ghost} btn-circle"
            onclick={onhelp}
            title="Help (keyboard shortcuts, DSL reference)"
            aria-label="Open help"
        >
            <HelpIcon size="18" aria-hidden="true" />
        </button>
        <button
            class="{ghost} btn-circle"
            onclick={onsettings}
            title="Settings"
            aria-label="Open settings"
        >
            <SettingsIcon size="18" aria-hidden="true" />
        </button>
        <span class="ml-2 text-sm xl:text-md truncate max-w-48 xl:max-w-80" title={$fileName}>
            {$fileName}
        </span>
        {#if $editor.isDirty}
            <span class="font-bold" title="Unsaved changes">*<span class="sr-only">(unsaved changes)</span></span>
        {/if}
    </div>
</div>
