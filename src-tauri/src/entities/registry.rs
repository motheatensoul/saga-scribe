use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub unicode: String,
    pub char: String,
    pub description: String,
    #[serde(default)]
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityDefinitionFile {
    pub version: String,
    pub name: String,
    pub entities: HashMap<String, Entity>,
}

/// Registry of named character entities (e.g., `eth` → `ð`).
///
/// Provides both forward lookup (name → char) and reverse lookup (char → name)
/// for entity resolution and preservation during import/export cycles.
#[derive(Debug, Clone)]
pub struct EntityRegistry {
    /// Forward mapping: entity name → entity definition
    entities: HashMap<String, Entity>,
    /// Reverse mapping: character → entity name (for import reverse lookup)
    reverse_map: HashMap<String, String>,
}

impl Default for EntityRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl EntityRegistry {
    pub fn new() -> Self {
        Self {
            entities: HashMap::new(),
            reverse_map: HashMap::new(),
        }
    }

    /// Builds the reverse lookup map from current entities.
    fn build_reverse_map(&mut self) {
        self.reverse_map.clear();
        for (name, entity) in &self.entities {
            // Map the character to the entity name
            // Only add if not already present (first entity wins for ambiguous chars)
            if !self.reverse_map.contains_key(&entity.char) {
                self.reverse_map.insert(entity.char.clone(), name.clone());
            }
        }
    }

    /// Load entities from a JSON definition file
    pub fn load_from_file<P: AsRef<Path>>(&mut self, path: P) -> Result<(), String> {
        let content = fs::read_to_string(path.as_ref()).map_err(|e| e.to_string())?;
        let def: EntityDefinitionFile = serde_json::from_str(&content).map_err(|e| e.to_string())?;

        for (name, entity) in def.entities {
            self.entities.insert(name, entity);
        }

        self.build_reverse_map();
        Ok(())
    }

    /// Load entities from a JSON string
    pub fn load_from_str(&mut self, json: &str) -> Result<(), String> {
        let def: EntityDefinitionFile = serde_json::from_str(json).map_err(|e| e.to_string())?;

        for (name, entity) in def.entities {
            self.entities.insert(name, entity);
        }

        self.build_reverse_map();
        Ok(())
    }

    /// Get an entity by name
    pub fn get(&self, name: &str) -> Option<&Entity> {
        self.entities.get(name)
    }

    /// Check if an entity exists
    pub fn contains(&self, name: &str) -> bool {
        self.entities.contains_key(name)
    }

    /// Resolve an entity name to its character representation
    pub fn resolve(&self, name: &str) -> Option<&str> {
        self.entities.get(name).map(|e| e.char.as_str())
    }

    /// Reverse lookup: find the entity name for a given character.
    ///
    /// Used during import to preserve entity references that were resolved
    /// by the XML parser. Returns the entity name if the character matches
    /// a known entity.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let name = registry.reverse_lookup("ð"); // Returns Some("eth")
    /// ```
    pub fn reverse_lookup(&self, char_str: &str) -> Option<&str> {
        self.reverse_map.get(char_str).map(|s| s.as_str())
    }

    /// Get all entity names
    pub fn names(&self) -> Vec<&String> {
        self.entities.keys().collect()
    }

    /// Get all entities as a list
    pub fn list(&self) -> Vec<(&String, &Entity)> {
        self.entities.iter().collect()
    }

    /// Get entities as a serializable map
    pub fn to_map(&self) -> &HashMap<String, Entity> {
        &self.entities
    }
}
