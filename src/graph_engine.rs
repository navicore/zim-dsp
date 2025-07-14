//! Graph-based audio engine for the REPL

use crate::graph::{Connection, ConnectionExpr, GraphExecutor, ModuleInfo};
use crate::graph_modules::{GraphEnvelope, GraphFilter, GraphOscillator, GraphVca};
use crate::modules::ModuleType;
use crate::parser::{parse_line, Command};
use anyhow::{anyhow, Result};
// use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
// use std::sync::{Arc, Mutex};

/// Audio engine using the new graph executor
pub struct GraphEngine {
    graph: GraphExecutor,
    stream: Option<cpal::Stream>,
    is_running: bool,
    #[allow(dead_code)]
    sample_rate: f32,
    // Store output module name for audio routing
    output_module: Option<String>,
}

impl GraphEngine {
    pub fn new() -> Self {
        Self {
            graph: GraphExecutor::new(),
            stream: None,
            is_running: false,
            sample_rate: 44100.0,
            output_module: None,
        }
    }

    /// Load a patch from text content
    pub fn load_patch(&mut self, patch_content: &str) -> Result<()> {
        self.clear_patch();

        for line in patch_content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            self.process_line(line)?;
        }

        Ok(())
    }

    /// Process a line of DSL code
    pub fn process_line(&mut self, line: &str) -> Result<String> {
        // First try parsing with the existing parser
        match parse_line(line) {
            Ok(command) => self.handle_parsed_command(command),
            Err(_) => {
                // Try parsing new syntax (like module.port connections)
                self.handle_extended_syntax(line)
            }
        }
    }

    fn handle_parsed_command(&mut self, command: Command) -> Result<String> {
        match command {
            Command::CreateModule { name, module_type, params } => {
                self.create_module(name.clone(), module_type, &params)?;
                Ok(format!("Created module: {name}"))
            }
            Command::Connect { from, to } => {
                // Handle old-style connections by converting to new syntax
                if to == "out" {
                    self.output_module = Some(from.clone());
                    Ok(format!("Connected {from} to output"))
                } else {
                    // Convert old syntax to new - assume connecting to audio input
                    self.parse_connection(&format!("{to}.audio"), &from)
                }
            }
            Command::SetParam { module, param, value } => {
                // TODO: Implement parameter setting on graph modules
                Ok(format!("Set {module}.{param} = {value}"))
            }
        }
    }

    fn handle_extended_syntax(&mut self, line: &str) -> Result<String> {
        // Handle new syntax patterns

        // Pattern: module.port <- source.port
        if line.contains("<-") {
            let parts: Vec<&str> = line.split("<-").collect();
            if parts.len() == 2 {
                let dest = parts[0].trim();
                let source = parts[1].trim();

                return self.parse_connection(dest, source);
            }
        }

        Err(anyhow!("Unrecognized syntax: {}", line))
    }

    fn parse_connection(&mut self, dest: &str, source_expr: &str) -> Result<String> {
        // Special case for output
        if dest == "out" {
            // Parse source to get module and port
            let parts: Vec<&str> = source_expr.split('.').collect();
            if parts.len() == 2 {
                self.output_module = Some(parts[0].to_string());
                return Ok(format!("Connected {source_expr} to audio output"));
            }
            return Err(anyhow!("Output must be connected to a module.port"));
        }

        // Parse destination
        let dest_parts: Vec<&str> = dest.split('.').collect();
        if dest_parts.len() != 2 {
            return Err(anyhow!("Invalid destination format. Use: module.port"));
        }
        let (dest_module, dest_port) = (dest_parts[0], dest_parts[1]);

        // Parse source expression (could be complex)
        let expr = Self::parse_connection_expr(source_expr)?;

        self.graph.add_connection(Connection {
            to_module: dest_module.to_string(),
            to_port: dest_port.to_string(),
            expression: expr,
        });

        Ok(format!("Connected: {dest} <- {source_expr}"))
    }

    fn parse_connection_expr(expr: &str) -> Result<ConnectionExpr> {
        let expr = expr.trim();

        // Check for arithmetic operations
        if let Some(plus_pos) = expr.rfind(" + ") {
            let left = &expr[..plus_pos];
            let right = &expr[plus_pos + 3..];

            if let Ok(offset) = right.parse::<f32>() {
                // module.port + constant
                let base = Self::parse_connection_expr(left)?;
                return Ok(ConnectionExpr::Offset { expr: Box::new(base), offset });
            }
        }

        if let Some(mult_pos) = expr.rfind(" * ") {
            let left = &expr[..mult_pos];
            let right = &expr[mult_pos + 3..];

            if let Ok(factor) = right.parse::<f32>() {
                // module.port * constant
                let base = Self::parse_connection_expr(left)?;
                return Ok(ConnectionExpr::Scaled { expr: Box::new(base), factor });
            }
        }

        // Simple module.port reference
        let parts: Vec<&str> = expr.split('.').collect();
        if parts.len() == 2 {
            return Ok(ConnectionExpr::Direct {
                module: parts[0].to_string(),
                port: parts[1].to_string(),
            });
        }

        Err(anyhow!("Invalid connection expression: {}", expr))
    }

    fn create_module(
        &mut self,
        name: String,
        module_type: ModuleType,
        params: &[f32],
    ) -> Result<()> {
        let module: Box<dyn crate::graph::GraphModule> = match module_type {
            ModuleType::Oscillator => {
                // Handle waveform encoding (negative number means waveform type)
                let freq = if !params.is_empty() && params[0] < 0.0 {
                    // First param is waveform, second is frequency
                    params.get(1).copied().unwrap_or(440.0)
                } else {
                    // First param is frequency
                    params.first().copied().unwrap_or(440.0)
                };
                Box::new(GraphOscillator::new(freq))
            }
            ModuleType::Filter => {
                let cutoff = params.first().copied().unwrap_or(1000.0);
                let resonance = params.get(1).copied().unwrap_or(0.5);
                Box::new(GraphFilter::new(cutoff, resonance))
            }
            ModuleType::Envelope => {
                let attack = params.first().copied().unwrap_or(0.01);
                let decay = params.get(1).copied().unwrap_or(0.1);
                Box::new(GraphEnvelope::new(attack, decay))
            }
            ModuleType::Vca => {
                let gain = params.first().copied().unwrap_or(1.0);
                Box::new(GraphVca::new(gain))
            }
            _ => return Err(anyhow!("Module type {:?} not yet implemented", module_type)),
        };

        self.graph.add_module(name, module);
        Ok(())
    }

    /// Start audio processing
    #[allow(clippy::unnecessary_wraps)] // Will return errors when audio is implemented
    pub fn start(&mut self) -> Result<()> {
        if self.is_running {
            return Ok(());
        }

        // For now, just mark as running without actual audio
        // TODO: Implement proper thread-safe audio callback
        self.is_running = true;
        println!("Audio engine started (note: audio output not yet implemented for graph mode)");

        Ok(())
    }

    /// Stop audio processing
    pub fn stop(&mut self) {
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }
        self.is_running = false;
    }

    /// Clear the patch
    pub fn clear_patch(&mut self) {
        self.stop();
        self.graph = GraphExecutor::new();
        self.output_module = None;
    }

    /// List all modules
    pub fn list_modules(&self) -> Vec<String> {
        self.graph.list_modules()
    }

    /// Inspect a module
    pub fn inspect_module(&self, name: &str) -> Option<ModuleInfo> {
        self.graph.inspect_module(name)
    }

    /// Validate all connections
    pub fn validate_connections(&self) -> Vec<String> {
        self.graph.validate_connections()
    }
}
