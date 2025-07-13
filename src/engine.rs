//! Audio engine for the zim-dsp modular synthesizer.
//!
//! This module manages the audio graph, module connections, and real-time processing.

use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use fundsp::prelude::{pan, saw_hz, sine_hz, square_hz, triangle_hz, zero, AudioUnit};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::modules::{Module, ModuleType};
use crate::parser::{parse_line, Command};

/// The main audio engine that manages modules, connections, and audio processing.
pub struct Engine {
    modules: HashMap<String, Box<dyn Module>>,
    connections: Vec<Connection>,
    audio_graph: Option<Box<dyn AudioUnit>>,
    stream: Option<cpal::Stream>,
    is_running: bool,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // TODO: Will be used when graph building is implemented
struct Connection {
    from_module: String,
    from_output: String,
    to_module: String,
    to_input: String,
}

impl Engine {
    /// Create a new audio engine instance.
    #[must_use]
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
            connections: Vec::new(),
            audio_graph: None,
            stream: None,
            is_running: false,
        }
    }

    /// Load a patch from text content, parsing and building the audio graph.
    pub fn load_patch(&mut self, patch_content: &str) -> Result<()> {
        self.clear_patch();

        for line in patch_content.lines() {
            let line = line.trim();
            if !line.is_empty() && !line.starts_with('#') {
                self.process_line(line)?;
            }
        }

        self.rebuild_graph();
        Ok(())
    }

    /// Process a single line of DSL code.
    pub fn process_line(&mut self, line: &str) -> Result<String> {
        let command = parse_line(line)?;

        match &command {
            Command::CreateModule { name, module_type, params } => {
                self.create_module(name.clone(), *module_type, params)?;
                Ok(format!("Created module: {command}"))
            }
            Command::Connect { from, to } => {
                self.add_connection(from.clone(), to.clone());
                Ok(format!("Connected: {command}"))
            }
            Command::SetParam { module, param, value } => {
                self.set_parameter(module, param, *value)?;
                Ok(format!("Set parameter: {command}"))
            }
        }
    }

    /// Clear all modules and connections, resetting the engine.
    pub fn clear_patch(&mut self) {
        self.stop();
        self.modules.clear();
        self.connections.clear();
        self.audio_graph = None;
    }

    /// Start audio processing.
    pub fn start(&mut self) -> Result<()> {
        if self.is_running {
            return Ok(());
        }

        // Rebuild graph if needed
        if self.audio_graph.is_none() {
            self.rebuild_graph();
        }

        // Initialize audio stream
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow!("No output device available"))?;
        let config = device.default_output_config()?;

        // For now, just play a test tone
        let test_graph = (sine_hz(440.0) * 0.1) >> pan(0.0);

        let graph = Box::new(test_graph);

        let stream_config: cpal::StreamConfig = config.into();
        self.stream = Some(Self::run_output(&device, &stream_config, graph)?);
        self.is_running = true;

        Ok(())
    }

    /// Stop audio processing.
    pub fn stop(&mut self) {
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }
        self.is_running = false;
    }

    fn create_module(
        &mut self,
        name: String,
        module_type: ModuleType,
        params: &[f32],
    ) -> Result<()> {
        use crate::modules::create_module;

        let module = create_module(module_type, params)?;
        self.modules.insert(name, module);

        Ok(())
    }

    fn add_connection(&mut self, from: String, to: String) {
        // For now, just store the connection
        // Later we'll parse module.output syntax
        self.connections.push(Connection {
            from_module: from,
            from_output: "out".to_string(),
            to_module: to,
            to_input: "in".to_string(),
        });
    }

    fn set_parameter(&mut self, module: &str, param: &str, value: f32) -> Result<()> {
        if let Some(module_ref) = self.modules.get_mut(module) {
            module_ref.set_param(param, value)?;
        } else {
            return Err(anyhow!("Module '{module}' not found"));
        }

        Ok(())
    }

    fn rebuild_graph(&mut self) {
        // Build the audio graph from modules and connections

        // Find what connects to the output
        let output_sources: Vec<_> =
            self.connections.iter().filter(|conn| conn.to_module == "out").collect();

        if output_sources.is_empty() {
            // No output connection, create silence
            let graph = zero() >> pan(0.0);
            self.audio_graph = Some(Box::new(graph));
            return;
        }

        // For now, handle simple case: single source -> output
        if let Some(output_conn) = output_sources.first() {
            let source_name = &output_conn.from_module;

            // Look up the source module
            if let Some(module) = self.modules.get(source_name) {
                // Check module type
                if module.module_type() == ModuleType::Oscillator {
                    if let Some(osc_info) = module.as_oscillator() {
                        let graph: Box<dyn AudioUnit> = match osc_info.waveform.as_str() {
                            "saw" => Box::new((saw_hz(osc_info.frequency) * 0.1) >> pan(0.0)),
                            "square" => Box::new((square_hz(osc_info.frequency) * 0.1) >> pan(0.0)),
                            "triangle" | "tri" => {
                                Box::new((triangle_hz(osc_info.frequency) * 0.1) >> pan(0.0))
                            }
                            _ => Box::new((sine_hz(osc_info.frequency) * 0.1) >> pan(0.0)),
                        };

                        self.audio_graph = Some(graph);
                        return;
                    }
                }
            }
        }

        // Fallback to test tone if we can't build the graph
        let graph = (sine_hz(440.0) * 0.1) >> pan(0.0);
        self.audio_graph = Some(Box::new(graph));
    }

    // Audio output handling (from fundsp examples)
    fn run_output(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        mut graph: Box<dyn AudioUnit>,
    ) -> Result<cpal::Stream> {
        let sample_rate = f64::from(config.sample_rate.0);
        graph.set_sample_rate(sample_rate);
        graph.reset();

        let graph = Arc::new(Mutex::new(graph));

        let stream = device.build_output_stream(
            config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                for frame in data.chunks_mut(2) {
                    let (left, right) = graph.lock().unwrap().get_stereo();
                    frame[0] = left;
                    frame[1] = right;
                }
            },
            move |err| eprintln!("Audio stream error: {err}"),
            None,
        )?;

        stream.play()?;
        Ok(stream)
    }
}
