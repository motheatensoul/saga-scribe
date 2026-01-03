<script lang="ts">
    import { open, save } from '@tauri-apps/plugin-dialog';
    import { editor, fileName } from '$lib/stores/editor';
    import { templateStore } from '$lib/stores/template';
    import { openFile, saveFile, exportTei, listTemplates, compileDsl } from '$lib/tauri';

    let {
        onopen,
        onsave,
        onexport,
    }: {
        onopen?: (content: string) => void;
        onsave?: () => void;
        onexport?: () => void;
    } = $props();

    async function handleOpen() {
        const path = await open({
            filters: [{ name: 'TEI Scribe', extensions: ['teis', 'txt'] }],
        });
        if (path) {
            const file = await openFile(path as string);
            editor.setFile(file.path, file.content);
            onopen?.(file.content);
        }
    }

    async function handleSave() {
        let path = $editor.filePath;
        if (!path) {
            path = await save({
                filters: [{ name: 'TEI Scribe', extensions: ['teis'] }],
            });
        }
        if (path) {
            await saveFile(path, $editor.content);
            editor.setFile(path, $editor.content);
            onsave?.();
        }
    }

    async function handleExport() {
        const template = $templateStore.active;
        if (!template) {
            alert('Please select a template first');
            return;
        }

        const path = await save({
            filters: [{ name: 'TEI-XML', extensions: ['xml'] }],
        });

        if (path) {
            const teiContent = await compileDsl(
                $editor.content,
                template.header,
                template.footer
            );
            await exportTei(path, teiContent);
            onexport?.();
        }
    }

    async function handleTemplateChange(e: Event) {
        const select = e.target as HTMLSelectElement;
        const templates = $templateStore.templates;
        const template = templates.find((t) => t.id === select.value);
        if (template) {
            templateStore.setActive(template);
        }
    }
</script>

<div class="toolbar">
    <div class="toolbar-group">
        <button onclick={handleOpen} title="Open file (Ctrl+O)">Open</button>
        <button onclick={handleSave} title="Save file (Ctrl+S)">Save</button>
        <button onclick={handleExport} title="Export to TEI-XML">Export</button>
    </div>

    <div class="toolbar-group">
        <label>
            Template:
            <select onchange={handleTemplateChange}>
                {#each $templateStore.templates as template}
                    <option value={template.id}>{template.name}</option>
                {/each}
            </select>
        </label>
    </div>

    <div class="toolbar-file">
        <span class="filename">{$fileName}</span>
        {#if $editor.isDirty}
            <span class="dirty-indicator">*</span>
        {/if}
    </div>
</div>

<style>
    .toolbar {
        display: flex;
        align-items: center;
        gap: 1rem;
        padding: 0.5rem 1rem;
        background: #24292e;
        color: white;
        border-bottom: 1px solid #1b1f23;
    }

    .toolbar-group {
        display: flex;
        align-items: center;
        gap: 0.5rem;
    }

    button {
        padding: 0.375rem 0.75rem;
        background: #2ea44f;
        color: white;
        border: none;
        border-radius: 4px;
        cursor: pointer;
        font-size: 0.875rem;
    }

    button:hover {
        background: #22863a;
    }

    label {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        font-size: 0.875rem;
    }

    select {
        padding: 0.25rem 0.5rem;
        border-radius: 4px;
        border: 1px solid #444d56;
        background: #2f363d;
        color: white;
    }

    .toolbar-file {
        margin-left: auto;
        display: flex;
        align-items: center;
        gap: 0.25rem;
    }

    .filename {
        font-size: 0.875rem;
        color: #959da5;
    }

    .dirty-indicator {
        color: #f9826c;
        font-weight: bold;
    }
</style>
