import { get } from "svelte/store";
import { resolveResource } from "@tauri-apps/api/path";
import {
    listTemplates,
    listStylesheets,
    loadEntities,
    loadTextFile,
    loadCustomEntities,
    loadCustomMappings,
    loadOnpHeadwords,
    loadInflections,
} from "$lib/tauri";
import { settings } from "$lib/stores/settings";
import { templateStore } from "$lib/stores/template";
import { stylesheetStore } from "$lib/stores/stylesheets";
import { entityStore } from "$lib/stores/entities";
import { errorStore } from "$lib/stores/errors";
import { dictionaryStore, inflectionStore } from "$lib/stores/dictionary";
import { initPerf, perfRecord } from "$lib/perf";

/** The bundled data the compiler is given with each compile */
export interface CompilerResources {
    normalizerJson: string | null;
    entityMappingsJson: string | null;
}

// Flatpak resource path prefix
const FLATPAK_RESOURCE_PREFIX = "/app/bin";

/**
 * Generate all possible paths for a resource file.
 * Tries: Tauri resolved path, dev path, Flatpak path
 */
async function getResourcePaths(resourceName: string): Promise<string[]> {
    const tauriPath = await resolveResource(resourceName);

    // Dev path: transform Tauri path to static folder
    const devPath = tauriPath.replace(
        new RegExp(`src-tauri/target/[^/]+/${resourceName.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}$`),
        `static/${resourceName}`,
    );

    // Flatpak path: /app/bin/{resourceName}
    const flatpakPath = `${FLATPAK_RESOURCE_PREFIX}/${resourceName}`;

    return [tauriPath, devPath, flatpakPath];
}

/**
 * Loads a bundled resource from the first of its possible paths that works,
 * logging each attempt under `category`; `null` if none did.
 */
async function loadResource<T>(
    category: string,
    resourceName: string,
    what: string,
    load: (path: string) => Promise<T>,
    loaded: (result: T, path: string) => string,
): Promise<T | null> {
    for (const path of await getResourcePaths(resourceName)) {
        try {
            errorStore.info(category, `Trying to load ${what} from: ${path}`);
            const result = await load(path);
            errorStore.info(category, loaded(result, path));
            return result;
        } catch (e) {
            errorStore.warning(category, `Failed to load ${what} from ${path}`, String(e));
        }
    }
    return null;
}

/** Runs `load`, logging a failure under `category` as a warning */
async function tryLoad(category: string, failure: string, load: () => Promise<void>) {
    try {
        await load();
    } catch (e) {
        errorStore.warning(category, failure, String(e));
    }
}

async function loadTemplates() {
    errorStore.info("Templates", "Loading templates...");
    const templates = await listTemplates();
    templateStore.setTemplates(templates);

    // The previously active template, else the first
    const savedTemplateId = get(settings).activeTemplateId;
    const activeTemplate =
        (savedTemplateId ? templates.find((t) => t.id === savedTemplateId) : undefined) ?? templates[0];
    if (activeTemplate) {
        templateStore.setActive(activeTemplate);
    }
    errorStore.info("Templates", `Loaded ${templates.length} templates`);
}

async function loadStylesheets() {
    errorStore.info("Stylesheets", "Loading stylesheets...");
    const stylesheets = await listStylesheets();
    stylesheetStore.setStylesheets(stylesheets);
    const activeId = get(settings).activeStylesheetId;
    if (activeId && !stylesheets.some((stylesheet) => stylesheet.id === activeId)) {
        settings.update({ activeStylesheetId: "default" });
    }
    errorStore.info("Stylesheets", `Loaded ${stylesheets.length} stylesheets`);
}

/** MENOTA entities, the user's own, and their base letters; returns the base mappings' JSON */
async function loadEntitySet(): Promise<string | null> {
    const entities = await loadResource(
        "Entities",
        "entities/menota.json",
        "entities",
        loadEntities,
        (entities, path) => `Loaded ${Object.keys(entities).length} entities from ${path}`,
    );
    if (!entities) {
        errorStore.error("Entities", "Failed to load entities from any path");
        entityStore.setError("Could not find entity definitions file");
        return null;
    }
    entityStore.setBuiltinEntities(entities);

    await tryLoad("Entities", "Failed to load custom entities", async () => {
        const customEntities = await loadCustomEntities();
        const customCount = Object.keys(customEntities).length;
        if (customCount > 0) {
            errorStore.info("Entities", `Loaded ${customCount} custom entities`);
            entityStore.setCustomEntities(customEntities);
        }
    });

    // Base entity mappings (diplomatic normalization defaults); the raw JSON goes to the compiler
    const baseMappings = await loadResource(
        "Entities",
        "normalizer/entity-base-letters.json",
        "base mappings",
        async (path) => {
            const text = await loadTextFile(path);
            return { text, mappings: JSON.parse(text).mappings || {} };
        },
        ({ mappings }, path) => `Loaded ${Object.keys(mappings).length} base entity mappings from ${path}`,
    );
    if (baseMappings) {
        entityStore.setBaseMappings(baseMappings.mappings);
    }

    // Custom entity mappings (user overrides)
    await tryLoad("Entities", "Failed to load custom mappings", async () => {
        const customMappings = await loadCustomMappings();
        const customCount = Object.keys(customMappings).length;
        if (customCount > 0) {
            errorStore.info("Entities", `Loaded ${customCount} custom entity mappings`);
            entityStore.setCustomMappings(customMappings);
        }
    });

    return baseMappings?.text ?? null;
}

/**
 * Loads everything the app needs at startup: settings, templates,
 * stylesheets, entities, the normalizer and the dictionaries. Failures are
 * logged; the app starts with whatever loaded.
 */
export async function loadAppResources(): Promise<CompilerResources> {
    errorStore.info("App", "Application starting...");
    const start = performance.now();
    const perfLog = await initPerf();
    if (perfLog) {
        errorStore.info("App", `Writing the performance log to ${perfLog}`);
    }

    // Settings first: the others read them
    try {
        errorStore.info("Settings", "Loading settings...");
        await settings.load();
        errorStore.info("Settings", "Settings loaded");
    } catch (e) {
        errorStore.error("Settings", "Failed to load settings", String(e));
    }

    try {
        await loadTemplates();
    } catch (e) {
        errorStore.error("Templates", "Failed to load templates", String(e));
    }

    try {
        await loadStylesheets();
    } catch (e) {
        errorStore.error("Stylesheets", "Failed to load stylesheets", String(e));
    }

    const entityMappingsJson = await loadEntitySet();

    // Normalizer dictionary for multi-level transcription
    const normalizerJson = await loadResource(
        "Normalizer",
        "normalizer/menota-levels.json",
        "normalizer dictionary",
        loadTextFile,
        (_, path) => `Loaded normalizer dictionary from ${path}`,
    );
    if (!normalizerJson) {
        errorStore.warning(
            "Normalizer",
            "Could not load normalizer dictionary - multi-level output may not work correctly",
        );
    }

    // ONP dictionary for lemmatization
    dictionaryStore.setLoading();
    const headwords = await loadResource(
        "Dictionary",
        "dictionary/onp-headwords.json",
        "ONP headwords",
        loadOnpHeadwords,
        (count, path) => `Loaded ${count} ONP headwords from ${path}`,
    );
    if (headwords !== null) {
        dictionaryStore.setLoaded(headwords);
    }

    // User inflection mappings
    await tryLoad("Dictionary", "Failed to load inflection mappings", async () => {
        const store = await loadInflections();
        const count = Object.keys(store.forms).length;
        if (count > 0) {
            errorStore.info("Dictionary", `Loaded ${count} user inflection mappings`);
            inflectionStore.setMappings(store.forms);
        }
    });

    perfRecord("startup", performance.now() - start);
    errorStore.info("App", "Application ready");
    return { normalizerJson, entityMappingsJson };
}
