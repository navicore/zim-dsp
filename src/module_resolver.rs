//! Module resolution and loading system
//!
//! Handles finding and loading .zim modules from various search paths

use crate::embedded_stdlib::EmbeddedStdlib;
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

/// Module search path configuration
#[derive(Debug, Clone)]
pub struct ModuleSearchPaths {
    /// Path to the current patch file (for relative imports)
    pub current_file_dir: Option<PathBuf>,
    /// User modules directory
    pub user_modules_dir: PathBuf,
    /// System modules directory  
    pub system_modules_dir: PathBuf,
}

impl Default for ModuleSearchPaths {
    fn default() -> Self {
        let user_modules_dir = dirs::home_dir().map_or_else(
            || PathBuf::from("./user_modules"),
            |mut path| {
                path.push(".config");
                path.push("zim-dsp");
                path.push("modules");
                path
            },
        );

        let system_modules_dir = PathBuf::from("/usr/local/share/zim-dsp/modules");

        Self {
            current_file_dir: None,
            user_modules_dir,
            system_modules_dir,
        }
    }
}

impl ModuleSearchPaths {
    /// Create search paths relative to a specific patch file
    pub fn from_patch_file<P: AsRef<Path>>(patch_file: P) -> Self {
        let mut paths = Self::default();

        if let Some(parent) = patch_file.as_ref().parent() {
            paths.current_file_dir = Some(parent.to_path_buf());
        }

        paths
    }

    /// Get all search directories in priority order
    #[must_use]
    pub fn search_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = Vec::new();

        // 1. Relative to current file
        if let Some(current_dir) = &self.current_file_dir {
            dirs.push(current_dir.clone());
        }

        // 2. User modules directory
        dirs.push(self.user_modules_dir.clone());

        // 3. System modules directory
        dirs.push(self.system_modules_dir.clone());

        dirs
    }
}

/// Module resolver handles finding modules by name
#[derive(Debug)]
pub struct ModuleResolver {
    pub search_paths: ModuleSearchPaths,
    pub embedded_stdlib: EmbeddedStdlib,
}

impl ModuleResolver {
    /// Create a new resolver with default search paths
    #[must_use]
    pub fn new() -> Self {
        Self {
            search_paths: ModuleSearchPaths::default(),
            embedded_stdlib: EmbeddedStdlib::new(),
        }
    }

    /// Create a resolver relative to a specific patch file
    pub fn from_patch_file<P: AsRef<Path>>(patch_file: P) -> Self {
        Self {
            search_paths: ModuleSearchPaths::from_patch_file(patch_file),
            embedded_stdlib: EmbeddedStdlib::new(),
        }
    }

    /// Resolve a module path to an actual file path
    ///
    /// # Arguments
    /// * `module_path` - Module path like "mymodules:supersaw", `"basic_osc"`, or "stdlib:uncertainty"
    ///
    /// # Returns
    /// The resolved file path, or an error if not found
    ///
    /// # Errors
    /// Returns an error if the module cannot be found in any search path
    pub fn resolve_module(&self, module_path: &str) -> Result<PathBuf> {
        // Check if this is a stdlib module first
        if EmbeddedStdlib::is_stdlib_path(module_path) {
            // Stdlib modules don't have file paths - they're embedded
            // Return a virtual path for consistency
            return Ok(PathBuf::from(format!("<embedded>/{module_path}")));
        }

        let file_path = self.module_path_to_file_path(module_path);

        // Search in priority order
        for search_dir in self.search_paths.search_dirs() {
            let candidate = search_dir.join(&file_path);

            if candidate.exists() && candidate.is_file() {
                return Ok(candidate);
            }
        }

        Err(anyhow!(
            "Module '{}' not found in search paths: {:?}",
            module_path,
            self.search_paths.search_dirs()
        ))
    }

    /// Convert module path to file path
    ///
    /// Examples:
    /// - `"basic_osc"` -> `"basic_osc.zim"`
    /// - `"mymodules:supersaw"` -> `"mymodules/supersaw.zim"`
    #[must_use]
    pub fn module_path_to_file_path(&self, module_path: &str) -> PathBuf {
        module_path.find(':').map_or_else(
            || PathBuf::from(format!("{module_path}.zim")),
            |colon_pos| {
                // Package:module format -> package/module.zim
                let package = &module_path[..colon_pos];
                let module = &module_path[colon_pos + 1..];
                PathBuf::from(package).join(format!("{module}.zim"))
            },
        )
    }

    /// Load and parse a module file
    ///
    /// # Errors
    /// Returns an error if the module file cannot be found or read
    pub fn load_module(&self, module_path: &str) -> Result<String> {
        // Check if this is a stdlib module first
        if EmbeddedStdlib::is_stdlib_path(module_path) {
            if let Some(module_name) = EmbeddedStdlib::extract_module_name(module_path) {
                return self
                    .embedded_stdlib
                    .get_module(module_name)
                    .map(std::string::ToString::to_string);
            }
            return Err(anyhow!("Invalid stdlib module path: {}", module_path));
        }

        let file_path = self.resolve_module(module_path)?;

        std::fs::read_to_string(&file_path)
            .map_err(|e| anyhow!("Failed to read module file '{}': {}", file_path.display(), e))
    }

    /// Check if a module exists without loading it
    #[must_use]
    pub fn module_exists(&self, module_path: &str) -> bool {
        // Check stdlib first
        if EmbeddedStdlib::is_stdlib_path(module_path) {
            if let Some(module_name) = EmbeddedStdlib::extract_module_name(module_path) {
                return self.embedded_stdlib.has_module(module_name);
            }
            return false;
        }

        self.resolve_module(module_path).is_ok()
    }

    /// List all available modules in search paths
    #[must_use]
    pub fn list_available_modules(&self) -> Vec<String> {
        let mut modules = Vec::new();

        // Add embedded stdlib modules first
        for module_name in self.embedded_stdlib.list_modules() {
            modules.push(format!("stdlib:{module_name}"));
        }

        // Add filesystem modules
        for search_dir in self.search_paths.search_dirs() {
            if let Ok(entries) = std::fs::read_dir(&search_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("zim") {
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            // Check if it's in a subdirectory (package)
                            if let Some(parent) = path.parent() {
                                if parent != search_dir {
                                    if let Some(package) =
                                        parent.file_name().and_then(|s| s.to_str())
                                    {
                                        modules.push(format!("{package}:{stem}"));
                                        continue;
                                    }
                                }
                            }
                            modules.push(stem.to_string());
                        }
                    }
                }
            }
        }

        modules.sort();
        modules.dedup();
        modules
    }
}

impl Default for ModuleResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_path_conversion() {
        let resolver = ModuleResolver::new();

        assert_eq!(resolver.module_path_to_file_path("basic_osc"), PathBuf::from("basic_osc.zim"));

        assert_eq!(
            resolver.module_path_to_file_path("mymodules:supersaw"),
            PathBuf::from("mymodules/supersaw.zim")
        );
    }

    #[test]
    fn test_search_paths_priority() {
        let temp_dir = std::env::temp_dir();
        let patch_file = temp_dir.join("test.zim");

        let paths = ModuleSearchPaths::from_patch_file(&patch_file);
        let search_dirs = paths.search_dirs();

        // Should start with the patch file's directory
        assert_eq!(search_dirs[0], temp_dir);

        // Should have user modules dir
        assert!(search_dirs.iter().any(|p| p.to_string_lossy().contains("zim-dsp/modules")));

        // Should have system modules dir
        assert!(search_dirs.contains(&PathBuf::from("/usr/local/share/zim-dsp/modules")));
    }
}
