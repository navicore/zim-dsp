use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use fundsp::prelude::*;

use crate::modules::{Module, ModuleType};
use crate::parser::{parse_line, Command};

pub struct Engine {
    modules: HashMap<String, Box<dyn Module>>,
    connections: Vec<Connection>,
    audio_graph: Option<Box<dyn AudioUnit64>>,
    stream: Option<cpal::Stream>,
    is_running: bool,
}

#[derive(Debug, Clone)]
struct Connection {
    from_module: String,
    from_output: String,
    to_module: String,
    to_input: String,
}

impl Engine {
    pub fn new() -> Result<Self> {
        Ok(Engine {
            modules: HashMap::new(),
            connections: Vec::new(),
            audio_graph: None,
            stream: None,
            is_running: false,
        })
    }
    
    pub fn load_patch(&mut self, patch_content: &str) -> Result<()> {
        self.clear_patch();
        
        for line in patch_content.lines() {
            let line = line.trim();
            if !line.is_empty() && !line.starts_with('#') {
                self.process_line(line)?;
            }
        }
        
        self.rebuild_graph()?;
        Ok(())
    }
    
    pub fn process_line(&mut self, line: &str) -> Result<String> {
        let command = parse_line(line)?;
        
        match command {
            Command::CreateModule { name, module_type, params } => {
                self.create_module(name, module_type, params)?;
                Ok(format!("Created module: {}", command))
            }
            Command::Connect { from, to } => {
                self.add_connection(from, to)?;
                Ok(format!("Connected: {}", command))
            }
            Command::SetParam { module, param, value } => {
                self.set_parameter(module, param, value)?;
                Ok(format!("Set parameter: {}", command))
            }
        }
    }
    
    pub fn clear_patch(&mut self) {
        self.stop();
        self.modules.clear();
        self.connections.clear();
        self.audio_graph = None;
    }
    
    pub fn start(&mut self) -> Result<()> {
        if self.is_running {
            return Ok(());
        }
        
        // Rebuild graph if needed
        if self.audio_graph.is_none() {
            self.rebuild_graph()?;
        }
        
        // Initialize audio stream
        let host = cpal::default_host();
        let device = host.default_output_device()
            .ok_or_else(|| anyhow!("No output device available"))?;
        let config = device.default_output_config()?;
        
        // For now, just play a test tone
        let sample_rate = config.sample_rate().0 as f64;
        let test_graph = sine_hz(440.0) * 0.1 >> pan(0.0);
        
        let mut graph = Box::new(test_graph);
        
        self.stream = Some(self.run_output(device, config.into(), graph)?);
        self.is_running = true;
        
        Ok(())
    }
    
    pub fn stop(&mut self) {
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }
        self.is_running = false;
    }
    
    fn create_module(&mut self, name: String, module_type: ModuleType, params: Vec<f32>) -> Result<()> {
        use crate::modules::create_module;
        
        let module = create_module(module_type, params)?;
        self.modules.insert(name, module);
        
        Ok(())
    }
    
    fn add_connection(&mut self, from: String, to: String) -> Result<()> {
        // For now, just store the connection
        // Later we'll parse module.output syntax
        self.connections.push(Connection {
            from_module: from.clone(),
            from_output: "out".to_string(),
            to_module: to.clone(),
            to_input: "in".to_string(),
        });
        
        Ok(())
    }
    
    fn set_parameter(&mut self, module: String, param: String, value: f32) -> Result<()> {
        if let Some(module) = self.modules.get_mut(&module) {
            module.set_param(&param, value)?;
        } else {
            return Err(anyhow!("Module '{}' not found", module));
        }
        
        Ok(())
    }
    
    fn rebuild_graph(&mut self) -> Result<()> {
        // TODO: Build actual graph from modules and connections
        // For now, just create a simple test graph
        let graph = sine_hz(440.0) * 0.1 >> pan(0.0);
        self.audio_graph = Some(Box::new(graph));
        
        Ok(())
    }
    
    // Audio output handling (from fundsp examples)
    fn run_output(
        &self,
        device: cpal::Device,
        config: cpal::StreamConfig,
        mut graph: Box<dyn AudioUnit64>,
    ) -> Result<cpal::Stream> {
        let sample_rate = config.sample_rate.0 as f64;
        graph.set_sample_rate(sample_rate);
        graph.reset();
        
        let graph = Arc::new(Mutex::new(graph));
        
        let stream = device.build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let mut graph = graph.lock().unwrap();
                for frame in data.chunks_mut(2) {
                    let (left, right) = graph.get_stereo();
                    frame[0] = left as f32;
                    frame[1] = right as f32;
                }
            },
            move |err| eprintln!("Audio stream error: {}", err),
            None,
        )?;
        
        stream.play()?;
        Ok(stream)
    }
}