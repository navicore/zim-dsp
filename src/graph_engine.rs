//! Graph-based audio engine for the REPL

use crate::graph::{Connection, ConnectionExpr, GraphExecutor, ModuleInfo};
use crate::graph_modules::{
    GraphClockDiv, GraphEnvelope, GraphFilter, GraphLfo, GraphManualGate, GraphMonoMixer,
    GraphMult, GraphNoiseGen, GraphOscillator, GraphSampleHold, GraphSeq8, GraphSlewGen,
    GraphStereoMixer, GraphStereoOutput, GraphSwitch, GraphVca, GraphVisual,
};
use crate::modules::ModuleType;
use crate::observability::SignalObserver;
use crate::parser::{parse_line, Command};
use crate::user_modules::UserModuleRegistry;
use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
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
    // User module registry
    user_modules: UserModuleRegistry,
}

impl Default for GraphEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphEngine {
    #[must_use]
    pub fn new() -> Self {
        Self::new_with_patch_context(None)
    }

    #[must_use]
    pub fn new_with_patch_context(patch_file: Option<&str>) -> Self {
        let mut user_modules = UserModuleRegistry::new();

        // Load user modules using search hierarchy
        Self::load_user_modules_with_search(&mut user_modules, patch_file);

        Self {
            graph: Arc::new(Mutex::new(GraphExecutor::new())),
            stream: None,
            is_running: false,
            sample_rate: 44100.0,
            output_module: None,
            output_port: None,
            has_stereo_output: false,
            user_modules,
        }
    }

    /// Load user modules using a search hierarchy of directories
    fn load_user_modules_with_search(
        user_modules: &mut crate::user_modules::UserModuleRegistry,
        patch_file: Option<&str>,
    ) {
        let mut search_paths = Vec::new();

        // 1. Current directory
        search_paths.push("usermodules".to_string());

        // 2. Same directory as patch file (if provided)
        if let Some(patch_path) = patch_file {
            if let Some(patch_dir) = std::path::Path::new(patch_path).parent() {
                let patch_usermodules = patch_dir.join("usermodules");
                search_paths.push(patch_usermodules.to_string_lossy().to_string());
            }
        }

        // 3. User home directory
        if let Some(home_dir) = dirs::home_dir() {
            let home_usermodules = home_dir.join(".zim-dsp").join("usermodules");
            search_paths.push(home_usermodules.to_string_lossy().to_string());
        }

        // Try each search path in order
        let mut total_loaded = 0;
        let mut loaded_from = Vec::new();

        for path in &search_paths {
            if let Ok(count) = user_modules.scan_directory(path) {
                if count > 0 {
                    total_loaded += count;
                    loaded_from.push(format!("{path} ({count} modules)"));
                }
            }
        }

        if total_loaded > 0 {
            println!("Loaded {total_loaded} user modules from: {}", loaded_from.join(", "));
        } else if !search_paths.is_empty() {
            println!("No user modules found. Searched: {}", search_paths.join(", "));
        }
    }

    /// Load a patch from text content
    /// Load a patch from text content
    ///
    /// # Errors
    /// Returns an error if any line in the patch fails to parse or process
    pub fn load_patch(&mut self, patch_content: &str) -> Result<()> {
        self.clear_patch();

        // Phase 1: Preprocess to expand user modules
        let expanded_patch = self.preprocess_patch(patch_content);

        // Phase 2: Process expanded patch normally
        for line in expanded_patch.lines() {
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
        // Check for user module creation before trying built-in parser
        if let Ok(result) = self.try_process_user_module(line) {
            return Ok(result);
        }

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
        }
    }

    fn handle_extended_syntax(&self, line: &str) -> Result<String> {
        // Handle new syntax patterns

        // Pattern: source.port -> module.port
        if line.contains(" -> ") {
            let parts: Vec<&str> = line.split(" -> ").collect();
            if parts.len() == 2 {
                let source = parts[0].trim();
                let dest = parts[1].trim();

                return self.parse_connection(dest, source);
            }
        }

        // Pattern: module.port <- source.port
        if line.contains(" <- ") {
            let parts: Vec<&str> = line.split(" <- ").collect();
            if parts.len() == 2 {
                let dest = parts[0].trim();
                let source = parts[1].trim();

                return self.parse_connection(dest, source);
            }
        }

        Err(anyhow!("Unrecognized syntax: {}", line))
    }

    /// Try to process a line as a user module creation
    ///
    /// # Errors
    /// Returns an error if the line is not a user module or processing fails
    fn try_process_user_module(&mut self, line: &str) -> Result<String> {
        let trimmed = line.trim();

        // Check if this looks like a module creation line
        if let Some(colon_pos) = trimmed.find(':') {
            let name = trimmed[..colon_pos].trim();
            let rest = trimmed[colon_pos + 1..].trim();

            // Extract the module type (first word after colon)
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if let Some(module_type_str) = parts.first() {
                // Check if this is a user module type
                if let Some(template) = self.user_modules.get(module_type_str) {
                    // Clone the template to avoid borrowing issues
                    let template = template.clone();
                    // This is a user module - expand and process it
                    return self.process_user_module_expansion(name, &template);
                }
            }
        }

        // Not a user module
        Err(anyhow!("Not a user module"))
    }

    /// Process a user module expansion in REPL context
    ///
    /// # Errors
    /// Returns an error if template expansion or command processing fails
    fn process_user_module_expansion(
        &mut self,
        instance_name: &str,
        template: &crate::user_modules::UserModuleTemplate,
    ) -> Result<String> {
        // Expand the user module template
        let expanded_commands = template.expand(instance_name);

        let mut results = Vec::new();
        results.push(format!("# Expanding user module: {instance_name}"));

        // Process each expanded command
        for command in expanded_commands {
            let command_str = command.to_string();

            // Skip EXTERNAL_* placeholder connections for now
            if command_str.contains("EXTERNAL_INPUT_") || command_str.contains("EXTERNAL_OUTPUT_") {
                continue;
            }

            // Process the expanded command normally
            match self.handle_parsed_command(command) {
                Ok(result) => results.push(result),
                Err(e) => {
                    return Err(anyhow!(
                        "Error processing expanded command '{}': {}",
                        command_str,
                        e
                    ))
                }
            }
        }

        Ok(results.join("\n"))
    }

    fn parse_connection(&self, dest: &str, source_expr: &str) -> Result<String> {
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

    /// Inspect a user module by name
    #[must_use]
    pub fn inspect_user_module(&self, name: &str) -> Option<ModuleInfo> {
        self.user_modules.get(name).map(|template| {
            // Convert user module inputs/outputs to PortDescriptors
            let inputs = template
                .inputs
                .iter()
                .map(|name| crate::graph::PortDescriptor {
                    name: name.clone(),
                    default_value: 0.0,
                    description: format!("User module input: {name}"),
                })
                .collect();

            let outputs = template
                .outputs
                .iter()
                .map(|name| crate::graph::PortDescriptor {
                    name: name.clone(),
                    default_value: 0.0,
                    description: format!("User module output: {name}"),
                })
                .collect();

            ModuleInfo {
                name: format!("user:{}", template.name),
                inputs,
                outputs,
            }
        })
    }

    /// List all available user modules
    #[must_use]
    pub fn list_user_modules(&self) -> Vec<String> {
        self.user_modules.list_modules().iter().map(|s| (*s).clone()).collect()
    }

    /// Expand a patch with user modules for debugging (dry-run)
    #[must_use]
    pub fn expand_patch(&self, patch_content: &str) -> Vec<String> {
        let mut expanded_lines = Vec::new();

        for line in patch_content.lines() {
            let trimmed = line.trim();

            // Skip empty lines and comments
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Check if this looks like a module creation line
            if let Some(colon_pos) = trimmed.find(':') {
                let name = trimmed[..colon_pos].trim();
                let rest = trimmed[colon_pos + 1..].trim();

                // Extract the module type (first word after colon)
                let parts: Vec<&str> = rest.split_whitespace().collect();
                if let Some(module_type_str) = parts.first() {
                    // Check if this is a user module type first
                    if let Some(template) = self.user_modules.get(module_type_str) {
                        // This is a user module - expand it
                        expanded_lines.push(format!("# Expanding user module: {name}"));
                        let expanded_commands = template.expand(name);
                        for cmd in expanded_commands {
                            expanded_lines.push(cmd.to_string());
                        }
                    } else {
                        // Regular module or unparseable - keep as-is
                        expanded_lines.push(trimmed.to_string());
                    }
                } else {
                    // Malformed module creation line
                    expanded_lines.push(trimmed.to_string());
                }
            } else {
                // Not a module creation - keep as-is
                expanded_lines.push(trimmed.to_string());
            }
        }

        expanded_lines
    }

    /// Preprocess a patch to expand user modules and resolve external connections
    fn preprocess_patch(&self, patch_content: &str) -> String {
        let mut expanded_lines = Vec::new();
        let mut user_module_instances = std::collections::HashMap::new();

        // Phase 1: Expand user modules and collect instance info
        for line in patch_content.lines() {
            let trimmed = line.trim();

            // Skip empty lines and comments
            if trimmed.is_empty() || trimmed.starts_with('#') {
                expanded_lines.push(trimmed.to_string());
                continue;
            }

            // Check if this looks like a module creation line
            if let Some(colon_pos) = trimmed.find(':') {
                let name = trimmed[..colon_pos].trim();
                let rest = trimmed[colon_pos + 1..].trim();

                // Extract the module type (first word after colon)
                let parts: Vec<&str> = rest.split_whitespace().collect();
                if let Some(module_type_str) = parts.first() {
                    // Check if this is a user module type first
                    if let Some(template) = self.user_modules.get(module_type_str) {
                        // This is a user module - expand it
                        expanded_lines.push(format!("# Expanding user module: {name}"));
                        let expanded_commands = template.expand(name);
                        for cmd in expanded_commands {
                            expanded_lines.push(cmd.to_string());
                        }

                        // Store instance info for connection resolution
                        user_module_instances.insert(name.to_string(), template.clone());
                    } else {
                        // Regular module or unparseable - keep as-is
                        expanded_lines.push(trimmed.to_string());
                    }
                } else {
                    // Malformed module creation line
                    expanded_lines.push(trimmed.to_string());
                }
            } else {
                // Not a module creation - keep as-is for now
                expanded_lines.push(trimmed.to_string());
            }
        }

        // Phase 2: Resolve external connections
        let resolved_lines =
            Self::resolve_external_connections(expanded_lines, &user_module_instances);

        resolved_lines.join("\n")
    }

    /// Resolve external connection placeholders to actual connections
    fn resolve_external_connections(
        lines: Vec<String>,
        user_module_instances: &std::collections::HashMap<
            String,
            crate::user_modules::UserModuleTemplate,
        >,
    ) -> Vec<String> {
        let mut resolved_lines = Vec::new();

        for line in lines {
            if line.contains("EXTERNAL_INPUT_") || line.contains("EXTERNAL_OUTPUT_") {
                // This is a connection with external placeholders - skip for now
                // These will be resolved when we encounter the actual connections
                continue;
            }
            if line.contains(" -> ") || line.contains(" <- ") {
                // This might be a connection to/from a user module - resolve it
                resolved_lines.push(Self::resolve_connection_line(&line, user_module_instances));
            } else {
                // Regular line - keep as-is
                resolved_lines.push(line);
            }
        }

        resolved_lines
    }

    /// Resolve a single connection line with user module ports
    ///
    /// # Errors
    /// Returns an error if connection parsing fails
    fn resolve_connection_line(
        line: &str,
        user_module_instances: &std::collections::HashMap<
            String,
            crate::user_modules::UserModuleTemplate,
        >,
    ) -> String {
        // Check if this line contains connections to user module ports
        let mut resolved_line = line.to_string();

        // Parse connection direction
        if let Some(arrow_pos) = line.find(" -> ") {
            let from_part = line[..arrow_pos].trim();
            let to_part = line[arrow_pos + 4..].trim();

            let resolved_from = Self::resolve_module_port(from_part, user_module_instances, false);
            let resolved_to = Self::resolve_module_port(to_part, user_module_instances, true);

            resolved_line = format!("{resolved_from} -> {resolved_to}");
        } else if let Some(arrow_pos) = line.find(" <- ") {
            let to_part = line[..arrow_pos].trim();
            let from_part = line[arrow_pos + 4..].trim();

            let resolved_from = Self::resolve_module_port(from_part, user_module_instances, false);
            let resolved_to = Self::resolve_module_port(to_part, user_module_instances, true);

            resolved_line = format!("{resolved_to} <- {resolved_from}");
        }

        resolved_line
    }

    /// Resolve a module.port reference to internal implementation
    fn resolve_module_port(
        module_port: &str,
        user_module_instances: &std::collections::HashMap<
            String,
            crate::user_modules::UserModuleTemplate,
        >,
        is_input: bool,
    ) -> String {
        // Check if this is a user module port reference
        if let Some(dot_pos) = module_port.find('.') {
            let module_name = &module_port[..dot_pos];
            let port_name = &module_port[dot_pos + 1..];

            // Check if this module is a user module instance
            if let Some(template) = user_module_instances.get(module_name) {
                // Map user module port to internal implementation
                if is_input {
                    // This is an input to the user module
                    if template.inputs.contains(&port_name.to_string()) {
                        // For now, map directly to the first internal module
                        // This is a simplification - real implementation would need more sophisticated mapping
                        return format!("{module_name}_vca.audio");
                    }
                } else {
                    // This is an output from the user module
                    if template.outputs.contains(&port_name.to_string()) {
                        // For now, map directly to the first internal module
                        return format!("{module_name}_vca.out");
                    }
                }
            }
        }

        // Not a user module port, return as-is
        module_port.to_string()
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
