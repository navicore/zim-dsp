//! Parser for the zim-dsp DSL.
//!
//! This module handles parsing of the text-based modular synthesis language,
//! converting lines of DSL code into executable commands.

use crate::modules::{parse_module_type, ModuleType};
use anyhow::{anyhow, Result};

/// Port definition for patchbay interface
#[derive(Debug, Clone)]
pub struct PortDef {
    pub name: String,
    pub port_number: u32,
    pub description: Option<String>,
}

/// Patchbay interface definition
#[derive(Debug, Clone)]
pub struct PatchbayDef {
    pub ports: Vec<PortDef>,
}

/// Import statement
#[derive(Debug, Clone)]
pub struct ImportDef {
    pub module_path: String,
    pub alias: Option<String>,
}

/// Module type reference - either built-in or imported
#[derive(Debug, Clone)]
pub enum ModuleTypeRef {
    /// Built-in module type
    BuiltIn(ModuleType),
    /// Imported module type by name
    Imported(String),
}

/// Commands that can be parsed from the DSL.
#[derive(Debug, Clone)]
pub enum Command {
    /// Create a new module with the given name, type, and parameters.
    CreateModule { name: String, module_type: ModuleTypeRef, params: Vec<f32> },
    /// Connect the output of one module to the input of another.
    Connect { from: String, to: String },
    /// Set a parameter value on a module.
    SetParam { module: String, param: String, value: f32 },
    /// Define patchbay interface for this module
    DefinePatchbay { patchbay: PatchbayDef },
    /// Import a module from external file
    Import { import: ImportDef },
}

impl std::fmt::Display for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CreateModule { name, module_type, .. } => match module_type {
                ModuleTypeRef::BuiltIn(mt) => write!(f, "{name}: {mt}"),
                ModuleTypeRef::Imported(imported_name) => write!(f, "{name}: {imported_name}"),
            },
            Self::Connect { from, to } => {
                write!(f, "{from} -> {to}")
            }
            Self::SetParam { module, param, value } => {
                write!(f, "{module}.{param} = {value}")
            }
            Self::DefinePatchbay { patchbay } => {
                write!(f, "patchbay: {} ports", patchbay.ports.len())
            }
            Self::Import { import } => {
                if let Some(alias) = &import.alias {
                    write!(f, "import {} as {}", import.module_path, alias)
                } else {
                    write!(f, "import {}", import.module_path)
                }
            }
        }
    }
}

/// Parse multiple lines of patch notation, handling multi-line constructs
///
/// # Errors
/// Returns an error if the lines cannot be parsed
pub fn parse_lines(lines: &[&str]) -> Result<Vec<Command>> {
    let mut commands = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            i += 1;
            continue;
        }

        // Check for patchbay definition
        if line == "patchbay:" {
            let (patchbay_command, lines_consumed) = parse_patchbay_block(&lines[i..])?;
            commands.push(patchbay_command);
            i += lines_consumed;
            continue;
        }

        // Parse single line normally
        if let Ok(command) = parse_line(line) {
            commands.push(command);
        } // Skip unparseable lines
        i += 1;
    }

    Ok(commands)
}

/// Parse a patchbay block starting from "patchbay:" line
fn parse_patchbay_block(lines: &[&str]) -> Result<(Command, usize)> {
    if lines.is_empty() || lines[0].trim() != "patchbay:" {
        return Err(anyhow!("Expected patchbay: line"));
    }

    let mut ports = Vec::new();
    let mut i = 1; // Start after "patchbay:" line

    // Parse indented port definitions
    while i < lines.len() {
        let line = lines[i].trim();

        // Stop at empty line or non-indented line
        if line.is_empty() || !lines[i].starts_with("  ") {
            break;
        }

        // Parse port definition: "  name: port number [description]"
        if let Some(colon_pos) = line.find(':') {
            let port_name = line[..colon_pos].trim().to_string();
            let rest = line[colon_pos + 1..].trim();

            // Parse "port number" format
            if let Some(port_keyword_pos) = rest.find("port ") {
                let after_port = rest[port_keyword_pos + 5..].trim();
                let port_parts: Vec<&str> = after_port.split_whitespace().collect();

                if !port_parts.is_empty() {
                    if let Ok(port_number) = port_parts[0].parse::<u32>() {
                        let description = if port_parts.len() > 1 {
                            Some(port_parts[1..].join(" "))
                        } else {
                            None
                        };

                        ports.push(PortDef {
                            name: port_name,
                            port_number,
                            description,
                        });
                    }
                }
            }
        }

        i += 1;
    }

    let patchbay = PatchbayDef { ports };
    let command = Command::DefinePatchbay { patchbay };

    Ok((command, i))
}

/// Parse a single line of patch notation
///
/// # Errors
/// Returns an error if the line cannot be parsed as a valid command
///
/// # Panics
/// Panics if arrow position is found but subsequent parsing fails (internal logic error)
#[allow(clippy::too_many_lines)]
pub fn parse_line(line: &str) -> Result<Command> {
    let line = line.trim();

    let line = line.find(" #").map_or(line, |comment_pos| line[..comment_pos].trim());

    // Skip empty lines and comments
    if line.is_empty() || line.starts_with('#') {
        return Err(anyhow!("Empty or comment line"));
    }

    // Import statement: "import module:name as alias" or "import module:name"
    if let Some(stripped) = line.strip_prefix("import ") {
        let import_part = stripped.trim(); // Remove "import "

        if let Some(as_pos) = import_part.find(" as ") {
            let module_path = import_part[..as_pos].trim().to_string();
            let alias = import_part[as_pos + 4..].trim().to_string();
            return Ok(Command::Import {
                import: ImportDef { module_path, alias: Some(alias) },
            });
        }
        return Ok(Command::Import {
            import: ImportDef {
                module_path: import_part.to_string(),
                alias: None,
            },
        });
    }

    // Patchbay definition: "patchbay:"
    if line == "patchbay:" {
        // For now, return empty patchbay - we'll need multi-line parsing later
        return Ok(Command::DefinePatchbay {
            patchbay: PatchbayDef { ports: Vec::new() },
        });
    }

    // Module creation: "name: type [params]"
    if let Some(colon_pos) = line.find(':') {
        let name = line[..colon_pos].trim().to_string();
        let rest = line[colon_pos + 1..].trim();

        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.is_empty() {
            return Err(anyhow!("Missing module type"));
        }

        // Try to parse as built-in module type first
        let module_type = parse_module_type(parts[0]).map_or_else(
            |_| {
                // Assume it's an imported module type
                let mut params: Vec<f32> = Vec::new();
                for part in &parts[1..] {
                    if let Ok(num) = part.parse::<f32>() {
                        params.push(num);
                    }
                }
                (ModuleTypeRef::Imported(parts[0].to_string()), params)
            },
            |builtin_type| {
                // Handle built-in module types with special parameter parsing
                let mut params: Vec<f32> = Vec::new();
                let mut waveform: Option<String> = None;

                for (i, part) in parts[1..].iter().enumerate() {
                    if i == 0 && builtin_type == ModuleType::Oscillator {
                        // First param for oscillator might be waveform
                        match *part {
                            "sine" | "saw" | "square" | "tri" | "triangle" => {
                                waveform = Some((*part).to_string());
                                continue;
                            }
                            _ => {}
                        }
                    }
                    // Try to parse as number
                    if let Ok(num) = part.parse::<f32>() {
                        params.push(num);
                    }
                }

                // Store waveform in params for now (hack - we'll improve this later)
                if let Some(wf) = waveform {
                    // Encode waveform as a special negative number
                    let wf_code = match wf.as_str() {
                        "saw" => -2.0,
                        "square" => -3.0,
                        "tri" | "triangle" => -4.0,
                        _ => -1.0,
                    };
                    params.insert(0, wf_code);
                }

                (ModuleTypeRef::BuiltIn(builtin_type), params)
            },
        );

        return Ok(Command::CreateModule {
            name,
            module_type: module_type.0,
            params: module_type.1,
        });
    }

    // Parameter setting: "module.param <- value"
    if let Some(arrow_pos) = line.find("<-") {
        let left = line[..arrow_pos].trim();
        let right = line[arrow_pos + 2..].trim();

        // Check if it's a parameter assignment
        if let Some(dot_pos) = left.find('.') {
            let module = left[..dot_pos].to_string();
            let param = left[dot_pos + 1..].to_string();

            if let Ok(value) = right.parse::<f32>() {
                return Ok(Command::SetParam { module, param, value });
            }
        }

        // Otherwise it's a connection
        return Ok(Command::Connect {
            from: right.to_string(),
            to: left.to_string(),
        });
    }

    // Output routing: "out <- source"
    if line.starts_with("out") && line.contains("<-") {
        let arrow_pos = line.find("<-").unwrap();
        let source = line[arrow_pos + 2..].trim();

        return Ok(Command::Connect {
            from: source.to_string(),
            to: "out".to_string(),
        });
    }

    Err(anyhow!("Could not parse line: {line}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_module_creation() {
        let cmd = parse_line("vco: osc 440").unwrap();
        match cmd {
            Command::CreateModule { name, module_type, params } => {
                assert_eq!(name, "vco");
                assert!(matches!(module_type, ModuleTypeRef::BuiltIn(ModuleType::Oscillator)));
                assert_eq!(params, vec![440.0]);
            }
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_parse_connection() {
        let cmd = parse_line("vcf <- vco").unwrap();
        match cmd {
            Command::Connect { from, to } => {
                assert_eq!(from, "vco");
                assert_eq!(to, "vcf");
            }
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_parse_param() {
        let cmd = parse_line("vcf.cutoff <- 800").unwrap();
        match cmd {
            Command::SetParam { module, param, value } => {
                assert_eq!(module, "vcf");
                assert_eq!(param, "cutoff");
                assert!((value - 800.0).abs() < f32::EPSILON);
            }
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_parse_import_with_alias() {
        let cmd = parse_line("import mymodules:supersaw as ss").unwrap();
        match cmd {
            Command::Import { import } => {
                assert_eq!(import.module_path, "mymodules:supersaw");
                assert_eq!(import.alias, Some("ss".to_string()));
            }
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_parse_import_without_alias() {
        let cmd = parse_line("import mymodules:supersaw").unwrap();
        match cmd {
            Command::Import { import } => {
                assert_eq!(import.module_path, "mymodules:supersaw");
                assert_eq!(import.alias, None);
            }
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_parse_imported_module_creation() {
        let cmd = parse_line("osc1: supersaw 440").unwrap();
        match cmd {
            Command::CreateModule { name, module_type, params } => {
                assert_eq!(name, "osc1");
                assert!(matches!(module_type, ModuleTypeRef::Imported(ref s) if s == "supersaw"));
                assert_eq!(params, vec![440.0]);
            }
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_parse_patchbay() {
        let cmd = parse_line("patchbay:").unwrap();
        match cmd {
            Command::DefinePatchbay { patchbay } => {
                assert_eq!(patchbay.ports.len(), 0); // Empty for now
            }
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_parse_multiline_patchbay() {
        let lines = vec![
            "patchbay:",
            "  gate: port 1",
            "  pitch: port 2 Frequency control input",
            "  audio_out: port 3",
            "",
            "osc: osc sine 440",
        ];

        let commands = parse_lines(&lines).unwrap();
        assert_eq!(commands.len(), 2); // patchbay + module creation

        match &commands[0] {
            Command::DefinePatchbay { patchbay } => {
                assert_eq!(patchbay.ports.len(), 3);

                assert_eq!(patchbay.ports[0].name, "gate");
                assert_eq!(patchbay.ports[0].port_number, 1);
                assert_eq!(patchbay.ports[0].description, None);

                assert_eq!(patchbay.ports[1].name, "pitch");
                assert_eq!(patchbay.ports[1].port_number, 2);
                assert_eq!(
                    patchbay.ports[1].description,
                    Some("Frequency control input".to_string())
                );

                assert_eq!(patchbay.ports[2].name, "audio_out");
                assert_eq!(patchbay.ports[2].port_number, 3);
                assert_eq!(patchbay.ports[2].description, None);
            }
            _ => panic!("Expected patchbay command"),
        }

        // Second command should be module creation
        match &commands[1] {
            Command::CreateModule { name, .. } => {
                assert_eq!(name, "osc");
            }
            _ => panic!("Expected module creation command"),
        }
    }

    #[test]
    fn test_parse_lines_mixed_content() {
        let lines = vec![
            "import mymodules:supersaw as ss",
            "",
            "# This is a comment",
            "osc1: ss 440",
            "osc2: osc sine 220",
            "osc2.freq <- osc1.sine",
        ];

        let commands = parse_lines(&lines).unwrap();
        assert_eq!(commands.len(), 4); // import + 2 modules + connection

        assert!(matches!(commands[0], Command::Import { .. }));
        assert!(matches!(commands[1], Command::CreateModule { .. }));
        assert!(matches!(commands[2], Command::CreateModule { .. }));
        assert!(matches!(commands[3], Command::Connect { .. }));
    }
}
