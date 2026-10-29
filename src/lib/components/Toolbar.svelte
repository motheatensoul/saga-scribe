<script lang="ts">
    import { editor, fileName } from "$lib/stores/editor";
    import { templateStore } from "$lib/stores/template";
    import { settings } from "$lib/stores/settings";
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
    } from "@lucide/svelte";

    let {
        onopen,
        onimport,
        onsave,
        onexportxml,
        onexportdict,
        onexporthtml,
        onexportpdf,
        onundo,
        onredo,
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
        onundo?: () => void; // Undo lemmatization
        onredo?: () => void; // Redo lemmatization
        onsettings?: () => void; // Open settings dialog
        onhelp?: () => void; // Open help dialog
    } = $props();

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

<div class="navbar bg-neutral text-neutral-content px-4 min-h-12">
    <div class="flex gap-2">
        <button
            class="btn btn-primary hover:btn-secondary text-primary-content hover:text-seconary-content btn-sm xl:btn-md"
            onclick={onopen}
            title="Open project (Ctrl+O)"><OpenIcon size="18" />Open</button
        >
        <button
            class="btn btn-primary hover:btn-secondary text-primary-content hover:text-seconary-content btn-sm xl:btn-md"
            onclick={onimport}
            title="Import from other format (XML/TEI/TXT)"><ImportIcon size="18" />Import</button
        >
        <button
            class="btn btn-primary hover:btn-secondary text-primary-content hover:text-seconary-content btn-sm xl:btn-md"
            onclick={onsave}
            title="Save project (Ctrl+S)"><SaveIcon size="18" />Save</button
        >
        <!-- Opened and closed explicitly: a focus-based dropdown gets stuck when a
             native file dialog takes focus away and hands it back -->
        <details class="dropdown dropdown-end" bind:this={exportMenu} bind:open={exportOpen}>
            <summary
                class="btn btn-ghost btn-sm xl:btn-md list-none"
                title="Export options"
            >
                Export
                <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
                    <path fill-rule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clip-rule="evenodd" />
                </svg>
            </summary>
            <ul class="dropdown-content z-1 menu p-2 shadow bg-base-100 text-base-content rounded-box w-52">
                <li><button onclick={() => runExport(onexportxml)}>TEI-XML</button></li>
                <li><button onclick={() => runExport(onexporthtml)}>HTML</button></li>
                <li><button onclick={() => runExport(onexportpdf)}>PDF</button></li>
                <li class="border-t border-base-300 mt-1 pt-1"><button onclick={() => runExport(onexportdict)}>Dictionary JSON</button></li>
            </ul>
        </details>
    </div>

    <div class="flex items-center gap-2 ml-4">
        <span class="text-xs xl:text-sm font-bold">Template:</span>
        <select
            class="select select-sm select-bordered bg-neutral-focus text-primary text-xs lg:text-sm font-bold"
            onchange={handleTemplateChange}
            value={$templateStore.active?.id ?? ""}
        >
            {#each $templateStore.templates as template}
                <option value={template.id}>{template.name}</option>
            {/each}
        </select>
    </div>

    <div class="ml-auto flex items-center gap-2">
        <button
            class="btn btn-ghost btn-sm xl:btn-md"
            onclick={onundo}
            disabled={!$canUndo}
            title="Undo lemmatization (Ctrl+Shift+Z)"
        >
            <UndoIcon size="18" />Lemma Undo
        </button>
        <button
            class="btn btn-ghost btn-sm xl:btn-md"
            onclick={onredo}
            disabled={!$canRedo}
            title="Redo lemmatization (Ctrl+Shift+Y)"
        >
            <RedoIcon size="18" />Lemma Redo
        </button>
        <div class="divider divider-horizontal mx-0"></div>
        <button
            class="btn btn-ghost btn-sm btn-circle xl:btn-md"
            onclick={onhelp}
            title="Help (keyboard shortcuts, DSL reference)"
            aria-label="Open help"
        >
            <HelpIcon size="18" />
        </button>
        <button
            class="btn btn-ghost btn-sm btn-circle xl:btn-md"
            onclick={onsettings}
            title="Settings"
            aria-label="Open settings"
        >
            <SettingsIcon size="18" />
        </button>
        <span class="text-sm opacity-70 xl:text-md">{$fileName}</span>
        {#if $editor.isDirty}
            <span class="text-warning font-bold">*</span>
        {/if}
    </div>
</div>
