<script lang="ts">
    import { onMount } from 'svelte';
    import { Splitpanes, Pane } from 'svelte-splitpanes';
    import Editor from '$lib/components/Editor.svelte';
    import Preview from '$lib/components/Preview.svelte';
    import Toolbar from '$lib/components/Toolbar.svelte';
    import TemplateManager from '$lib/components/TemplateManager.svelte';
    import EntityBrowser from '$lib/components/EntityBrowser.svelte';
    import ErrorPanel from '$lib/components/ErrorPanel.svelte';
    import { editor } from '$lib/stores/editor';
    import { templateStore } from '$lib/stores/template';
    import { entityStore } from '$lib/stores/entities';
    import { settings } from '$lib/stores/settings';
    import { errorStore, errorCounts } from '$lib/stores/errors';
    import { listTemplates, compileDsl, loadEntities, loadTextFile, loadCustomMappings } from '$lib/tauri';
    import { resolveResource, appDataDir } from '@tauri-apps/api/path';

    let editorComponent: Editor;
    let previewContent = $state('');
    let showTemplateManager = $state(false);
    let showEntityBrowser = $state(false);
    let showErrorPanel = $state(false);
    let compileTimeout: ReturnType<typeof setTimeout>;
    let entitiesJson = $state<string | null>(null);
    let normalizerJson = $state<string | null>(null);

    onMount(async () => {
        errorStore.info('App', 'Application starting...');

        // Load settings first
        try {
            errorStore.info('Settings', 'Loading settings...');
            await settings.load();
            errorStore.info('Settings', 'Settings loaded');
        } catch (e) {
            errorStore.error('Settings', 'Failed to load settings', String(e));
        }

        // Load templates
        try {
            errorStore.info('Templates', 'Loading templates...');
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

            errorStore.info('Templates', `Loaded ${templates.length} templates`);
        } catch (e) {
            errorStore.error('Templates', 'Failed to load templates', String(e));
        }

        // Load default MENOTA entities
        // Try resource path (production), then derive static folder from resource path (development)
        const resourcePath = await resolveResource('entities/menota.json');

        // For development, the resource path is like: .../src-tauri/target/debug/entities/menota.json
        // We need: .../static/entities/menota.json
        const devPath = resourcePath.replace(
            /src-tauri\/target\/[^/]+\/entities\/menota\.json$/,
            'static/entities/menota.json'
        );

        const entityPaths = [resourcePath, devPath];

        let entities = null;
        let loadedFrom = '';

        for (const path of entityPaths) {
            try {
                errorStore.info('Entities', `Trying to load from: ${path}`);
                entities = await loadEntities(path);
                loadedFrom = path;
                break;
            } catch (e) {
                errorStore.warning('Entities', `Failed to load from ${path}`, String(e));
            }
        }

        if (entities) {
            const entityCount = Object.keys(entities).length;
            errorStore.info('Entities', `Loaded ${entityCount} entities from ${loadedFrom}`);
            entityStore.setEntities(entities);
            entitiesJson = JSON.stringify({ version: '1.0', name: 'MENOTA', entities });

            // Load base entity mappings (diplomatic normalization defaults)
            const baseMappingsResourcePath = await resolveResource('normalizer/entity-base-letters.json');
            const baseMappingsDevPath = baseMappingsResourcePath.replace(
                /src-tauri\/target\/[^/]+\/normalizer\/entity-base-letters\.json$/,
                'static/normalizer/entity-base-letters.json'
            );
            const baseMappingsPaths = [baseMappingsResourcePath, baseMappingsDevPath];

            for (const path of baseMappingsPaths) {
                try {
                    errorStore.info('Entities', `Trying to load base mappings from: ${path}`);
                    const baseMappingsText = await loadTextFile(path);
                    const baseMappingsData = JSON.parse(baseMappingsText);
                    const mappings = baseMappingsData.mappings || {};
                    const mappingsCount = Object.keys(mappings).length;
                    errorStore.info('Entities', `Loaded ${mappingsCount} base entity mappings from ${path}`);
                    entityStore.setBaseMappings(mappings);
                    break;
                } catch (e) {
                    errorStore.warning('Entities', `Failed to load base mappings from ${path}`, String(e));
                }
            }

            // Load custom entity mappings (user overrides)
            try {
                const customMappings = await loadCustomMappings();
                const customCount = Object.keys(customMappings).length;
                if (customCount > 0) {
                    errorStore.info('Entities', `Loaded ${customCount} custom entity mappings`);
                    entityStore.setCustomMappings(customMappings);
                }
            } catch (e) {
                errorStore.warning('Entities', 'Failed to load custom mappings', String(e));
            }
        } else {
            errorStore.error('Entities', 'Failed to load entities from any path');
            entityStore.setError('Could not find entity definitions file');
        }

        // Load normalizer dictionary for multi-level transcription
        const normalizerResourcePath = await resolveResource('normalizer/menota-levels.json');
        const normalizerDevPath = normalizerResourcePath.replace(
            /src-tauri\/target\/[^/]+\/normalizer\/menota-levels\.json$/,
            'static/normalizer/menota-levels.json'
        );
        const normalizerPaths = [normalizerResourcePath, normalizerDevPath];

        for (const path of normalizerPaths) {
            try {
                errorStore.info('Normalizer', `Trying to load from: ${path}`);
                normalizerJson = await loadTextFile(path);
                errorStore.info('Normalizer', `Loaded normalizer dictionary from ${path}`);
                break;
            } catch (e) {
                errorStore.warning('Normalizer', `Failed to load from ${path}`, String(e));
            }
        }

        if (!normalizerJson) {
            errorStore.warning('Normalizer', 'Could not load normalizer dictionary - multi-level output may not work correctly');
        }

        errorStore.info('App', 'Application ready');
    });

    async function updatePreview(content: string) {
        if (!$settings.autoPreview) return;

        clearTimeout(compileTimeout);
        compileTimeout = setTimeout(async () => {
            const template = $templateStore.active;
            if (template) {
                try {
                    previewContent = await compileDsl(
                        content,
                        template.header,
                        template.footer,
                        {
                            wordWrap: template.wordWrap,
                            autoLineNumbers: template.autoLineNumbers,
                            multiLevel: template.multiLevel,
                            entitiesJson: entitiesJson ?? undefined,
                            normalizerJson: normalizerJson ?? undefined,
                        }
                    );
                } catch (e) {
                    previewContent = `Error: ${e}`;
                }
            }
        }, $settings.previewDelay);
    }

    function handleOpen(content: string) {
        editorComponent?.setContent(content);
        updatePreview(content);
    }

    function handleEditorChange(content: string) {
        updatePreview(content);
    }

    function handleEntityInsert(text: string) {
        editorComponent?.insertText(text);
        showEntityBrowser = false;
    }
</script>

<div class="flex flex-col h-screen overflow-hidden bg-base-100">
    <Toolbar onopen={handleOpen} />

    <div class="flex-1 overflow-hidden">
        <Splitpanes theme="themed">
            <Pane minSize={20}>
                <div class="flex flex-col h-full">
                    <div class="flex justify-between items-center px-4 py-2 bg-base-200 border-b border-base-300 font-medium text-sm">
                        <span>DSL Editor</span>
                        <div class="flex gap-1">
                            <button
                                class="btn btn-ghost btn-xs"
                                title="Insert entity"
                                onclick={() => (showEntityBrowser = true)}
                            >
                                ꝥ
                            </button>
                            <button
                                class="btn btn-ghost btn-xs"
                                title="Manage templates"
                                onclick={() => (showTemplateManager = true)}
                            >
                                ⚙
                            </button>
                            <button
                                class="btn btn-ghost btn-xs"
                                class:text-error={$errorCounts.error > 0}
                                title="View logs"
                                onclick={() => (showErrorPanel = true)}
                            >
                                {#if $errorCounts.error > 0}
                                    ✕
                                {:else}
                                    ☰
                                {/if}
                            </button>
                        </div>
                    </div>
                    <Editor bind:this={editorComponent} onchange={handleEditorChange} />
                </div>
            </Pane>
            <Pane minSize={20}>
                <Preview content={previewContent} />
            </Pane>
        </Splitpanes>
    </div>

    {#if showTemplateManager}
        <div class="modal modal-open">
            <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
            <div class="modal-backdrop" onclick={() => (showTemplateManager = false)}></div>
            <div class="modal-box">
                <TemplateManager onclose={() => (showTemplateManager = false)} />
            </div>
        </div>
    {/if}

    {#if showEntityBrowser}
        <div class="modal modal-open">
            <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
            <div class="modal-backdrop" onclick={() => (showEntityBrowser = false)}></div>
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
            <div class="modal-backdrop" onclick={() => (showErrorPanel = false)}></div>
            <div class="modal-box max-w-3xl">
                <ErrorPanel onclose={() => (showErrorPanel = false)} />
            </div>
        </div>
    {/if}
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
