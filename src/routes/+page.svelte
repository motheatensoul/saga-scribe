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
    import { listTemplates, compileDsl, loadEntities } from '$lib/tauri';
    import { resolveResource, appDataDir } from '@tauri-apps/api/path';

    let editorComponent: Editor;
    let previewContent = $state('');
    let showTemplateManager = $state(false);
    let showEntityBrowser = $state(false);
    let showErrorPanel = $state(false);
    let compileTimeout: ReturnType<typeof setTimeout>;
    let entitiesJson = $state<string | null>(null);

    onMount(async () => {
        errorStore.info('App', 'Application starting...');

        // Load templates
        try {
            errorStore.info('Templates', 'Loading templates...');
            const templates = await listTemplates();
            templateStore.setTemplates(templates);
            if (templates.length > 0) {
                templateStore.setActive(templates[0]);
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
                            entitiesJson: entitiesJson ?? undefined,
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
