<script lang="ts">
    import { templateStore, type Template } from '$lib/stores/template';
    import { saveTemplate } from '$lib/tauri';

    let { onclose }: { onclose?: () => void } = $props();

    let editingTemplate: Template | null = $state(null);
    let isCreating = $state(false);

    function handleEdit(template: Template) {
        editingTemplate = { ...template };
        isCreating = false;
    }

    function handleCreate() {
        editingTemplate = {
            id: '',
            name: '',
            description: '',
            header: '',
            footer: '',
            wordWrap: false,
            autoLineNumbers: false,
            multiLevel: false,
        };
        isCreating = true;
    }

    async function handleSave() {
        if (!editingTemplate) return;

        if (isCreating && !editingTemplate.id) {
            editingTemplate.id = editingTemplate.name
                .toLowerCase()
                .replace(/\s+/g, '-')
                .replace(/[^a-z0-9-]/g, '');
        }

        await saveTemplate(editingTemplate);
        editingTemplate = null;
    }

    function handleCancel() {
        editingTemplate = null;
    }
</script>

<div class="template-manager">
    <div class="header">
        <h2>Template Manager</h2>
        <button class="close-btn" onclick={onclose}>×</button>
    </div>

    {#if editingTemplate}
        <div class="edit-form">
            <label>
                Name
                <input type="text" bind:value={editingTemplate.name} />
            </label>
            <label>
                Description
                <input type="text" bind:value={editingTemplate.description} />
            </label>
            <label>
                Header (TEI-XML before content)
                <textarea rows="8" bind:value={editingTemplate.header}></textarea>
            </label>
            <label>
                Footer (TEI-XML after content)
                <textarea rows="4" bind:value={editingTemplate.footer}></textarea>
            </label>
            <label class="checkbox-label">
                <input type="checkbox" bind:checked={editingTemplate.wordWrap} />
                Enable word wrapping (&lt;w&gt; tags)
            </label>
            <label class="checkbox-label">
                <input type="checkbox" bind:checked={editingTemplate.autoLineNumbers} />
                Automatic line numbering (&lt;lb n="..."&gt;)
            </label>
            <label class="checkbox-label">
                <input type="checkbox" bind:checked={editingTemplate.multiLevel} />
                Multi-level output (MENOTA me:facs/me:dipl/me:norm)
            </label>
            <div class="form-actions">
                <button onclick={handleCancel}>Cancel</button>
                <button class="primary" onclick={handleSave}>Save</button>
            </div>
        </div>
    {:else}
        <div class="template-list">
            {#each $templateStore.templates as template}
                <div class="template-item">
                    <div class="template-info">
                        <strong>{template.name}</strong>
                        <span>{template.description}</span>
                    </div>
                    <button onclick={() => handleEdit(template)}>Edit</button>
                </div>
            {/each}
        </div>
        <button class="create-btn" onclick={handleCreate}>+ Create Template</button>
    {/if}
</div>

<style>
    .template-manager {
        padding: 1rem;
        background: white;
        border-radius: 8px;
        max-width: 600px;
        max-height: 80vh;
        overflow-y: auto;
    }

    .header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 1rem;
    }

    h2 {
        margin: 0;
        font-size: 1.25rem;
    }

    .close-btn {
        background: none;
        border: none;
        font-size: 1.5rem;
        cursor: pointer;
        color: #666;
    }

    .template-list {
        display: flex;
        flex-direction: column;
        gap: 0.5rem;
    }

    .template-item {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.75rem;
        background: #f6f8fa;
        border-radius: 4px;
    }

    .template-info {
        display: flex;
        flex-direction: column;
        gap: 0.25rem;
    }

    .template-info span {
        font-size: 0.875rem;
        color: #666;
    }

    .edit-form {
        display: flex;
        flex-direction: column;
        gap: 1rem;
    }

    label {
        display: flex;
        flex-direction: column;
        gap: 0.25rem;
        font-weight: 500;
    }

    .checkbox-label {
        flex-direction: row;
        align-items: center;
        gap: 0.5rem;
        font-weight: normal;
    }

    .checkbox-label input {
        width: auto;
    }

    input,
    textarea {
        padding: 0.5rem;
        border: 1px solid #d1d5da;
        border-radius: 4px;
        font-family: inherit;
    }

    textarea {
        font-family: monospace;
        font-size: 0.875rem;
    }

    .form-actions {
        display: flex;
        justify-content: flex-end;
        gap: 0.5rem;
    }

    button {
        padding: 0.5rem 1rem;
        border: 1px solid #d1d5da;
        border-radius: 4px;
        background: white;
        cursor: pointer;
    }

    button.primary {
        background: #2ea44f;
        color: white;
        border-color: #2ea44f;
    }

    .create-btn {
        margin-top: 1rem;
        width: 100%;
    }
</style>
