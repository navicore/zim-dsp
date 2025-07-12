use anyhow::{Result, anyhow};
use fundsp::prelude::*;

/// Trait for all modules in the system
pub trait Module: Send {
    /// Process audio/control signals
    fn process(&mut self, inputs: &[f64], outputs: &mut [f64]);
    
    /// Set a parameter by name
    fn set_param(&mut self, name: &str, value: f32) -> Result<()>;
    
    /// Get current parameter value
    fn get_param(&self, name: &str) -> Option<f32>;
    
    /// Get module info
    fn info(&self) -> ModuleInfo;
}

#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub name: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub params: Vec<(String, f32, f32)>, // (name, min, max)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModuleType {
    Oscillator,
    Filter,
    Envelope,
    Vca,
    Mixer,
    Output,
}

impl std::fmt::Display for ModuleType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModuleType::Oscillator => write!(f, "osc"),
            ModuleType::Filter => write!(f, "filter"),
            ModuleType::Envelope => write!(f, "env"),
            ModuleType::Vca => write!(f, "vca"),
            ModuleType::Mixer => write!(f, "mix"),
            ModuleType::Output => write!(f, "out"),
        }
    }
}

/// Parse module type from string
pub fn parse_module_type(s: &str) -> Result<ModuleType> {
    match s {
        "osc" => Ok(ModuleType::Oscillator),
        "filter" => Ok(ModuleType::Filter),
        "env" => Ok(ModuleType::Envelope),
        "vca" => Ok(ModuleType::Vca),
        "mix" | "mixer" => Ok(ModuleType::Mixer),
        "out" | "output" => Ok(ModuleType::Output),
        _ => Err(anyhow!("Unknown module type: {}", s)),
    }
}

/// Create a module instance
pub fn create_module(module_type: ModuleType, params: Vec<f32>) -> Result<Box<dyn Module>> {
    match module_type {
        ModuleType::Oscillator => Ok(Box::new(Oscillator::new(params)?)),
        _ => Err(anyhow!("Module type {:?} not yet implemented", module_type)),
    }
}

// Example oscillator module implementation
pub struct Oscillator {
    frequency: f32,
    waveform: String,
    phase: f64,
    sample_rate: f64,
}

impl Oscillator {
    fn new(params: Vec<f32>) -> Result<Self> {
        let frequency = params.get(0).copied().unwrap_or(440.0);
        
        Ok(Oscillator {
            frequency,
            waveform: "sine".to_string(),
            phase: 0.0,
            sample_rate: 44100.0,
        })
    }
}

impl Module for Oscillator {
    fn process(&mut self, _inputs: &[f64], outputs: &mut [f64]) {
        if outputs.is_empty() {
            return;
        }
        
        let phase_inc = self.frequency as f64 / self.sample_rate;
        
        for output in outputs.iter_mut() {
            *output = match self.waveform.as_str() {
                "sine" => (self.phase * 2.0 * std::f64::consts::PI).sin(),
                "saw" => (self.phase * 2.0) - 1.0,
                "square" => if self.phase < 0.5 { 1.0 } else { -1.0 },
                _ => 0.0,
            };
            
            self.phase += phase_inc;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
        }
    }
    
    fn set_param(&mut self, name: &str, value: f32) -> Result<()> {
        match name {
            "freq" | "frequency" => {
                self.frequency = value;
                Ok(())
            }
            _ => Err(anyhow!("Unknown parameter: {}", name)),
        }
    }
    
    fn get_param(&self, name: &str) -> Option<f32> {
        match name {
            "freq" | "frequency" => Some(self.frequency),
            _ => None,
        }
    }
    
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            name: "Oscillator".to_string(),
            inputs: vec![],
            outputs: vec!["out".to_string()],
            params: vec![
                ("frequency".to_string(), 20.0, 20000.0),
            ],
        }
    }
}