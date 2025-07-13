//! Module system for the zim-dsp modular synthesizer.
//!
//! This module defines the trait for audio modules and provides implementations
//! for basic synthesis modules like oscillators, filters, and envelopes.

use anyhow::{anyhow, Result};

/// Trait for all modules in the system
#[allow(dead_code)] // TODO: Will be used when graph building is implemented
pub trait Module: Send {
    /// Process audio/control signals
    fn process(&mut self, inputs: &[f64], outputs: &mut [f64]);

    /// Set a parameter by name
    fn set_param(&mut self, name: &str, value: f32) -> Result<()>;

    /// Get current parameter value
    fn get_param(&self, name: &str) -> Option<f32>;

    /// Get module info
    fn info(&self) -> ModuleInfo;
    
    /// Get the module type
    fn module_type(&self) -> ModuleType;
    
    /// Get oscillator-specific info (returns None for non-oscillators)
    fn as_oscillator(&self) -> Option<OscillatorInfo> {
        None
    }
}

/// Information about an oscillator
#[derive(Debug, Clone)]
pub struct OscillatorInfo {
    pub frequency: f32,
    pub waveform: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // TODO: Will be used when modules are fully implemented
pub struct ModuleInfo {
    pub name: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub params: Vec<(String, f32, f32)>, // (name, min, max)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
            Self::Oscillator => write!(f, "osc"),
            Self::Filter => write!(f, "filter"),
            Self::Envelope => write!(f, "env"),
            Self::Vca => write!(f, "vca"),
            Self::Mixer => write!(f, "mix"),
            Self::Output => write!(f, "out"),
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
        _ => Err(anyhow!("Unknown module type: {s}")),
    }
}

/// Create a module instance
pub fn create_module(module_type: ModuleType, params: &[f32]) -> Result<Box<dyn Module>> {
    match module_type {
        ModuleType::Oscillator => Ok(Box::new(Oscillator::new(params))),
        _ => Err(anyhow!("Module type {module_type:?} not yet implemented")),
    }
}

/// Basic oscillator module implementation.
///
/// Supports sine, saw, and square waveforms.
pub struct Oscillator {
    frequency: f32,
    waveform: String,
    phase: f64,
    sample_rate: f64,
}

impl Oscillator {
    fn new(params: &[f32]) -> Self {
        let mut frequency = 440.0;
        let mut waveform = "sine".to_string();
        
        // Check if first param is waveform encoding (negative number)
        if let Some(&first) = params.first() {
            if first < 0.0 {
                // It's a waveform encoding
                waveform = match first as i32 {
                    -1 => "sine".to_string(),
                    -2 => "saw".to_string(),
                    -3 => "square".to_string(),
                    -4 => "triangle".to_string(),
                    _ => "sine".to_string(),
                };
                // Get frequency from second param
                frequency = params.get(1).copied().unwrap_or(440.0);
            } else {
                // First param is frequency
                frequency = first;
            }
        }
        
        Self {
            frequency,
            waveform,
            phase: 0.0,
            sample_rate: 44_100.0,
        }
    }
    
    /// Get the waveform type
    pub fn waveform(&self) -> &str {
        &self.waveform
    }
}

impl Module for Oscillator {
    fn process(&mut self, _inputs: &[f64], outputs: &mut [f64]) {
        if outputs.is_empty() {
            return;
        }

        let phase_inc = f64::from(self.frequency) / self.sample_rate;

        for output in outputs.iter_mut() {
            *output = match self.waveform.as_str() {
                "sine" => (self.phase * 2.0 * std::f64::consts::PI).sin(),
                "saw" => self.phase.mul_add(2.0, -1.0),
                "square" => {
                    if self.phase < 0.5 {
                        1.0
                    } else {
                        -1.0
                    }
                }
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
            "waveform" => {
                // For now, validate against known waveforms
                match value as u32 {
                    0 => self.waveform = "sine".to_string(),
                    1 => self.waveform = "saw".to_string(),
                    2 => self.waveform = "square".to_string(),
                    _ => return Err(anyhow!("Invalid waveform index")),
                }
                Ok(())
            }
            _ => Err(anyhow!("Unknown parameter: {name}")),
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
            params: vec![("frequency".to_string(), 20.0, 20_000.0)],
        }
    }
    
    fn module_type(&self) -> ModuleType {
        ModuleType::Oscillator
    }
    
    fn as_oscillator(&self) -> Option<OscillatorInfo> {
        Some(OscillatorInfo {
            frequency: self.frequency,
            waveform: self.waveform.clone(),
        })
    }
}
