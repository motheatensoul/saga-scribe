<script lang="ts">
    import { open, save } from "@tauri-apps/plugin-dialog";
    import { editor, fileName } from "$lib/stores/editor";
    import { templateStore } from "$lib/stores/template";
    import { settings } from "$lib/stores/settings";
    import { canUndo, canRedo } from "$lib/stores/lemmatizationHistory";

    let {
        onopen,
        onsave,
        onexportxml,
        onexportdict,
        onundo,
        onredo,
        onsettings,
        onhelp,
    }: {
        onopen?: () => void; // Parent handles opening (shows dialog, loads project)
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
            class="btn btn-primary btn-sm"
            onclick={onopen}
            title="Open project (Ctrl+O)">Open</button
        >
        <button
            class="btn btn-primary btn-sm"
            onclick={onsave}
            title="Save project (Ctrl+S)">Save</button
        >
        <button
            class="btn btn-ghost btn-sm"
            onclick={onexportxml}
            title="Export TEI-XML to separate file">Export XML</button
        >
        <button
            class="btn btn-ghost btn-sm"
            onclick={onexportdict}
            title="Export inflection dictionary to JSON">Export Dict</button
        >
    </div>

    <div class="flex items-center gap-2 ml-4">
        <span class="text-sm">Template:</span>
        <select
            class="select select-sm select-bordered bg-neutral-focus"
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
            class="btn btn-ghost btn-sm"
            onclick={onundo}
            disabled={!$canUndo}
            title="Undo lemmatization (Ctrl+Shift+Z)"
        >
            Undo
        </button>
        <button
            class="btn btn-ghost btn-sm"
            onclick={onredo}
            disabled={!$canRedo}
            title="Redo lemmatization (Ctrl+Shift+Y)"
        >
            Redo
        </button>
        <div class="divider divider-horizontal mx-0"></div>
        <button
            class="btn btn-ghost btn-sm btn-circle"
            onclick={onhelp}
            title="Help (keyboard shortcuts, DSL reference)"
            aria-label="Open help"
        >
            ?
        </button>
        <button
            class="btn btn-ghost btn-sm btn-circle"
            onclick={onsettings}
            title="Settings"
            aria-label="Open settings"
        >
            ⚙️
        </button>
        <span class="text-sm opacity-70">{$fileName}</span>
        {#if $editor.isDirty}
            <span class="text-warning font-bold">*</span>
        {/if}
    </div>
</div>
