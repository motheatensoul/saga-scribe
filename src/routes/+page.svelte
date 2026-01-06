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
    import { listTemplates, compileDsl, loadEntities, loadTextFile } from '$lib/tauri';
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

<div class="app">
    <Toolbar onopen={handleOpen} />

    <div class="main-content">
        <Splitpanes>
            <Pane minSize={20}>
                <div class="pane-content">
                    <div class="pane-header">
                        <span>DSL Editor</span>
                        <div class="header-actions">
                            <button
                                class="icon-btn"
                                title="Insert entity"
                                onclick={() => (showEntityBrowser = true)}
                            >
                                ꝥ
                            </button>
                            <button
                                class="icon-btn"
                                title="Manage templates"
                                onclick={() => (showTemplateManager = true)}
                            >
                                ⚙
                            </button>
                            <button
                                class="icon-btn"
                                class:has-errors={$errorCounts.error > 0}
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
        <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
        <div class="modal-overlay" onclick={() => (showTemplateManager = false)} role="dialog">
            <div onclick={(e) => e.stopPropagation()} role="presentation">
                <TemplateManager onclose={() => (showTemplateManager = false)} />
            </div>
        </div>
    {/if}

    {#if showEntityBrowser}
        <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
        <div class="modal-overlay" onclick={() => (showEntityBrowser = false)} role="dialog">
            <div onclick={(e) => e.stopPropagation()} role="presentation">
                <EntityBrowser
                    oninsert={handleEntityInsert}
                    onclose={() => (showEntityBrowser = false)}
                />
            </div>
        </div>
    {/if}

    {#if showErrorPanel}
        <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
        <div class="modal-overlay" onclick={() => (showErrorPanel = false)} role="dialog">
            <div onclick={(e) => e.stopPropagation()} role="presentation">
                <ErrorPanel onclose={() => (showErrorPanel = false)} />
            </div>
        </div>
    {/if}
</div>

<style>
    .app {
        display: flex;
        flex-direction: column;
        height: 100vh;
        overflow: hidden;
    }

    .main-content {
        flex: 1;
        overflow: hidden;
    }

    .main-content :global(.splitpanes) {
        height: 100%;
    }

    .pane-content {
        display: flex;
        flex-direction: column;
        height: 100%;
    }

    .pane-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.5rem 1rem;
        background: #e1e4e8;
        border-bottom: 1px solid #d1d5da;
        font-weight: 500;
        font-size: 0.875rem;
    }

    .header-actions {
        display: flex;
        gap: 0.25rem;
    }

    .icon-btn {
        background: none;
        border: none;
        cursor: pointer;
        font-size: 1rem;
        padding: 0.25rem;
        border-radius: 4px;
    }

    .icon-btn:hover {
        background: rgba(0, 0, 0, 0.1);
    }

    .icon-btn.has-errors {
        color: #cb2431;
    }

    .modal-overlay {
        position: fixed;
        inset: 0;
        background: rgba(0, 0, 0, 0.5);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 100;
    }
</style>
