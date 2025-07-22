//! Graph-based audio engine for the REPL

use crate::graph::{Connection, ConnectionExpr, GraphExecutor, ModuleInfo};
use crate::graph_modules::{
    GraphClockDiv, GraphEnvelope, GraphFilter, GraphLfo, GraphManualGate, GraphMonoMixer,
    GraphMult, GraphNoiseGen, GraphOscillator, GraphSeq8, GraphSlewGen, GraphStereoMixer,
    GraphStereoOutput, GraphSwitch, GraphVca, GraphVisual,
};
use crate::modules::ModuleType;
use crate::observability::SignalObserver;
use crate::parser::{parse_line, Command};
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

    /// Validate all connections
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
