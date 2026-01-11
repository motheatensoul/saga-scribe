<script lang="ts">
    import {
        validationStore,
        validationResult,
        isValidating,
        validationError,
        validationCounts,
        availableSchemas,
        selectedSchemaId,
    } from "$lib/stores/validation";
    import { listSchemas, validateXml } from "$lib/tauri";
    import { errorStore } from "$lib/stores/errors";
    import {
        CheckCircle,
        XCircle,
        AlertTriangle,
        Loader,
        X as CloseButton,
    } from "@lucide/svelte";
    import { onMount } from "svelte";

    let { xmlContent, onclose }: { xmlContent: string; onclose?: () => void } =
        $props();

    onMount(async () => {
        try {
            const schemas = await listSchemas();
            validationStore.setSchemas(schemas);
        } catch (e) {
            errorStore.warning("Validation", "Failed to load schemas", String(e));
        }
    });

    async function runValidation() {
        if (!xmlContent) {
            validationStore.setError("No XML content to validate");
            return;
        }

        const schemaId = $selectedSchemaId;
        validationStore.startValidation();

        try {
            const result = await validateXml(xmlContent, schemaId);
            validationStore.setResultKeepSchemas(result);

            if (result.valid) {
                errorStore.info(
                    "Validation",
                    `Document is valid against ${result.schemaName}`,
                );
            } else {
                errorStore.warning(
                    "Validation",
                    `Found ${result.errorCount} error(s) and ${result.warningCount} warning(s)`,
                );
            }
        } catch (e) {
            validationStore.setError(String(e));
            errorStore.error("Validation", "Validation failed", String(e));
        }
    }

    function handleSchemaChange(event: Event) {
        const target = event.target as HTMLSelectElement;
        validationStore.selectSchema(target.value);
    }
</script>

<div
    class="bg-base-100 text-base-content font-mono text-sm h-full flex flex-col"
>
    <div
        class="flex justify-between items-center p-3 border-b border-base-300"
    >
        <h2 class="flex items-center gap-2 font-bold">
            XML Validation
            {#if $validationResult?.valid === true}
                <CheckCircle class="text-success" size={16} />
            {:else if $validationResult?.valid === false}
                <XCircle class="text-error" size={16} />
            {/if}
        </h2>
        <div class="flex gap-2 items-center">
            <select
                class="select select-xs select-bordered bg-base-100 text-base-content"
                value={$selectedSchemaId}
                onchange={handleSchemaChange}
                disabled={$isValidating}
            >
                {#each $availableSchemas as schema (schema.id)}
                    <option value={schema.id}>{schema.name}</option>
                {/each}
                {#if $availableSchemas.length === 0}
                    <option value="tei-p5">TEI P5</option>
                {/if}
            </select>
            <button
                class="btn btn-primary btn-sm"
                onclick={runValidation}
                disabled={$isValidating || !xmlContent}
            >
                {#if $isValidating}
                    <Loader class="animate-spin" size={14} />
                {:else}
                    Validate
                {/if}
            </button>
            <button class="btn btn-ghost btn-sm btn-circle" onclick={onclose} aria-label="Close">
                <CloseButton size={16} />
            </button>
        </div>
    </div>

    <div class="overflow-y-auto flex-1 p-2">
        {#if $isValidating}
            <div class="text-center py-8 flex items-center justify-center gap-2">
                <Loader class="animate-spin" size={20} />
                <span>Validating...</span>
            </div>
        {:else if $validationError}
            <div class="p-4 bg-error/20 rounded text-error">
                {$validationError}
            </div>
        {:else if $validationResult}
            {#if $validationResult.valid}
                <div class="p-4 flex items-center gap-2 text-success">
                    <CheckCircle size={20} />
                    <span
                        >Document is valid against {$validationResult.schemaName}</span
                    >
                </div>
            {:else}
                <div class="mb-2 flex gap-4 text-xs">
                    <span class="flex items-center gap-1 text-error">
                        <XCircle size={14} />
                        {$validationCounts.errors} error(s)
                    </span>
                    {#if $validationCounts.warnings > 0}
                        <span class="flex items-center gap-1 text-warning">
                            <AlertTriangle size={14} />
                            {$validationCounts.warnings} warning(s)
                        </span>
                    {/if}
                </div>

                {#each $validationResult.errors as error, i}
                    <div
                        class="grid grid-cols-[1.5rem_auto_1fr] gap-2 p-1.5 rounded hover:bg-base-content/10 items-start"
                    >
                        <span
                            class="text-center"
                            class:text-error={!error.isWarning}
                            class:text-warning={error.isWarning}
                        >
                            {error.isWarning ? "⚠" : "✕"}
                        </span>
                        {#if error.line}
                            <span class="opacity-50 text-xs">Line {error.line}</span>
                        {:else}
                            <span></span>
                        {/if}
                        <span class="break-words">{error.message}</span>
                    </div>
                {/each}
            {/if}
        {:else}
            <div class="text-center py-8 opacity-50">
                Select a schema and click "Validate" to check your XML
            </div>
        {/if}
    </div>
</div>
