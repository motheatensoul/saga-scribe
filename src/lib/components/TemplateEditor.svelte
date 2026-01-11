<script lang="ts">
    import { templateStore, type Template } from "$lib/stores/template";
    import { listTemplates, saveTemplate, deleteTemplate } from "$lib/tauri";
    import { errorStore } from "$lib/stores/errors";

    //Icons
    import { BookCopy, PencilRuler, X as CloseButton } from "@lucide/svelte";

    let {
        isopen = $bindable(false),
    }: {
        isopen?: boolean;
    } = $props();

    type ViewMode = "list" | "edit" | "create";
    let viewMode = $state<ViewMode>("list");
    let editingTemplate = $state<Template | null>(null);
    let originalId = $state<string>("");
    let isSaving = $state(false);
    let isDeleting = $state(false);
    let validationError = $state<string | null>(null);

    // Built-in template IDs that cannot be edited or deleted
    const BUILTIN_IDS = ["tei-p5", "menota"];

    function isBuiltin(id: string): boolean {
        return BUILTIN_IDS.includes(id);
    }

    function handleClose() {
        if (viewMode !== "list") {
            // Confirm if there are unsaved changes
            viewMode = "list";
            editingTemplate = null;
            validationError = null;
        } else {
            isopen = false;
        }
    }

    function handleBackdropClick(e: MouseEvent) {
        if (e.target === e.currentTarget) {
            handleClose();
        }
    }

    function handleKeydown(e: KeyboardEvent) {
        if (e.key === "Escape") {
            handleClose();
        }
    }

    function handleCreate() {
        editingTemplate = {
            id: "",
            name: "",
            description: "",
            header: `<?xml version="1.0" encoding="UTF-8"?>
<TEI xmlns="http://www.tei-c.org/ns/1.0">
  <teiHeader>
    <fileDesc>
      <titleStmt>
        <title><!-- Title --></title>
      </titleStmt>
      <publicationStmt>
        <p><!-- Publication info --></p>
      </publicationStmt>
      <sourceDesc>
        <p><!-- Source description --></p>
      </sourceDesc>
    </fileDesc>
  </teiHeader>
  <text>
    <body>`,
            footer: `    </body>
  </text>
</TEI>`,
            wordWrap: false,
            autoLineNumbers: false,
            multiLevel: false,
            wrapPages: false,
        };
        originalId = "";
        viewMode = "create";
        validationError = null;
    }

    function handleEdit(template: Template) {
        if (isBuiltin(template.id)) {
            // Clone built-in template for customization
            editingTemplate = {
                ...template,
                id: `${template.id}-custom`,
                name: `${template.name} (Custom)`,
                description: `Customized version of ${template.name}`,
            };
            originalId = "";
            viewMode = "create";
        } else {
            editingTemplate = { ...template };
            originalId = template.id;
            viewMode = "edit";
        }
        validationError = null;
    }

    function handleDuplicate(template: Template) {
        editingTemplate = {
            ...template,
            id: `${template.id}-copy`,
            name: `${template.name} (Copy)`,
            description: template.description,
        };
        originalId = "";
        viewMode = "create";
        validationError = null;
    }

    function validateTemplate(): boolean {
        if (!editingTemplate) return false;

        if (!editingTemplate.name.trim()) {
            validationError = "Template name is required";
            return false;
        }

        if (viewMode === "create") {
            // Generate ID from name if empty
            if (!editingTemplate.id) {
                editingTemplate.id = editingTemplate.name
                    .toLowerCase()
                    .replace(/\s+/g, "-")
                    .replace(/[^a-z0-9-]/g, "")
                    .substring(0, 50);
            }

            // Check for duplicate ID
            if (isBuiltin(editingTemplate.id)) {
                validationError =
                    "Cannot use a built-in template ID. Please choose a different name.";
                return false;
            }

            const existingIds = $templateStore.templates
                .filter((t) => !isBuiltin(t.id))
                .map((t) => t.id);
            if (existingIds.includes(editingTemplate.id)) {
                validationError =
                    "A template with this ID already exists. Please choose a different name.";
                return false;
            }
        }

        if (!editingTemplate.header.trim()) {
            validationError = "Header XML is required";
            return false;
        }

        if (!editingTemplate.footer.trim()) {
            validationError = "Footer XML is required";
            return false;
        }

        validationError = null;
        return true;
    }

    async function handleSave() {
        if (!editingTemplate || !validateTemplate()) return;

        isSaving = true;
        try {
            await saveTemplate(editingTemplate);

            // Refresh template list
            const templates = await listTemplates();
            templateStore.setTemplates(templates);

            errorStore.info(
                "Template",
                `Template "${editingTemplate.name}" saved successfully`,
            );

            viewMode = "list";
            editingTemplate = null;
        } catch (e) {
            validationError = `Failed to save template: ${e}`;
            errorStore.error("Template", `Failed to save: ${e}`);
        } finally {
            isSaving = false;
        }
    }

    async function handleDelete(template: Template) {
        if (isBuiltin(template.id)) return;

        if (
            !confirm(
                `Are you sure you want to delete "${template.name}"? This cannot be undone.`,
            )
        ) {
            return;
        }

        isDeleting = true;
        try {
            await deleteTemplate(template.id);

            // Refresh template list
            const templates = await listTemplates();
            templateStore.setTemplates(templates);

            // If the deleted template was active, switch to first available
            if ($templateStore.active?.id === template.id) {
                if (templates.length > 0) {
                    templateStore.setActive(templates[0]);
                }
            }

            errorStore.info("Template", `Template "${template.name}" deleted`);
        } catch (e) {
            errorStore.error("Template", `Failed to delete: ${e}`);
        } finally {
            isDeleting = false;
        }
    }

    function handleCancel() {
        viewMode = "list";
        editingTemplate = null;
        validationError = null;
    }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if isopen}
    <!-- Only handles dismissing the dialog by clicking outside of it, no a11y relevance -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
        onclick={handleBackdropClick}
        role="dialog"
        aria-modal="true"
        aria-labelledby="template-editor-title"
        tabindex="-1"
    >
        <div
            class="bg-base-100 rounded-lg shadow-xl w-full max-w-4xl mx-4 max-h-[90vh] flex flex-col"
        >
            <!-- Header -->
            <div
                class="flex items-center justify-between p-4 border-b border-base-300"
            >
                <h2 id="template-editor-title" class="text-xl font-bold">
                    {#if viewMode === "list"}
                        Template Manager
                    {:else if viewMode === "create"}
                        Create Template
                    {:else}
                        Edit Template
                    {/if}
                </h2>
                <button
                    class="btn btn-ghost btn-sm btn-circle"
                    onclick={handleClose}
                    aria-label="Close"
                >
                    <CloseButton size="18" strokeWidth="3" />
                </button>
            </div>

            <!-- Content -->
            <div class="p-4 overflow-y-auto flex-1">
                {#if viewMode === "list"}
                    <!-- Template List -->
                    <div class="space-y-3">
                        {#each $templateStore.templates as template}
                            <div
                                class="flex items-center justify-between p-4 bg-base-200 rounded-lg gap-4"
                            >
                                <div class="flex-1 min-w-0">
                                    <div class="flex items-center gap-2">
                                        <strong class="text-base">
                                            {template.name}
                                        </strong>
                                        {#if isBuiltin(template.id)}
                                            <span
                                                class="badge badge-sm badge-outline"
                                                >Built-in</span
                                            >
                                        {/if}
                                        {#if $templateStore.active?.id === template.id}
                                            <span
                                                class="badge badge-sm badge-primary"
                                                >Active</span
                                            >
                                        {/if}
                                    </div>
                                    <p
                                        class="text-sm text-base-content/70 mt-1"
                                    >
                                        {template.description}
                                    </p>
                                    <div
                                        class="flex flex-wrap gap-2 mt-2 text-xs"
                                    >
                                        {#if template.wordWrap}
                                            <span class="badge badge-xs"
                                                >Word wrap</span
                                            >
                                        {/if}
                                        {#if template.autoLineNumbers}
                                            <span class="badge badge-xs"
                                                >Auto line numbers</span
                                            >
                                        {/if}
                                        {#if template.multiLevel}
                                            <span class="badge badge-xs"
                                                >Multi-level</span
                                            >
                                        {/if}
                                    </div>
                                </div>
                                <div class="flex grow justify-end ">
                                    <button
                                        class="btn btn-ghost btn-sm hover:btn-accent"
                                        onclick={() =>
                                            handleDuplicate(template)}
                                        title="Duplicate template"
                                    >
                                        <BookCopy size="20" />
                                    </button>
                                    <button
                                        class="btn btn-ghost btn-sm hover:btn-accent"
                                        onclick={() => handleEdit(template)}
                                        title={isBuiltin(template.id)
                                            ? "Clone and customize"
                                            : "Edit template"}
                                    >
                                        <PencilRuler size="20" />
                                    </button>
                                    {#if !isBuiltin(template.id)}
                                        <button
                                            class="btn btn-ghost btn-sm text-error"
                                            onclick={() =>
                                                handleDelete(template)}
                                            disabled={isDeleting}
                                            title="Delete template"
                                        >
                                            🗑️
                                        </button>
                                    {/if}
                                </div>
                            </div>
                        {/each}
                    </div>
                {:else if editingTemplate}
                    <!-- Edit/Create Form -->
                    <div class="space-y-4">
                        {#if validationError}
                            <div class="alert alert-error">
                                <span>{validationError}</span>
                            </div>
                        {/if}

                        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                            <!-- Name -->
                            <div class="form-control">
                                <label class="label" for="template-name">
                                    <span class="label-text font-medium"
                                        >Name *</span
                                    >
                                </label>
                                <input
                                    id="template-name"
                                    type="text"
                                    class="input input-bordered"
                                    bind:value={editingTemplate.name}
                                    placeholder="My Custom Template"
                                />
                            </div>

                            <!-- ID (read-only for existing) -->
                            <div class="form-control">
                                <label class="label" for="template-id">
                                    <span class="label-text font-medium"
                                        >ID</span
                                    >
                                    <span class="label-text-alt"
                                        >Auto-generated from name</span
                                    >
                                </label>
                                <input
                                    id="template-id"
                                    type="text"
                                    class="input input-bordered font-mono text-sm"
                                    bind:value={editingTemplate.id}
                                    readonly={viewMode === "edit"}
                                    placeholder="my-custom-template"
                                />
                            </div>
                        </div>

                        <!-- Description -->
                        <div class="form-control">
                            <label class="label" for="template-description">
                                <span class="label-text font-medium"
                                    >Description</span
                                >
                            </label>
                            <input
                                id="template-description"
                                type="text"
                                class="input input-bordered"
                                bind:value={editingTemplate.description}
                                placeholder="A brief description of this template"
                            />
                        </div>

                        <!-- Options -->
                        <div class="form-control">
                            <span class="label">
                                <span class="label-text font-medium"
                                    >Options</span
                                >
                            </span>
                            <div
                                class="flex flex-wrap gap-4 p-3 bg-base-200 rounded-lg"
                            >
                                <label
                                    class="label cursor-pointer gap-2 flex-1 min-w-50"
                                >
                                    <input
                                        type="checkbox"
                                        class="checkbox checkbox-sm checkbox-primary"
                                        bind:checked={editingTemplate.wordWrap}
                                    />
                                    <div class="flex flex-col">
                                        <span class="label-text"
                                            >Word wrapping</span
                                        >
                                        <span
                                            class="label-text-alt text-base-content/60"
                                        >
                                            Wrap words in &lt;w&gt; tags
                                        </span>
                                    </div>
                                </label>
                                <label
                                    class="label cursor-pointer gap-2 flex-1 min-w-50"
                                >
                                    <input
                                        type="checkbox"
                                        class="checkbox checkbox-sm checkbox-primary"
                                        bind:checked={
                                            editingTemplate.autoLineNumbers
                                        }
                                    />
                                    <div class="flex flex-col">
                                        <span class="label-text"
                                            >Auto line numbers</span
                                        >
                                        <span
                                            class="label-text-alt text-base-content/60"
                                        >
                                            Add n="..." to &lt;lb/&gt;
                                        </span>
                                    </div>
                                </label>
                                <label
                                    class="label cursor-pointer gap-2 flex-1 min-w-50"
                                >
                                    <input
                                        type="checkbox"
                                        class="checkbox checkbox-sm checkbox-primary"
                                        bind:checked={
                                            editingTemplate.multiLevel
                                        }
                                    />
                                    <div class="flex flex-col">
                                        <span class="label-text"
                                            >Multi-level (MENOTA)</span
                                        >
                                        <span
                                            class="label-text-alt text-base-content/60"
                                        >
                                            facs/dipl/norm levels
                                        </span>
                                    </div>
                                </label>
                            </div>
                        </div>

                        <!-- Header -->
                        <div class="form-control">
                            <label class="label" for="template-header">
                                <span class="label-text font-medium"
                                    >Header XML *</span
                                >
                                <span class="label-text-alt"
                                    >TEI-XML before transcription content</span
                                >
                            </label>
                            <textarea
                                id="template-header"
                                class="textarea textarea-bordered font-mono text-sm h-48 leading-relaxed"
                                bind:value={editingTemplate.header}
                                placeholder="<?xml version=&quot;1.0&quot;?>..."
                                spellcheck="false"
                            ></textarea>
                        </div>

                        <!-- Footer -->
                        <div class="form-control">
                            <label class="label" for="template-footer">
                                <span class="label-text font-medium"
                                    >Footer XML *</span
                                >
                                <span class="label-text-alt"
                                    >TEI-XML after transcription content</span
                                >
                            </label>
                            <textarea
                                id="template-footer"
                                class="textarea textarea-bordered font-mono text-sm h-24 leading-relaxed"
                                bind:value={editingTemplate.footer}
                                placeholder="</body>...</TEI>"
                                spellcheck="false"
                            ></textarea>
                        </div>

                        <div class="p-3 bg-info/10 rounded-lg text-sm">
                            <strong>Tip:</strong> The header should contain everything
                            up to and including the opening &lt;body&gt; tag. The
                            footer should close all tags opened in the header.
                        </div>
                    </div>
                {/if}
            </div>

            <!-- Footer -->
            <div
                class="flex items-center justify-between p-4 border-t border-base-300"
            >
                {#if viewMode === "list"}
                    <button class="btn btn-outline" onclick={handleCreate}>
                        + New Template
                    </button>
                    <button
                        class="btn btn-ghost"
                        onclick={() => (isopen = false)}
                    >
                        Close
                    </button>
                {:else}
                    <button class="btn btn-ghost" onclick={handleCancel}>
                        Cancel
                    </button>
                    <button
                        class="btn btn-primary"
                        onclick={handleSave}
                        disabled={isSaving}
                    >
                        {#if isSaving}
                            <span class="loading loading-spinner loading-sm"
                            ></span>
                        {/if}
                        {viewMode === "create"
                            ? "Create Template"
                            : "Save Changes"}
                    </button>
                {/if}
            </div>
        </div>
    </div>
{/if}
