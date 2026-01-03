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

#[derive(Debug, Clone, Default)]
pub struct EntityRegistry {
    entities: HashMap<String, Entity>,
}

impl EntityRegistry {
    pub fn new() -> Self {
        Self {
            entities: HashMap::new(),
        }
    }

    /// Load entities from a JSON definition file
    pub fn load_from_file<P: AsRef<Path>>(&mut self, path: P) -> Result<(), String> {
        let content = fs::read_to_string(path.as_ref()).map_err(|e| e.to_string())?;
        let def: EntityDefinitionFile = serde_json::from_str(&content).map_err(|e| e.to_string())?;

        for (name, entity) in def.entities {
            self.entities.insert(name, entity);
        }

        Ok(())
    }

    /// Load entities from a JSON string
    pub fn load_from_str(&mut self, json: &str) -> Result<(), String> {
        let def: EntityDefinitionFile = serde_json::from_str(json).map_err(|e| e.to_string())?;

        for (name, entity) in def.entities {
            self.entities.insert(name, entity);
        }

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
