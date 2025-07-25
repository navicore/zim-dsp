//! Embedded standard library modules
//!
//! This module provides .zim modules that are embedded in the binary
//! for portability and guaranteed availability.

use anyhow::{anyhow, Result};
use std::collections::HashMap;

/// Static map of embedded standard library modules
///
/// Each module is stored as a (content, description) tuple where:
/// - content: The actual .zim module source code
/// - description: Human-readable description of what the module does
static EMBEDDED_MODULES: &[(&str, &str, &str)] = &[(
    "uncertainty",
    include_str!("../stdlib/uncertainty.zim"),
    "Random CV and gate generator inspired by the Buchla Source of Uncertainty",
)];

/// Manages embedded standard library modules
#[derive(Debug)]
pub struct EmbeddedStdlib {
    modules: HashMap<String, (String, String)>, // name -> (content, description)
}

impl EmbeddedStdlib {
    /// Create a new embedded stdlib manager
    #[must_use]
    pub fn new() -> Self {
        let mut modules = HashMap::new();

        // Load all embedded modules
        for (name, content, description) in EMBEDDED_MODULES {
            modules
                .insert((*name).to_string(), ((*content).to_string(), (*description).to_string()));
        }

        Self { modules }
    }

    /// Check if a module exists in the embedded stdlib
    #[must_use]
    pub fn has_module(&self, name: &str) -> bool {
        self.modules.contains_key(name)
    }

    /// Get the content of an embedded module
    ///
    /// # Errors
    /// Returns an error if the module is not found
    pub fn get_module(&self, name: &str) -> Result<&str> {
        self.modules
            .get(name)
            .map(|(content, _)| content.as_str())
            .ok_or_else(|| anyhow!("Embedded stdlib module '{}' not found", name))
    }

    /// Get the description of an embedded module
    #[must_use]
    pub fn get_description(&self, name: &str) -> Option<&str> {
        self.modules.get(name).map(|(_, description)| description.as_str())
    }

    /// List all available embedded modules
    #[must_use]
    pub fn list_modules(&self) -> Vec<String> {
        let mut names: Vec<String> = self.modules.keys().cloned().collect();
        names.sort();
        names
    }

    /// Get module info for inspection
    #[must_use]
    pub fn get_module_info(&self, name: &str) -> Option<EmbeddedModuleInfo> {
        self.modules.get(name).map(|(content, description)| EmbeddedModuleInfo {
            name: name.to_string(),
            description: description.clone(),
            content: content.clone(),
            size_bytes: content.len(),
        })
    }

    /// Check if a module path refers to the stdlib namespace
    #[must_use]
    pub fn is_stdlib_path(module_path: &str) -> bool {
        module_path.starts_with("stdlib:")
    }

    /// Extract the module name from a stdlib path
    ///
    /// # Examples
    /// - `stdlib:uncertainty` -> `uncertainty`
    /// - `stdlib:filters:moog` -> `filters:moog`
    #[must_use]
    pub fn extract_module_name(module_path: &str) -> Option<&str> {
        if Self::is_stdlib_path(module_path) {
            module_path.strip_prefix("stdlib:")
        } else {
            None
        }
    }
}

impl Default for EmbeddedStdlib {
    fn default() -> Self {
        Self::new()
    }
}

/// Information about an embedded module
#[derive(Debug, Clone)]
pub struct EmbeddedModuleInfo {
    pub name: String,
    pub description: String,
    pub content: String,
    pub size_bytes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stdlib_path_detection() {
        assert!(EmbeddedStdlib::is_stdlib_path("stdlib:uncertainty"));
        assert!(EmbeddedStdlib::is_stdlib_path("stdlib:filters:moog"));
        assert!(!EmbeddedStdlib::is_stdlib_path("mymodules:voice"));
        assert!(!EmbeddedStdlib::is_stdlib_path("uncertainty"));
    }

    #[test]
    fn test_module_name_extraction() {
        assert_eq!(EmbeddedStdlib::extract_module_name("stdlib:uncertainty"), Some("uncertainty"));
        assert_eq!(
            EmbeddedStdlib::extract_module_name("stdlib:filters:moog"),
            Some("filters:moog")
        );
        assert_eq!(EmbeddedStdlib::extract_module_name("mymodules:voice"), None);
        assert_eq!(EmbeddedStdlib::extract_module_name("uncertainty"), None);
    }

    #[test]
    fn test_stdlib_with_uncertainty() {
        let stdlib = EmbeddedStdlib::new();
        assert!(!stdlib.list_modules().is_empty());
        assert!(stdlib.has_module("uncertainty"));
        assert!(stdlib.get_module("uncertainty").is_ok());

        let modules = stdlib.list_modules();
        assert!(modules.contains(&"uncertainty".to_string()));
    }
}
