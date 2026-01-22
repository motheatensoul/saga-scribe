<script lang="ts">
    import { open } from "@tauri-apps/plugin-dialog";
    import { deleteStylesheet, importStylesheet, listStylesheets } from "$lib/tauri";
    import { settings } from "$lib/stores/settings";
    import { stylesheetStore, DEFAULT_STYLESHEET_ID } from "$lib/stores/stylesheets";
    import { templateStore } from "$lib/stores/template";
    import { errorStore } from "$lib/stores/errors";

    //Icons
    import { X as CloseButton } from "@lucide/svelte";

    let {
        isopen = $bindable(false),
    }: {
        isopen?: boolean;
    } = $props();

    // Local state for editing (only commit on save)
    let localSettings = $state({
        fontSize: $settings.fontSize,
        theme: $settings.theme,
        autoPreview: $settings.autoPreview,
        previewDelay: $settings.previewDelay,
        activeTemplateId: $settings.activeTemplateId,
        activeStylesheetId: $settings.activeStylesheetId,
    });

    // Sync local state when dialog opens
    $effect(() => {
        if (isopen) {
            localSettings = {
                fontSize: $settings.fontSize,
                theme: $settings.theme,
                autoPreview: $settings.autoPreview,
                previewDelay: $settings.previewDelay,
                activeTemplateId: $settings.activeTemplateId,
                activeStylesheetId: $settings.activeStylesheetId,
            };
            refreshStylesheets();
        }
    });

    let activeStylesheet = $derived(
        $stylesheetStore.find(
            (stylesheet) => stylesheet.id === localSettings.activeStylesheetId,
        ),
    );

    async function refreshStylesheets() {
        try {
            const stylesheets = await listStylesheets();
            stylesheetStore.setStylesheets(stylesheets);
        } catch (e) {
            console.error("Failed to load stylesheets:", e);
        }
    }

    async function handleImportStylesheet() {
        const path = await open({
            filters: [{ name: "XSLT Stylesheet", extensions: ["xsl", "xslt"] }],
        });

        if (!path || Array.isArray(path)) {
            return;
        }

        try {
            const stylesheet = await importStylesheet(path);
            await refreshStylesheets();
            localSettings.activeStylesheetId = stylesheet.id;
        } catch (e) {
            errorStore.error("Settings", `Failed to import stylesheet: ${e}`);
        }
    }

    async function handleRemoveStylesheet() {
        const stylesheet = activeStylesheet;
        if (!stylesheet || stylesheet.builtIn) {
            return;
        }

        if (!confirm(`Remove stylesheet "${stylesheet.name}"?`)) {
            return;
        }

        try {
            await deleteStylesheet(stylesheet.id);
            await refreshStylesheets();
            if (localSettings.activeStylesheetId === stylesheet.id) {
                localSettings.activeStylesheetId = DEFAULT_STYLESHEET_ID;
            }
        } catch (e) {
            errorStore.error("Settings", `Failed to remove stylesheet: ${e}`);
        }
    }

    function handleSave() {
        settings.update(localSettings);
        isopen = false;
    }

    function handleCancel() {
        isopen = false;
    }

    function handleReset() {
        if (confirm("Reset all settings to defaults?")) {
            settings.reset();
            isopen = false;
        }
    }

    function handleBackdropClick(e: MouseEvent) {
        if (e.target === e.currentTarget) {
            handleCancel();
        }
    }

    function handleKeydown(e: KeyboardEvent) {
        if (e.key === "Escape") {
            handleCancel();
        }
    }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if isopen}
    <!-- This only handles using clicking outside the modal to dismiss, not relevant interaction mode for a11y -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
        onclick={handleBackdropClick}
        role="dialog"
        aria-modal="true"
        aria-labelledby="settings-title"
        tabindex="-1"
    >
        <div
            class="bg-base-100 rounded-lg shadow-xl w-full max-w-2xl mx-4 max-h-[90vh] flex flex-col"
        >
            <!-- Header -->
            <div
                class="flex items-center justify-between p-6 border-b border-base-300"
            >
                <h2 id="settings-title" class="text-2xl font-bold">Settings</h2>
                <button
                    class="btn btn-ghost btn-sm btn-circle"
                    onclick={handleCancel}
                    aria-label="Close settings"
                >
                    <CloseButton size="16" strokeWidth="3" />
                </button>
            </div>

            <!-- Content -->
            <div class="p-6 overflow-y-auto flex-1">
                <div class="space-y-6">
                    <!-- Appearance Section -->
                    <section>
                        <h3 class="text-lg font-semibold mb-4 text-primary">
                            Appearance
                        </h3>
                        <div class="space-y-4">
                            <!-- Theme -->
                            <div class="form-control">
                                <div class="mb-2">
                                    <span class="label-text font-medium">Theme</span>
                                </div>
                                <select
                                    id="theme-select"
                                    class="select select-bordered w-full max-w-xs"
                                    bind:value={localSettings.theme}
                                >
                                    <option value="system">System</option>
                                    <option value="light">Light</option>
                                    <option value="dark">Dark</option>
                                </select>
                                <div class="mt-1">
                                    <span class="label-text-alt text-base-content/60"
                                        >Choose the color scheme. System follows
                                        your OS theme preference</span
                                    >
                                </div>
                            </div>

                            <!-- Font Size -->
                            <div class="form-control">
                                <div class="flex justify-between items-center mb-2">
                                    <span class="label-text font-medium">Editor Font Size</span>
                                    <span class="badge badge-sm">{localSettings.fontSize}px</span>
                                </div>
                                <input
                                    id="font-size-input"
                                    type="range"
                                    min="10"
                                    max="24"
                                    bind:value={localSettings.fontSize}
                                    class="range range-primary w-full"
                                    step="1"
                                />
                                <div class="flex w-full justify-between text-xs px-2 mt-1">
                                    <span>10px</span>
                                    <span>14px</span>
                                    <span>18px</span>
                                    <span>24px</span>
                                </div>
                            </div>
                        </div>
                    </section>

                    <div class="divider"></div>

                    <!-- Editor Section -->
                    <section>
                        <h3 class="text-lg font-semibold mb-4 text-primary">
                            Editor
                        </h3>
                        <div class="space-y-4">
                            <!-- Auto Preview -->
                            <div class="form-control">
                                <label class="label cursor-pointer justify-start gap-4 p-0">
                                    <input
                                        type="checkbox"
                                        bind:checked={localSettings.autoPreview}
                                        class="checkbox checkbox-primary"
                                    />
                                    <div class="flex flex-col">
                                        <span class="label-text font-medium">Auto-preview</span>
                                        <span class="label-text-alt text-base-content/60">
                                            Automatically update preview as you type
                                        </span>
                                    </div>
                                </label>
                            </div>

                            <!-- Preview Delay -->
                            {#if localSettings.autoPreview}
                                <div class="form-control pl-8">
                                    <div class="flex justify-between items-center mb-2">
                                        <span class="label-text font-medium">Preview Delay</span>
                                        <span class="badge badge-sm">{localSettings.previewDelay}ms</span>
                                    </div>
                                    <input
                                        id="preview-delay-input"
                                        type="range"
                                        min="100"
                                        max="2000"
                                        bind:value={localSettings.previewDelay}
                                        class="range range-sm range-primary w-full"
                                        step="100"
                                    />
                                    <div class="flex w-full justify-between text-xs px-2 mt-1">
                                        <span>100ms</span>
                                        <span>500ms</span>
                                        <span>1000ms</span>
                                        <span>2000ms</span>
                                    </div>
                                    <div class="mt-2 text-xs text-base-content/60">
                                        Time to wait after typing before updating preview
                                    </div>
                                </div>
                            {/if}
                        </div>
                    </section>

                    <div class="divider"></div>

                    <!-- Stylesheets Section -->
                    <section>
                        <h3 class="text-lg font-semibold mb-4 text-primary">
                            Stylesheets
                        </h3>
                        <div class="space-y-4">
                            <div class="form-control">
                                <div class="mb-2">
                                    <label
                                        class="label-text font-medium"
                                        for="stylesheet-select"
                                    >
                                        Active XSLT Stylesheet
                                    </label>
                                </div>
                                <select
                                    id="stylesheet-select"
                                    class="select select-bordered w-full max-w-xs"
                                    bind:value={localSettings.activeStylesheetId}
                                >
                                    {#each $stylesheetStore as stylesheet}
                                        <option value={stylesheet.id}>
                                            {stylesheet.name}
                                        </option>
                                    {/each}
                                </select>
                                <div class="mt-1">
                                    <span class="label-text-alt text-base-content/60"
                                        >Used for XSLT preview and exports</span
                                    >
                                </div>
                            </div>
                            <div class="flex flex-wrap gap-2">
                                <button
                                    type="button"
                                    class="btn btn-ghost btn-sm"
                                    onclick={handleImportStylesheet}
                                >
                                    Import Stylesheet
                                </button>
                                <button
                                    type="button"
                                    class="btn btn-ghost btn-sm text-error"
                                    onclick={handleRemoveStylesheet}
                                    disabled={activeStylesheet?.builtIn ?? true}
                                >
                                    Remove Selected
                                </button>
                            </div>
                        </div>
                    </section>

                    <div class="divider"></div>

                    <!-- Template Section -->
                    <section>
                        <h3 class="text-lg font-semibold mb-4 text-primary">
                            Default Template
                        </h3>
                        <div class="form-control">
                            <div class="mb-2">
                                <span class="label-text font-medium"
                                    >Active Template</span
                                >
                            </div>
                            <select
                                id="template-select"
                                class="select select-bordered w-full max-w-xs"
                                bind:value={localSettings.activeTemplateId}
                            >
                                <option value={null}>None</option>
                                {#each $templateStore.templates as template}
                                    <option value={template.id}
                                        >{template.name}</option
                                    >
                                {/each}
                            </select>
                            <div class="mt-1">
                                <span class="label-text-alt text-base-content/60"
                                    >The template to use for new projects</span
                                >
                            </div>
                        </div>
                    </section>
                </div>
            </div>

            <!-- Footer -->
            <div
                class="flex items-center justify-between p-6 border-t border-base-300"
            >
                <button class="btn btn-ghost" onclick={handleReset}>
                    Reset to Defaults
                </button>
                <div class="flex gap-2">
                    <button class="btn btn-ghost" onclick={handleCancel}>
                        Cancel
                    </button>
                    <button class="btn btn-primary" onclick={handleSave}>
                        Save Changes
                    </button>
                </div>
            </div>
        </div>
    </div>
{/if}

<style>
    /* Ensure dialog is above everything */
    :global(body:has(dialog[open])) {
        overflow: hidden;
    }
</style>
