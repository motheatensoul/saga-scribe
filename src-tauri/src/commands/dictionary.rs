use crate::dictionary::{InflectedForm, InflectionStore, OnpEntry, OnpFullEntry, OnpRegistry};
use log::{error, info};
use std::sync::Mutex;
use tauri::{AppHandle, State};

/// Global state for the ONP registry (loaded once at startup)
pub struct OnpState(pub Mutex<Option<OnpRegistry>>);

/// Cached state for the inflection store (avoids disk I/O on every lookup)
pub struct InflectionState(pub Mutex<Option<InflectionStore>>);

/// Load ONP headwords from a JSON file
#[tauri::command]
pub fn load_onp_headwords(path: String, state: State<OnpState>) -> Result<usize, String> {
    info!("Loading ONP headwords from: {}", path);

    let mut registry = OnpRegistry::new();
    registry.load_from_file(&path).map_err(|e| {
        error!("Failed to load ONP headwords: {}", e);
        e
    })?;

    let count = registry.len();
    info!("Loaded {} ONP headwords", count);

    *state.0.lock().unwrap() = Some(registry);
    Ok(count)
}

/// Look up lemma candidates for a wordform
#[tauri::command]
pub fn lookup_lemma(wordform: String, state: State<OnpState>) -> Result<Vec<OnpEntry>, String> {
    let guard = state.0.lock().unwrap();
    let registry = guard.as_ref().ok_or("ONP registry not loaded")?;

    let results = registry.lookup_lemma(&wordform);
    Ok(results.into_iter().cloned().collect())
}

/// Search for lemmas by prefix (for autocomplete)
#[tauri::command]
pub fn search_lemma_prefix(
    prefix: String,
    limit: usize,
    state: State<OnpState>,
) -> Result<Vec<OnpEntry>, String> {
    let guard = state.0.lock().unwrap();
    let registry = guard.as_ref().ok_or("ONP registry not loaded")?;

    let results = registry.search_prefix(&prefix, limit);
    Ok(results.into_iter().cloned().collect())
}

/// Get a specific ONP entry by ID
#[tauri::command]
pub fn get_onp_entry(id: String, state: State<OnpState>) -> Result<Option<OnpEntry>, String> {
    let guard = state.0.lock().unwrap();
    let registry = guard.as_ref().ok_or("ONP registry not loaded")?;

    Ok(registry.get_by_id(&id).cloned())
}

/// Fetch full entry data from ONP API
#[tauri::command]
pub async fn fetch_onp_full_entry(id: String) -> Result<OnpFullEntry, String> {
    let url = format!("https://onp.ku.dk/json/onp/{}", id);
    info!("Fetching ONP entry: {}", url);

    let response = reqwest::get(&url)
        .await
        .map_err(|e| format!("Failed to fetch: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("ONP API returned status: {}", response.status()));
    }

    let entry: OnpFullEntry = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(entry)
}

/// Load user inflection mappings (also populates the cache)
#[tauri::command]
pub fn load_inflections(
    app: AppHandle,
    state: State<InflectionState>,
) -> Result<InflectionStore, String> {
    info!("Loading inflection mappings");
    let store = InflectionStore::load(&app)?;
    // Cache the loaded store
    *state.0.lock().unwrap() = Some(store.clone());
    Ok(store)
}

/// Look up inflections for a wordform (uses cached state for O(1) access)
#[tauri::command]
pub fn lookup_inflection(
    app: AppHandle,
    wordform: String,
    state: State<InflectionState>,
) -> Result<Vec<InflectedForm>, String> {
    let mut guard = state.0.lock().unwrap();
    
    // Lazy-load if cache is empty
    if guard.is_none() {
        info!("Inflection cache miss, loading from disk");
        *guard = Some(InflectionStore::load(&app)?);
    }
    
    let store = guard.as_ref().unwrap();
    Ok(store.lookup(&wordform).into_iter().cloned().collect())
}

/// Add an inflection mapping (updates both disk and cache)
/// The `wordform` parameter should be the diplomatic-level form for consistent lookups.
// We need to introduce an inflection struct as the argument here potentially to make clippy happy, not sure if that gels with tauri though.
#[allow(clippy::too_many_arguments)]
#[tauri::command(rename_all = "camelCase")]
pub fn add_inflection(
    app: AppHandle,
    wordform: String,
    onp_id: String,
    lemma: String,
    analysis: String,
    part_of_speech: String,
    facsimile: Option<String>,
    diplomatic: Option<String>,
    normalized: Option<String>,
    state: State<InflectionState>,
) -> Result<(), String> {
    info!(
        "Adding inflection: {} -> {} ({}) [facs: {:?}, dipl: {:?}, norm: {:?}]",
        wordform, lemma, analysis, facsimile, diplomatic, normalized
    );

    let mut guard = state.0.lock().unwrap();
    
    // Lazy-load if cache is empty
    if guard.is_none() {
        *guard = Some(InflectionStore::load(&app)?);
    }
    
    let store = guard.as_mut().unwrap();
    store.add(
        &wordform,
        InflectedForm {
            onp_id,
            lemma,
            analysis,
            part_of_speech,
            facsimile,
            diplomatic,
            normalized,
        },
    );
    store.save(&app)
}

/// Remove an inflection mapping (updates both disk and cache)
#[tauri::command(rename_all = "camelCase")]
pub fn remove_inflection(
    app: AppHandle,
    wordform: String,
    onp_id: String,
    analysis: String,
    state: State<InflectionState>,
) -> Result<(), String> {
    info!(
        "Removing inflection: {} ({}, {})",
        wordform, onp_id, analysis
    );

    let mut guard = state.0.lock().unwrap();
    
    // Lazy-load if cache is empty
    if guard.is_none() {
        *guard = Some(InflectionStore::load(&app)?);
    }
    
    let store = guard.as_mut().unwrap();
    store.remove(&wordform, &onp_id, &analysis);
    store.save(&app)
}

/// Clear all inflection mappings (updates both disk and cache)
#[tauri::command]
pub fn clear_inflections(
    app: AppHandle,
    state: State<InflectionState>,
) -> Result<(), String> {
    info!("Clearing all inflection mappings");

    let mut guard = state.0.lock().unwrap();
    
    // Create fresh empty store
    let store = InflectionStore::new();
    store.save(&app)?;
    
    // Update cache
    *guard = Some(store);
    Ok(())
}

/// Check if ONP registry is loaded
#[tauri::command]
pub fn is_onp_loaded(state: State<OnpState>) -> bool {
    state.0.lock().unwrap().is_some()
}

/// Get ONP registry stats
#[tauri::command]
pub fn get_onp_stats(state: State<OnpState>) -> Result<(usize, usize), String> {
    let guard = state.0.lock().unwrap();
    let registry = guard.as_ref().ok_or("ONP registry not loaded")?;
    Ok((registry.len(), 0)) // (headword count, placeholder for future stats)
}

/// Export inflection dictionary to a file (uses cache if available)
#[tauri::command]
pub fn export_inflections(
    app: AppHandle,
    path: String,
    state: State<InflectionState>,
) -> Result<usize, String> {
    info!("Exporting inflection dictionary to: {}", path);

    let mut guard = state.0.lock().unwrap();
    
    // Lazy-load if cache is empty
    if guard.is_none() {
        *guard = Some(InflectionStore::load(&app)?);
    }
    
    let store = guard.as_ref().unwrap();
    let count = store.entry_count();

    let content = serde_json::to_string_pretty(&store).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| format!("Failed to write file: {}", e))?;

    info!("Exported {} inflection entries", count);
    Ok(count)
}
