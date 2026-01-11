<script lang="ts">
    import { open, save } from "@tauri-apps/plugin-dialog";
    import { editor, fileName } from "$lib/stores/editor";
    import { templateStore } from "$lib/stores/template";
    import { settings } from "$lib/stores/settings";
    import { canUndo, canRedo } from "$lib/stores/lemmatizationHistory";

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
        onundo?: () => void; // Undo lemmatization
        onredo?: () => void; // Redo lemmatization
        onsettings?: () => void; // Open settings dialog
        onhelp?: () => void; // Open help dialog
    } = $props();

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
        <button
            class="btn btn-ghost btn-sm xl:btn-md"
            onclick={onexportxml}
            title="Export TEI-XML to separate file">Export XML</button
        >
        <button
            class="btn btn-ghost btn-sm xl:btn-md"
            onclick={onexportdict}
            title="Export inflection dictionary to JSON">Export Dict</button
        >
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
