//! User-defined modules for zim-dsp
//!
//! This module provides infrastructure for loading and managing user-defined
//! modules from .zim files in the usermodules/ directory.

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::parser::Command;

/// Represents a user-defined module template
#[derive(Debug, Clone)]
#[allow(dead_code)] // template_content will be used in next phase
pub struct UserModuleTemplate {
    /// Name of the module type
    pub name: String,
    /// Input port names
    pub inputs: Vec<String>,
    /// Output port names  
    pub outputs: Vec<String>,
    /// Internal module definitions and connections as raw text
    pub template_content: String,
}

impl UserModuleTemplate {
    /// Create a new user module template
    #[must_use]
    pub const fn new(
        name: String,
        inputs: Vec<String>,
        outputs: Vec<String>,
        template_content: String,
    ) -> Self {
        Self { name, inputs, outputs, template_content }
    }

    /// Expand this template into a flat list of commands for a given instance
    #[must_use]
    pub fn expand(&self, instance_name: &str) -> Vec<Command> {
        let mut expanded_commands = Vec::new();

        // Parse each line of the template content
        for line in self.template_content.lines() {
            let trimmed = line.trim();

            // Skip empty lines and comments
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Skip the module declaration line (already processed)
            if trimmed.starts_with("module ")
                || trimmed.starts_with("inputs:")
                || trimmed.starts_with("outputs:")
            {
                continue;
            }

            // Process template variables and prefix module names
            let expanded_line = self.expand_line(trimmed, instance_name);

            // Try to parse the expanded line as a command
            if let Ok(command) = crate::parser::parse_line(&expanded_line) {
                expanded_commands.push(command);
            }
        }

        expanded_commands
    }

    /// Expand a single line of template content
    fn expand_line(&self, line: &str, instance_name: &str) -> String {
        let mut expanded = line.to_string();

        // Handle template variable substitution and module prefixing
        if line.contains(':') {
            // This is a module creation line - prefix the module name
            if let Some(colon_pos) = line.find(':') {
                let module_name = line[..colon_pos].trim();
                let rest = &line[colon_pos..];
                expanded = format!("{instance_name}_{module_name}{rest}");
            }
        } else if line.contains("<-") {
            // This is a connection line - handle template variables and module prefixing
            expanded = self.expand_connection_line(line, instance_name);
        }

        expanded
    }

    /// Expand a connection line with template variables and module prefixing
    fn expand_connection_line(&self, line: &str, instance_name: &str) -> String {
        let mut expanded = line.to_string();

        // Replace template variables ($input/$output) with actual connection endpoints
        // For now, we'll create placeholder expansions - the actual connections will be
        // handled when the user module is instantiated in a host patch

        // Replace $input variables with external connection placeholders
        for input in &self.inputs {
            let template_var = format!("${input}");
            let placeholder = format!("EXTERNAL_INPUT_{input}");
            expanded = expanded.replace(&template_var, &placeholder);
        }

        // Replace $output variables with external connection placeholders
        for output in &self.outputs {
            let template_var = format!("${output}");
            let placeholder = format!("EXTERNAL_OUTPUT_{output}");
            expanded = expanded.replace(&template_var, &placeholder);
        }

        // Prefix internal module names
        expanded = Self::prefix_module_names(&expanded, instance_name);

        expanded
    }

    /// Prefix module names in a connection line
    fn prefix_module_names(line: &str, instance_name: &str) -> String {
        let mut result = line.to_string();

        // Find module references (words followed by '.' but not starting with EXTERNAL_)
        // This is a simplified approach - we'll improve it later
        let words: Vec<&str> = line.split_whitespace().collect();
        for word in words {
            if word.contains('.') && !word.starts_with("EXTERNAL_") {
                let parts: Vec<&str> = word.split('.').collect();
                if parts.len() == 2 {
                    let module_name = parts[0];
                    let port_name = parts[1];
                    let prefixed = format!("{instance_name}_{module_name}.{port_name}");
                    result = result.replace(word, &prefixed);
                }
            }
        }

        result
    }
}

/// Registry for user-defined modules
#[derive(Debug, Default)]
pub struct UserModuleRegistry {
    /// Map from module type name to template
    modules: HashMap<String, UserModuleTemplate>,
}

#[allow(dead_code)] // Some methods will be used in next phase
impl UserModuleRegistry {
    /// Create a new empty registry
    #[must_use]
    pub fn new() -> Self {
        Self { modules: HashMap::new() }
    }

    /// Register a user module template
    pub fn register(&mut self, template: UserModuleTemplate) {
        self.modules.insert(template.name.clone(), template);
    }

    /// Get a user module template by name
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&UserModuleTemplate> {
        self.modules.get(name)
    }

    /// Check if a module type is a user module
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.modules.contains_key(name)
    }

    /// List all registered user module names
    #[must_use]
    pub fn list_modules(&self) -> Vec<&String> {
        self.modules.keys().collect()
    }

    /// Get number of registered modules
    #[must_use]
    pub fn len(&self) -> usize {
        self.modules.len()
    }

    /// Check if registry is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }

    /// Scan a directory for user module files and load basic metadata
    ///
    /// # Errors
    /// Returns an error if the directory cannot be read or if any file cannot be parsed
    pub fn scan_directory<P: AsRef<Path>>(&mut self, dir_path: P) -> Result<usize> {
        let dir_path = dir_path.as_ref();

        if !dir_path.exists() {
            return Ok(0); // No usermodules directory is fine
        }

        if !dir_path.is_dir() {
            return Err(anyhow!("Path is not a directory: {}", dir_path.display()));
        }

        let mut loaded_count = 0;

        for entry in fs::read_dir(dir_path)? {
            let entry = entry?;
            let path = entry.path();

            // Only process .zim files
            if path.extension().and_then(|s| s.to_str()) == Some("zim") {
                match self.load_user_module(&path) {
                    Ok(()) => {
                        loaded_count += 1;
                        println!("Loaded user module: {}", path.display());
                    }
                    Err(e) => {
                        eprintln!("Failed to load user module {}: {}", path.display(), e);
                    }
                }
            }
        }

        Ok(loaded_count)
    }

    /// Load a single user module file (basic parsing for now)
    fn load_user_module(&mut self, file_path: &Path) -> Result<()> {
        let content = fs::read_to_string(file_path)?;

        let module_name = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow!("Invalid filename: {}", file_path.display()))?
            .to_string();

        // Parse the module definition
        let (parsed_name, inputs, outputs, template_content) =
            Self::parse_user_module_content(&content);

        // Use parsed name if available, otherwise fall back to filename
        let final_name = if parsed_name.is_empty() { module_name } else { parsed_name };

        let template = UserModuleTemplate::new(final_name, inputs, outputs, template_content);

        self.register(template);
        Ok(())
    }

    /// Parse user module content to extract metadata and template
    fn parse_user_module_content(content: &str) -> (String, Vec<String>, Vec<String>, String) {
        let mut module_name = String::new();
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        let mut template_lines = Vec::new();
        let mut in_module_block = false;
        let mut brace_depth = 0u32;

        for line in content.lines() {
            let trimmed = line.trim();

            // Skip comments and empty lines
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Check for module declaration
            if trimmed.starts_with("module ") && trimmed.contains('{') {
                // Extract module name: "module simple_gain {"
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    module_name = parts[1].trim_end_matches('{').trim().to_string();
                }
                in_module_block = true;
                brace_depth = 1;
                continue;
            }

            if in_module_block {
                // Track braces
                brace_depth +=
                    u32::try_from(trimmed.chars().filter(|&c| c == '{').count()).unwrap_or(0);
                brace_depth = brace_depth.saturating_sub(
                    u32::try_from(trimmed.chars().filter(|&c| c == '}').count()).unwrap_or(0),
                );

                // Check for end of module block
                if brace_depth == 0 {
                    break;
                }

                // Parse inputs/outputs
                if trimmed.starts_with("inputs:") {
                    let input_list = trimmed.strip_prefix("inputs:").unwrap().trim();
                    inputs = input_list
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                } else if trimmed.starts_with("outputs:") {
                    let output_list = trimmed.strip_prefix("outputs:").unwrap().trim();
                    outputs = output_list
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                } else {
                    // This is part of the template content
                    template_lines.push(line.to_string());
                }
            } else {
                // Outside module block - include in template
                template_lines.push(line.to_string());
            }
        }

        let template_content = template_lines.join("\n");
        (module_name, inputs, outputs, template_content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_module_template_creation() {
        let template = UserModuleTemplate::new(
            "test_module".to_string(),
            vec!["input1".to_string(), "input2".to_string()],
            vec!["output1".to_string()],
            "vca: vca 1.0".to_string(),
        );

        assert_eq!(template.name, "test_module");
        assert_eq!(template.inputs.len(), 2);
        assert_eq!(template.outputs.len(), 1);
        assert_eq!(template.template_content, "vca: vca 1.0");
    }

    #[test]
    fn test_user_module_registry() {
        let mut registry = UserModuleRegistry::new();
        assert!(registry.is_empty());

        let template = UserModuleTemplate::new(
            "test_module".to_string(),
            vec!["input1".to_string()],
            vec!["output1".to_string()],
            "content".to_string(),
        );

        registry.register(template);
        assert_eq!(registry.len(), 1);
        assert!(registry.contains("test_module"));
        assert!(!registry.contains("unknown_module"));

        let retrieved = registry.get("test_module").unwrap();
        assert_eq!(retrieved.name, "test_module");

        let modules = registry.list_modules();
        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0], "test_module");
    }

    #[test]
    fn test_scan_directory_nonexistent() {
        let mut registry = UserModuleRegistry::new();
        let result = registry.scan_directory("/nonexistent/path");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
        assert!(registry.is_empty());
    }

    #[test]
    fn test_scan_directory_empty() {
        let mut registry = UserModuleRegistry::new();

        // Create a temporary directory for testing
        let temp_dir = std::env::temp_dir().join("zim_test_usermodules_empty");
        let _ = std::fs::create_dir(&temp_dir);

        let result = registry.scan_directory(&temp_dir);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
        assert!(registry.is_empty());

        // Cleanup
        let _ = std::fs::remove_dir(&temp_dir);
    }

    #[test]
    fn test_parse_user_module_content() {
        let content = r"
# Simple gain user module
module simple_gain {
    inputs: audio
    outputs: out
    
    # Internal modules
    vca: vca 0.5
    
    # Internal connections
    vca.audio <- $audio
    $out <- vca.out
}
";

        let (name, inputs, outputs, template) =
            UserModuleRegistry::parse_user_module_content(content);
        assert_eq!(name, "simple_gain");
        assert_eq!(inputs, vec!["audio"]);
        assert_eq!(outputs, vec!["out"]);
        assert!(template.contains("vca: vca 0.5"));
        assert!(template.contains("vca.audio <- $audio"));
    }

    #[test]
    fn test_parse_multiple_inputs_outputs() {
        let content = r"
module complex_filter {
    inputs: audio, cutoff_cv, gate
    outputs: lowpass, highpass, bandpass
    
    vcf: filter moog 1000 0.5
    env: envelope 0.01 0.1
}
";

        let (name, inputs, outputs, _) = UserModuleRegistry::parse_user_module_content(content);
        assert_eq!(name, "complex_filter");
        assert_eq!(inputs, vec!["audio", "cutoff_cv", "gate"]);
        assert_eq!(outputs, vec!["lowpass", "highpass", "bandpass"]);
    }

    #[test]
    fn test_template_expansion() {
        let template = UserModuleTemplate::new(
            "simple_gain".to_string(),
            vec!["audio".to_string()],
            vec!["out".to_string()],
            r"
# Internal modules
vca: vca 0.5

# Internal connections  
vca.audio <- $audio
$out <- vca.out
"
            .to_string(),
        );

        let commands = template.expand("ef");
        assert_eq!(commands.len(), 3); // vca creation + two connections

        // Check that module creation was prefixed
        match &commands[0] {
            Command::CreateModule { name, .. } => {
                assert_eq!(name, "ef_vca");
            }
            _ => panic!("Expected CreateModule command"),
        }
    }

    #[test]
    fn test_line_expansion() {
        let template = UserModuleTemplate::new(
            "test".to_string(),
            vec!["audio".to_string()],
            vec!["out".to_string()],
            String::new(),
        );

        // Test module creation line
        let result = template.expand_line("vca: vca 0.5", "ef");
        assert_eq!(result, "ef_vca: vca 0.5");

        // Test connection line with template variables
        let result = template.expand_line("vca.audio <- $audio", "ef");
        assert_eq!(result, "ef_vca.audio <- EXTERNAL_INPUT_audio");

        let result = template.expand_line("$out <- vca.out", "ef");
        assert_eq!(result, "EXTERNAL_OUTPUT_out <- ef_vca.out");
    }
}
