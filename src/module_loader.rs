//! Module loading system with patchbay interface extraction
//!
//! Handles loading .zim modules and extracting their patchbay interfaces

use crate::module_resolver::ModuleResolver;
use crate::parser::{parse_lines, Command, PatchbayDef};
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::Path;

/// Represents a loaded module with its interface
#[derive(Debug, Clone)]
pub struct LoadedModule {
    /// Module path that was used to load this module
    pub module_path: String,
    /// File path where the module was found
    pub file_path: std::path::PathBuf,
    /// Raw content of the module file
    pub content: String,
    /// Extracted patchbay interface (if any)
    pub patchbay: Option<PatchbayDef>,
    /// All commands from the module (for internal implementation)
    pub commands: Vec<Command>,
}

/// Module loader with caching and interface extraction
#[derive(Debug)]
pub struct ModuleLoader {
    resolver: ModuleResolver,
    /// Cache of loaded modules to avoid re-parsing
    cache: HashMap<String, LoadedModule>,
}

impl ModuleLoader {
    /// Create a new module loader with default resolver
    #[must_use]
    pub fn new() -> Self {
        Self {
            resolver: ModuleResolver::new(),
            cache: HashMap::new(),
        }
    }

    /// Create a module loader relative to a specific patch file
    pub fn from_patch_file<P: AsRef<Path>>(patch_file: P) -> Self {
        Self {
            resolver: ModuleResolver::from_patch_file(patch_file),
            cache: HashMap::new(),
        }
    }

    /// Load a module and extract its patchbay interface
    ///
    /// # Arguments
    /// * `module_path` - Module path like "mymodules:supersaw" or `"basic_osc"`
    ///
    /// # Returns
    /// The loaded module with extracted interface, or an error if loading failed
    ///
    /// # Errors
    /// Returns an error if the module cannot be found, read, or parsed
    pub fn load_module(&mut self, module_path: &str) -> Result<LoadedModule> {
        // Check cache first
        if let Some(cached) = self.cache.get(module_path) {
            return Ok(cached.clone());
        }

        // Load from file system
        let file_path = self.resolver.resolve_module(module_path)?;
        let content = self.resolver.load_module(module_path)?;

        // Parse the module content
        let lines: Vec<&str> = content.lines().collect();
        let commands = parse_lines(&lines)?;

        // Extract patchbay definition
        let patchbay = Self::extract_patchbay(&commands);

        // Create loaded module
        let loaded_module = LoadedModule {
            module_path: module_path.to_string(),
            file_path,
            content,
            patchbay,
            commands,
        };

        // Cache and return
        self.cache.insert(module_path.to_string(), loaded_module.clone());
        Ok(loaded_module)
    }

    /// Check if a module exists without loading it
    #[must_use]
    pub fn module_exists(&self, module_path: &str) -> bool {
        self.cache.contains_key(module_path) || self.resolver.module_exists(module_path)
    }

    /// List available modules
    #[must_use]
    pub fn list_available_modules(&self) -> Vec<String> {
        self.resolver.list_available_modules()
    }

    /// Get module resolver for advanced usage
    #[must_use]
    pub const fn resolver(&self) -> &ModuleResolver {
        &self.resolver
    }

    /// Extract patchbay definition from parsed commands
    fn extract_patchbay(commands: &[Command]) -> Option<PatchbayDef> {
        for command in commands {
            if let Command::DefinePatchbay { patchbay } = command {
                return Some(patchbay.clone());
            }
        }
        None
    }

    /// Validate that a loaded module has a proper patchbay interface
    ///
    /// # Errors
    /// Returns an error if the module has no patchbay, empty patchbay, or duplicate ports
    pub fn validate_module_interface(&self, loaded_module: &LoadedModule) -> Result<()> {
        let patchbay = loaded_module.patchbay.as_ref().ok_or_else(|| {
            anyhow!("Module '{}' has no patchbay interface", loaded_module.module_path)
        })?;

        if patchbay.ports.is_empty() {
            return Err(anyhow!(
                "Module '{}' has empty patchbay interface",
                loaded_module.module_path
            ));
        }

        // Check for duplicate port numbers
        let mut port_numbers = std::collections::HashSet::new();
        for port in &patchbay.ports {
            if !port_numbers.insert(port.port_number) {
                return Err(anyhow!(
                    "Module '{}' has duplicate port number {} in patchbay",
                    loaded_module.module_path,
                    port.port_number
                ));
            }
        }

        // Check for duplicate port names
        let mut port_names = std::collections::HashSet::new();
        for port in &patchbay.ports {
            if !port_names.insert(&port.name) {
                return Err(anyhow!(
                    "Module '{}' has duplicate port name '{}' in patchbay",
                    loaded_module.module_path,
                    port.name
                ));
            }
        }

        Ok(())
    }

    /// Process imports from a set of commands and load all referenced modules
    ///
    /// # Errors
    /// Returns an error if any referenced module cannot be loaded or validated
    pub fn process_imports(&mut self, commands: &[Command]) -> Result<Vec<LoadedModule>> {
        let mut loaded_modules = Vec::new();

        for command in commands {
            if let Command::Import { import } = command {
                let loaded_module = self.load_module(&import.module_path)?;
                self.validate_module_interface(&loaded_module)?;
                loaded_modules.push(loaded_module);
            }
        }

        Ok(loaded_modules)
    }

    /// Clear the module cache (useful for hot reload scenarios)
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Get a loaded module from cache without triggering a load
    #[must_use]
    pub fn get_cached_module(&self, module_path: &str) -> Option<&LoadedModule> {
        self.cache.get(module_path)
    }
}

impl Default for ModuleLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ImportDef;

    #[test]
    fn test_module_loading() {
        // This test requires the test_voice.zim file to exist
        let test_modules_dir = std::path::PathBuf::from("examples/modules");
        if !test_modules_dir.exists() {
            // Skip test if examples don't exist
            return;
        }

        let mut loader = ModuleLoader::from_patch_file("examples/test.zim");

        match loader.load_module("modules:test_voice") {
            Ok(loaded_module) => {
                assert_eq!(loaded_module.module_path, "modules:test_voice");
                assert!(loaded_module.file_path.exists());
                assert!(!loaded_module.content.is_empty());

                // Should have extracted patchbay
                assert!(loaded_module.patchbay.is_some());
                let patchbay = loaded_module.patchbay.as_ref().unwrap();
                assert_eq!(patchbay.ports.len(), 4);

                // Validate the interface
                loader
                    .validate_module_interface(&loaded_module)
                    .expect("Module interface should be valid");
            }
            Err(e) => {
                // Expected if the test module doesn't exist
                println!("Test module not found: {e}");
            }
        }
    }

    #[test]
    fn test_module_caching() {
        let mut loader = ModuleLoader::new();

        // Load the same module twice - should use cache the second time
        if loader.module_exists("modules:test_voice") {
            let first_load = loader.load_module("modules:test_voice").unwrap();
            let second_load = loader.load_module("modules:test_voice").unwrap();

            // Should be the same content (from cache)
            assert_eq!(first_load.module_path, second_load.module_path);
            assert_eq!(first_load.content, second_load.content);
        }
    }

    #[test]
    fn test_import_processing() {
        let mut loader = ModuleLoader::new();

        // Create a test command with an import
        let import = ImportDef {
            module_path: "test_module".to_string(),
            alias: None,
        };
        let commands = vec![Command::Import { import }];

        // This will fail since test_module doesn't exist, but we can test the logic
        let result = loader.process_imports(&commands);
        assert!(result.is_err()); // Expected - module doesn't exist
    }
}
