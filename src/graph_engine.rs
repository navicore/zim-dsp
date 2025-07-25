//! Graph-based audio engine for the REPL

use crate::graph::{Connection, ConnectionExpr, GraphExecutor, ModuleInfo};
use crate::graph_modules::{
    GraphClockDiv, GraphEnvelope, GraphFilter, GraphLfo, GraphManualGate, GraphMonoMixer,
    GraphMult, GraphNoiseGen, GraphOscillator, GraphSampleHold, GraphSeq8, GraphSlewGen,
    GraphStereoMixer, GraphStereoOutput, GraphSwitch, GraphVca, GraphVisual,
};
use crate::module_loader::{LoadedModule, ModuleLoader};
use crate::modules::ModuleType;
use crate::observability::SignalObserver;
use crate::parser::{parse_line, Command, ModuleTypeRef, PatchbayDef};
use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Audio engine using the new graph executor
pub struct GraphEngine {
    graph: Arc<Mutex<GraphExecutor>>,
    stream: Option<cpal::Stream>,
    is_running: bool,
    #[allow(dead_code)]
    sample_rate: f32,
    // Store output module and port for audio routing
    output_module: Option<String>,
    output_port: Option<String>,
    // Track if we have a stereo output module
    has_stereo_output: bool,
    // Module loading system
    module_loader: ModuleLoader,
    // Store imported module interfaces by alias or path
    imported_modules: HashMap<String, PatchbayDef>,
}

impl Default for GraphEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphEngine {
    #[must_use]
    pub fn new() -> Self {
        Self {
            graph: Arc::new(Mutex::new(GraphExecutor::new())),
            stream: None,
            is_running: false,
            sample_rate: 44100.0,
            output_module: None,
            output_port: None,
            has_stereo_output: false,
            module_loader: ModuleLoader::new(),
            imported_modules: HashMap::new(),
        }
    }

    /// Create a new engine with module loading relative to a patch file
    #[must_use]
    pub fn from_patch_file<P: AsRef<std::path::Path>>(patch_file: P) -> Self {
        Self {
            graph: Arc::new(Mutex::new(GraphExecutor::new())),
            stream: None,
            is_running: false,
            sample_rate: 44100.0,
            output_module: None,
            output_port: None,
            has_stereo_output: false,
            module_loader: ModuleLoader::from_patch_file(patch_file),
            imported_modules: HashMap::new(),
        }
    }

    /// Load a patch from text content
    /// Load a patch from text content
    ///
    /// # Errors
    /// Returns an error if any line in the patch fails to parse or process
    pub fn load_patch(&mut self, patch_content: &str) -> Result<()> {
        self.clear_patch();

        for line in patch_content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Err(e) = self.process_line(line) {
                return Err(anyhow!("Error on line '{}': {}", line, e));
            }
        }

        Ok(())
    }

    /// Process a line of DSL code
    /// Process a line of DSL code
    ///
    /// # Errors
    /// Returns an error if the line cannot be parsed or processed
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
                match module_type {
                    ModuleTypeRef::BuiltIn(builtin_type) => {
                        self.create_module(name.clone(), builtin_type, &params)?;
                        Ok(format!("Created module: {name}"))
                    }
                    ModuleTypeRef::Imported(imported_name) => {
                        // Check if the imported module is available
                        if let Some(_patchbay) = self.imported_modules.get(&imported_name) {
                            // Load the full module and instantiate it
                            let loaded_module = self.module_loader.load_module(&imported_name)?;
                            self.instantiate_imported_module(&name, &loaded_module)?;

                            let module_count = loaded_module
                                .commands
                                .iter()
                                .filter(|cmd| matches!(cmd, Command::CreateModule { .. }))
                                .count();

                            Ok(format!(
                                "Created imported module '{name}' with {module_count} internal modules"
                            ))
                        } else {
                            Err(anyhow!(
                                "Imported module '{}' not found. Available modules: {:?}",
                                imported_name,
                                self.imported_modules.keys().collect::<Vec<_>>()
                            ))
                        }
                    }
                }
            }
            Command::Connect { from, to } => {
                // Handle connections
                if to == "out" {
                    // Create implicit stereo output module if needed
                    if !self.has_stereo_output {
                        self.create_module("_output".to_string(), ModuleType::StereoOutput, &[])?;
                        self.has_stereo_output = true;
                    }

                    // Route to stereo output's mono input
                    self.parse_connection("_output.mono", &from)
                } else if to.starts_with("out.") {
                    // Direct stereo output routing (out.left, out.right)
                    if !self.has_stereo_output {
                        self.create_module("_output".to_string(), ModuleType::StereoOutput, &[])?;
                        self.has_stereo_output = true;
                    }

                    let port = to.strip_prefix("out.").unwrap();
                    self.parse_connection(&format!("_output.{port}"), &from)
                } else if to.contains('.') {
                    // New style: dest already has port (e.g., vca.audio <- vco.sine)
                    self.parse_connection(&to, &from)
                } else {
                    // Old style: assume connecting to audio input (e.g., vcf <- vco)
                    self.parse_connection(&format!("{to}.audio"), &from)
                }
            }
            Command::SetParam { module, param, value } => {
                self.graph.lock().unwrap().set_module_param(&module, &param, value)?;
                Ok(format!("Set {module}.{param} = {value}"))
            }
            Command::Import { import } => {
                // Load the module (textual inclusion approach)
                let loaded_module = self.module_loader.load_module(&import.module_path)?;

                // Store the module for instantiation (no patchbay validation needed)
                let key = import.alias.as_ref().unwrap_or(&import.module_path).clone();

                // Create a dummy patchbay entry so the module can be instantiated
                let dummy_patchbay = PatchbayDef { ports: Vec::new() };
                self.imported_modules.insert(key.clone(), dummy_patchbay);

                let module_count = loaded_module
                    .commands
                    .iter()
                    .filter(|cmd| matches!(cmd, Command::CreateModule { .. }))
                    .count();

                Ok(format!(
                    "Imported module '{}' as '{}' with {} internal modules",
                    import.module_path, key, module_count
                ))
            }
            Command::DefinePatchbay { patchbay } => {
                // Store patchbay definition for the current module/patch
                // This is used when defining a module inline rather than importing
                let key = "_current_module".to_string();
                self.imported_modules.insert(key, patchbay.clone());
                Ok(format!("Defined patchbay with {} ports", patchbay.ports.len()))
            }
        }
    }

    fn handle_extended_syntax(&self, line: &str) -> Result<String> {
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

    /// Parse a connection between modules
    ///
    /// # Errors
    /// Returns an error if the destination format is invalid or connection parsing fails
    ///
    /// # Panics
    /// Panics if the graph mutex is poisoned
    pub fn parse_connection(&self, dest: &str, source_expr: &str) -> Result<String> {
        // Parse destination
        let dest_parts: Vec<&str> = dest.split('.').collect();
        if dest_parts.len() != 2 {
            return Err(anyhow!("Invalid destination format. Use: module.port"));
        }
        let (dest_module, dest_port) = (dest_parts[0], dest_parts[1]);

        // Parse source expression (could be complex)
        let expr = Self::parse_connection_expr(source_expr)?;

        // Track connections to stereo output module
        if dest_module == "_output" {
            let mut graph = self.graph.lock().unwrap();

            // Update connection state in the stereo output module
            if let Some(module) = graph.get_module_mut("_output") {
                if let Some(stereo_out) = module.as_any_mut().downcast_mut::<GraphStereoOutput>() {
                    match dest_port {
                        "left" => stereo_out.set_left_connected(true),
                        "right" => stereo_out.set_right_connected(true),
                        _ => {}
                    }
                }
            }

            graph.add_connection(Connection {
                to_module: dest_module.to_string(),
                to_port: dest_port.to_string(),
                expression: expr,
            });
        } else {
            self.graph.lock().unwrap().add_connection(Connection {
                to_module: dest_module.to_string(),
                to_port: dest_port.to_string(),
                expression: expr,
            });
        }

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
            } else if let Ok(offset) = left.parse::<f32>() {
                // constant + module.port
                let base = Self::parse_connection_expr(right)?;
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
            } else if let Ok(factor) = left.parse::<f32>() {
                // constant * module.port
                let base = Self::parse_connection_expr(right)?;
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

    fn create_module(&self, name: String, module_type: ModuleType, params: &[f32]) -> Result<()> {
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
            ModuleType::Lfo => {
                let frequency = params.first().copied().unwrap_or(1.0);
                Box::new(GraphLfo::new(frequency))
            }
            ModuleType::ManualGate => Box::new(GraphManualGate::new()),
            ModuleType::StereoOutput => Box::new(GraphStereoOutput::new()),
            ModuleType::Noise => Box::new(GraphNoiseGen::new()),
            ModuleType::Mixer => {
                // Default to 4-input mixer, or use parameter if provided
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let input_count = params.first().copied().unwrap_or(4.0) as usize;
                Box::new(GraphMonoMixer::new(input_count))
            }
            ModuleType::StereoMixer => {
                // Default to 4-channel stereo mixer, or use parameter if provided
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let channel_count = params.first().copied().unwrap_or(4.0) as usize;
                Box::new(GraphStereoMixer::new(channel_count))
            }
            ModuleType::Slew => {
                // Default rise/fall times, or use parameters if provided
                let rise_time = params.first().copied().unwrap_or(0.1);
                let fall_time = params.get(1).copied().unwrap_or(rise_time);
                Box::new(GraphSlewGen::new(rise_time, fall_time))
            }
            ModuleType::Seq8 => Box::new(GraphSeq8::new()),
            ModuleType::Visual => Box::new(GraphVisual::new()),
            ModuleType::Mult => Box::new(GraphMult::new()),
            ModuleType::Switch => {
                // Default to 4 inputs, or use parameter if provided
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let input_count = params.first().copied().unwrap_or(4.0) as usize;
                Box::new(GraphSwitch::new(input_count))
            }
            ModuleType::ClockDiv => {
                // Default to division by 4, or use parameter if provided
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let division = params.first().copied().unwrap_or(4.0) as usize;
                Box::new(GraphClockDiv::new(division))
            }
            ModuleType::SampleHold => Box::new(GraphSampleHold::new()),
            ModuleType::Output => {
                return Err(anyhow!("Module type {:?} not yet implemented", module_type))
            }
        };

        self.graph.lock().unwrap().add_module(name, module);
        Ok(())
    }

    /// Create a module from a type reference (either built-in or imported)
    /// This is used by the module instantiator
    ///
    /// # Errors
    /// Returns an error if module creation fails or if nested imported modules are encountered
    pub fn create_module_from_type_ref(
        &mut self,
        name: String,
        module_type_ref: ModuleTypeRef,
        params: &[f32],
    ) -> Result<()> {
        match module_type_ref {
            ModuleTypeRef::BuiltIn(builtin_type) => self.create_module(name, builtin_type, params),
            ModuleTypeRef::Imported(imported_name) => {
                // This should not happen during instantiation - imported modules
                // should be resolved before reaching this point
                Err(anyhow!("Cannot create nested imported module: {}", imported_name))
            }
        }
    }

    /// Set a module parameter (public wrapper for instantiator)
    ///
    /// # Errors
    /// Returns an error if the module or parameter is not found
    ///
    /// # Panics
    /// Panics if the graph mutex is poisoned
    pub fn set_module_param(&self, module_name: &str, param_name: &str, value: f32) -> Result<()> {
        self.graph.lock().unwrap().set_module_param(module_name, param_name, value)
    }

    /// Instantiate an imported module by creating its internal modules and connections
    fn instantiate_imported_module(
        &mut self,
        instance_name: &str,
        loaded_module: &LoadedModule,
    ) -> Result<()> {
        // Note: With textual inclusion approach, patchbay is just another module
        // No special patchbay processing needed

        // Process all internal commands (skip the patchbay definition)
        for command in &loaded_module.commands {
            match command {
                Command::DefinePatchbay { .. } => {
                    // Skip patchbay definition - already processed during import
                }
                Command::CreateModule { name, module_type, params } => {
                    // Create internal module with prefixed name
                    let internal_name = format!("{instance_name}_{name}");
                    self.create_module_from_type_ref(internal_name, module_type.clone(), params)?;
                }
                Command::Connect { from, to } => {
                    // Process internal connections with name prefixing
                    let prefixed_from = Self::prefix_module_reference(instance_name, from);
                    let prefixed_to = Self::prefix_module_reference(instance_name, to);

                    self.parse_connection(&prefixed_to, &prefixed_from)?;
                }
                Command::SetParam { module, param, value } => {
                    // Set parameters on internal modules
                    let internal_module = format!("{instance_name}_{module}");
                    self.set_module_param(&internal_module, param, *value)?;
                }
                Command::Import { .. } => {
                    // Nested imports - would need recursive handling
                    return Err(anyhow!(
                        "Nested imports not yet supported in module '{}'",
                        loaded_module.module_path
                    ));
                }
            }
        }

        Ok(())
    }

    /// Prefix module references for instantiation
    fn prefix_module_reference(instance_name: &str, module_ref: &str) -> String {
        // Handle arithmetic expressions
        if module_ref.contains('*') || module_ref.contains('+') || module_ref.contains('-') {
            return Self::prefix_arithmetic_expression(instance_name, module_ref);
        }

        if module_ref.contains('.') {
            // module.port format
            let parts: Vec<&str> = module_ref.split('.').collect();
            if parts.len() == 2 {
                format!("{instance_name}_{}.{}", parts[0], parts[1])
            } else {
                format!("{instance_name}_{module_ref}")
            }
        } else {
            // Simple module reference (could be patchbay port)
            format!("{instance_name}_{module_ref}")
        }
    }

    /// Prefix arithmetic expressions by identifying and prefixing module references
    fn prefix_arithmetic_expression(instance_name: &str, expression: &str) -> String {
        // Simple approach: split on operators and prefix each part that looks like a module reference
        // Split on operators but preserve them
        let parts: Vec<&str> = expression.split_inclusive(&['*', '+', '-', ' ']).collect();
        let mut prefixed_parts = Vec::new();

        for part in parts {
            let trimmed = part.trim_end_matches(['*', '+', '-', ' ']);
            let suffix = &part[trimmed.len()..];

            // Check if this part is a number
            if trimmed.parse::<f32>().is_ok() {
                // It's a number, keep as-is
                prefixed_parts.push(part.to_string());
            } else if trimmed.contains('.') {
                // It's a module.port reference
                let module_parts: Vec<&str> = trimmed.split('.').collect();
                if module_parts.len() == 2 {
                    prefixed_parts.push(format!(
                        "{instance_name}_{}.{}{suffix}",
                        module_parts[0], module_parts[1]
                    ));
                } else {
                    prefixed_parts.push(format!("{instance_name}_{trimmed}{suffix}"));
                }
            } else if !trimmed.is_empty() {
                // It's a simple reference (module or patchbay port)
                prefixed_parts.push(format!("{instance_name}_{trimmed}{suffix}"));
            } else {
                // Just whitespace or operators
                prefixed_parts.push(part.to_string());
            }
        }

        prefixed_parts.join("")
    }

    /// Start audio processing
    /// Start audio processing
    ///
    /// # Errors
    /// Returns an error if the audio system cannot be initialized
    pub fn start(&mut self) -> Result<()> {
        if self.is_running {
            return Ok(());
        }

        // Get the default audio host
        let host = cpal::default_host();

        // Get the default output device
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow!("No audio output device available"))?;

        // Get the default output config
        let config = device.default_output_config()?;

        // Clone the sample rate for use in the closure
        #[allow(clippy::cast_precision_loss)]
        let sample_rate = config.sample_rate().0 as f32;
        self.sample_rate = sample_rate;

        // Clone the graph reference for the audio thread
        let graph_clone = Arc::clone(&self.graph);

        // For stereo output, we'll use the _output module
        let output_module = if self.has_stereo_output {
            Some("_output".to_string())
        } else {
            self.output_module.clone()
        };

        // Build the output stream
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => Self::build_stream::<f32>(
                &device,
                &config.into(),
                graph_clone,
                output_module,
                self.has_stereo_output,
            )?,
            cpal::SampleFormat::I16 => Self::build_stream::<i16>(
                &device,
                &config.into(),
                graph_clone,
                output_module,
                self.has_stereo_output,
            )?,
            cpal::SampleFormat::U16 => Self::build_stream::<u16>(
                &device,
                &config.into(),
                graph_clone,
                output_module,
                self.has_stereo_output,
            )?,
            _ => {
                return Err(anyhow!("Unsupported sample format"));
            }
        };

        // Start the stream
        stream.play()?;

        // Store the stream
        self.stream = Some(stream);
        self.is_running = true;

        println!("Audio engine started at {sample_rate} Hz");
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
    /// Clear the patch
    ///
    /// # Panics
    /// Panics if the graph mutex is poisoned
    pub fn clear_patch(&mut self) {
        self.stop();
        *self.graph.lock().unwrap() = GraphExecutor::new();
        self.output_module = None;
        self.output_port = None;
        self.has_stereo_output = false;
        self.module_loader.clear_cache();
        self.imported_modules.clear();
    }

    /// List all modules
    /// List all modules
    ///
    /// # Panics
    /// Panics if the graph mutex is poisoned
    #[must_use]
    pub fn list_modules(&self) -> Vec<String> {
        self.graph.lock().unwrap().list_modules()
    }

    /// Inspect a module
    /// Inspect a module
    ///
    /// # Panics
    /// Panics if the graph mutex is poisoned
    #[must_use]
    pub fn inspect_module(&self, name: &str) -> Option<ModuleInfo> {
        self.graph.lock().unwrap().inspect_module(name)
    }

    /// Inspect a module type (e.g., "osc", "filter") by creating a temporary instance
    ///
    /// # Panics
    /// Panics if module creation fails
    #[must_use]
    pub fn inspect_module_type(module_type_name: &str) -> Option<ModuleInfo> {
        use crate::modules::ModuleType;

        // Try to parse the module type
        let module_type = match module_type_name {
            "osc" | "oscillator" => ModuleType::Oscillator,
            "filter" | "vcf" => ModuleType::Filter,
            "envelope" | "env" => ModuleType::Envelope,
            "vca" => ModuleType::Vca,
            "lfo" => ModuleType::Lfo,
            "gate" | "manual_gate" => ModuleType::ManualGate,
            "stereo_output" | "stereo" => ModuleType::StereoOutput,
            "noise" => ModuleType::Noise,
            "mixer" => ModuleType::Mixer,
            "stereo_mixer" => ModuleType::StereoMixer,
            "slew" => ModuleType::Slew,
            "seq8" | "sequencer" => ModuleType::Seq8,
            "visual" => ModuleType::Visual,
            "mult" | "multiple" => ModuleType::Mult,
            "switch" => ModuleType::Switch,
            "clockdiv" | "clock_div" => ModuleType::ClockDiv,
            "samplehold" | "sample_hold" | "sh" => ModuleType::SampleHold,
            _ => return None,
        };

        // Create a temporary module to inspect its interface
        let temp_module: Box<dyn crate::graph::GraphModule> = match module_type {
            ModuleType::Oscillator => Box::new(crate::graph_modules::GraphOscillator::new(440.0)),
            ModuleType::Filter => Box::new(crate::graph_modules::GraphFilter::new(1000.0, 0.5)),
            ModuleType::Envelope => Box::new(crate::graph_modules::GraphEnvelope::new(0.01, 0.1)),
            ModuleType::Vca => Box::new(crate::graph_modules::GraphVca::new(1.0)),
            ModuleType::Lfo => Box::new(crate::graph_modules::GraphLfo::new(1.0)),
            ModuleType::ManualGate => Box::new(crate::graph_modules::GraphManualGate::new()),
            ModuleType::StereoOutput => Box::new(crate::graph_modules::GraphStereoOutput::new()),
            ModuleType::Noise => Box::new(crate::graph_modules::GraphNoiseGen::new()),
            ModuleType::Mixer => Box::new(crate::graph_modules::GraphMonoMixer::new(4)),
            ModuleType::StereoMixer => Box::new(crate::graph_modules::GraphStereoMixer::new(4)),
            ModuleType::Slew => Box::new(crate::graph_modules::GraphSlewGen::new(0.1, 0.1)),
            ModuleType::Seq8 => Box::new(crate::graph_modules::GraphSeq8::new()),
            ModuleType::Visual => Box::new(crate::graph_modules::GraphVisual::new()),
            ModuleType::Mult => Box::new(crate::graph_modules::GraphMult::new()),
            ModuleType::Switch => Box::new(crate::graph_modules::GraphSwitch::new(4)),
            ModuleType::ClockDiv => Box::new(crate::graph_modules::GraphClockDiv::new(4)),
            ModuleType::SampleHold => Box::new(crate::graph_modules::GraphSampleHold::new()),
            ModuleType::Output => return None, // Not implemented
        };

        Some(ModuleInfo {
            name: module_type_name.to_string(),
            inputs: temp_module.inputs(),
            outputs: temp_module.outputs(),
        })
    }

    /// Validate all connections
    ///
    /// # Panics
    /// Panics if the graph mutex is poisoned
    #[must_use]
    pub fn validate_connections(&self) -> Vec<String> {
        self.graph.lock().unwrap().validate_connections()
    }

    /// Activate all manual gate modules
    /// Activate all manual gate modules
    ///
    /// # Panics
    /// Panics if the graph mutex is poisoned
    #[must_use]
    pub fn activate_manual_gates(&self) -> usize {
        self.graph.lock().unwrap().activate_manual_gates()
    }

    /// Release all manual gate modules
    /// Release all manual gate modules
    ///
    /// # Panics
    /// Panics if the graph mutex is poisoned
    #[must_use]
    pub fn release_manual_gates(&self) -> usize {
        self.graph.lock().unwrap().release_manual_gates()
    }

    /// Add an observer to the graph for monitoring
    ///
    /// # Panics
    /// Panics if the graph mutex is poisoned
    #[allow(dead_code)] // Used by test framework
    pub fn add_observer(&self, observer: Box<dyn SignalObserver>) {
        self.graph.lock().unwrap().add_observer(observer);
    }

    /// Process the graph directly for testing (without audio output)
    ///
    /// # Panics
    /// Panics if the graph mutex is poisoned
    #[allow(dead_code)] // Used by test framework
    pub fn process_for_test(&self, sample_count: usize) {
        self.graph.lock().unwrap().process(sample_count);
    }

    /// Get access to the observer manager for test inspection
    ///
    /// # Panics
    /// Panics if the graph mutex is poisoned
    #[allow(dead_code)] // Used by test framework
    pub fn observer_manager_mut(&self) -> std::sync::MutexGuard<'_, crate::graph::GraphExecutor> {
        self.graph.lock().unwrap()
    }

    /// Build an audio stream for the given sample format
    ///
    /// # Errors
    /// Returns an error if the audio stream cannot be created
    fn build_stream<T>(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        graph: Arc<Mutex<GraphExecutor>>,
        output_module: Option<String>,
        is_stereo: bool,
    ) -> Result<cpal::Stream>
    where
        T: cpal::Sample + cpal::SizedSample + cpal::FromSample<f32>,
    {
        let channels = config.channels as usize;

        let err_fn = |err| eprintln!("Audio stream error: {err}");

        let stream = device.build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                // Fill with silence by default
                for sample in data.iter_mut() {
                    *sample = T::EQUILIBRIUM;
                }

                // Lock the graph for processing
                if let Ok(mut graph) = graph.lock() {
                    let samples_per_channel = data.len() / channels;

                    // Process the graph
                    graph.process(samples_per_channel);

                    // Get output from the designated module
                    if let Some(ref output_module) = output_module {
                        if is_stereo {
                            // Get stereo output
                            let left_buffer = graph.get_output(output_module, "left");
                            let right_buffer = graph.get_output(output_module, "right");

                            if let (Some(left), Some(right)) = (left_buffer, right_buffer) {
                                // Interleave stereo samples
                                for (i, frame) in data.chunks_mut(channels).enumerate() {
                                    if i < left.len() && i < right.len() {
                                        let left_sample = cpal::Sample::from_sample(left[i]);
                                        let right_sample = cpal::Sample::from_sample(right[i]);

                                        if channels >= 2 {
                                            frame[0] = left_sample;
                                            frame[1] = right_sample;
                                        } else {
                                            // Mono output - mix left and right
                                            let mixed = (left[i] + right[i]) * 0.5;
                                            frame[0] = cpal::Sample::from_sample(mixed);
                                        }
                                    }
                                }
                            }
                        } else {
                            // Legacy mono output
                            if let Some(buffer) = graph.get_output(output_module, "output") {
                                // Copy the output buffer to the audio stream
                                for (i, frame) in data.chunks_mut(channels).enumerate() {
                                    if i < buffer.len() {
                                        let value = buffer[i];
                                        let sample = cpal::Sample::from_sample(value);
                                        for channel_sample in frame.iter_mut() {
                                            *channel_sample = sample;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            err_fn,
            None, // No timeout
        )?;

        Ok(stream)
    }
}
