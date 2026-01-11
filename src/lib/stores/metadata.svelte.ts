/**
 * Metadata store for managing manuscript metadata state.
 * Uses Svelte 5 runes for reactivity.
 */

import type { Metadata } from "$lib/types/metadata";
import { createEmptyMetadata } from "$lib/types/metadata";

/** Current project metadata */
let currentMetadata = $state<Metadata>(createEmptyMetadata());

/** Whether metadata has been modified since last save */
let isDirty = $state(false);

/** Whether metadata editor modal is open */
let isEditorOpen = $state(false);

/**
 * Get the current metadata (reactive)
 */
export function getMetadata(): Metadata {
    return currentMetadata;
}

/**
 * Set the current metadata
 */
export function setMetadata(metadata: Metadata | undefined) {
    if (metadata) {
        currentMetadata = metadata;
    } else {
        currentMetadata = createEmptyMetadata();
    }
    isDirty = false;
}

/**
 * Update metadata and mark as dirty
 */
export function updateMetadata(metadata: Metadata) {
    currentMetadata = metadata;
    isDirty = true;
}

/**
 * Check if metadata has unsaved changes
 */
export function isMetadataDirty(): boolean {
    return isDirty;
}

/**
 * Mark metadata as saved (not dirty)
 */
export function markMetadataSaved() {
    isDirty = false;
}

/**
 * Reset metadata to empty state
 */
export function resetMetadata() {
    currentMetadata = createEmptyMetadata();
    isDirty = false;
}

/**
 * Get metadata as JSON string for saving
 */
export function getMetadataJson(): string {
    return JSON.stringify(currentMetadata);
}

/**
 * Load metadata from JSON string
 */
export function loadMetadataFromJson(json: string) {
    try {
        const parsed = JSON.parse(json) as Metadata;
        setMetadata(parsed);
    } catch (e) {
        console.error("Failed to parse metadata JSON:", e);
        resetMetadata();
    }
}

/**
 * Open the metadata editor modal
 */
export function openMetadataEditor() {
    isEditorOpen = true;
}

/**
 * Close the metadata editor modal
 */
export function closeMetadataEditor() {
    isEditorOpen = false;
}

/**
 * Check if metadata editor is open
 */
export function isMetadataEditorOpen(): boolean {
    return isEditorOpen;
}

/**
 * Toggle metadata editor modal
 */
export function toggleMetadataEditor() {
    isEditorOpen = !isEditorOpen;
}

/**
 * Create a reactive binding for the editor open state
 */
export function getEditorOpenState() {
    return {
        get isOpen() {
            return isEditorOpen;
        },
        set isOpen(value: boolean) {
            isEditorOpen = value;
        },
    };
}
