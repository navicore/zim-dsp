//! Parser for the zim-dsp DSL.
//!
//! This module handles parsing of the text-based modular synthesis language,
//! converting lines of DSL code into executable commands.

use crate::modules::{parse_module_type, ModuleType};
use anyhow::{anyhow, Result};

/// Commands that can be parsed from the DSL.
#[derive(Debug, Clone)]
pub enum Command {
    /// Create a new module with the given name, type, and parameters.
    CreateModule { name: String, module_type: ModuleType, params: Vec<f32> },
    /// Connect the output of one module to the input of another.
    Connect { from: String, to: String },
    /// Set a parameter value on a module.
    SetParam { module: String, param: String, value: f32 },
}

impl std::fmt::Display for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CreateModule { name, module_type, .. } => {
                write!(f, "{name}: {module_type}")
            }
            Self::Connect { from, to } => {
                write!(f, "{from} -> {to}")
            }
            Self::SetParam { module, param, value } => {
                write!(f, "{module}.{param} = {value}")
            }
        }
    }
}

/// Parse a single line of patch notation
pub fn parse_line(line: &str) -> Result<Command> {
    let line = line.trim();

    // Skip empty lines and comments
    if line.is_empty() || line.starts_with('#') {
        return Err(anyhow!("Empty or comment line"));
    }

    // Module creation: "name: type [params]"
    if let Some(colon_pos) = line.find(':') {
        let name = line[..colon_pos].trim().to_string();
        let rest = line[colon_pos + 1..].trim();

        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.is_empty() {
            return Err(anyhow!("Missing module type"));
        }

        let module_type = parse_module_type(parts[0])?;

        // For oscillators, check if waveform is specified
        let mut params: Vec<f32> = Vec::new();
        let mut waveform: Option<String> = None;

        for (i, part) in parts[1..].iter().enumerate() {
            if i == 0 && module_type == ModuleType::Oscillator {
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

        return Ok(Command::CreateModule { name, module_type, params });
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
                assert_eq!(module_type, ModuleType::Oscillator);
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
}
