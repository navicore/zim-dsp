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

    /// Get filter-specific info (returns None for non-filters)
    fn as_filter(&self) -> Option<FilterInfo> {
        None
    }

    /// Get envelope-specific info (returns None for non-envelopes)
    fn as_envelope(&self) -> Option<EnvelopeInfo> {
        None
    }

    /// Get VCA-specific info (returns None for non-VCAs)
    fn as_vca(&self) -> Option<VcaInfo> {
        None
    }
}

/// Information about an oscillator
#[derive(Debug, Clone)]
pub struct OscillatorInfo {
    pub frequency: f32,
    pub waveform: String,
}

/// Information about a filter
#[derive(Debug, Clone)]
pub struct FilterInfo {
    pub cutoff: f32,
    #[allow(dead_code)] // Will be used when proper filtering is implemented
    pub resonance: f32,
    #[allow(dead_code)] // Will be used when proper filtering is implemented
    pub filter_type: String,
}

/// Information about an envelope
#[derive(Debug, Clone)]
pub struct EnvelopeInfo {
    #[allow(dead_code)] // Will be used when complex routing is implemented
    pub attack: f32,
    #[allow(dead_code)] // Will be used when complex routing is implemented
    pub decay: f32,
    #[allow(dead_code)] // Will be used when complex routing is implemented
    pub current_value: f32,
}

/// Information about a VCA
#[derive(Debug, Clone)]
pub struct VcaInfo {
    #[allow(dead_code)] // Will be used when complex routing is implemented
    pub gain: f32,
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
        ModuleType::Filter => Ok(Box::new(Filter::new(params))),
        ModuleType::Envelope => Ok(Box::new(Envelope::new(params))),
        ModuleType::Vca => Ok(Box::new(Vca::new(params))),
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
    #[allow(clippy::cast_possible_truncation)]
    fn new(params: &[f32]) -> Self {
        let mut frequency = 440.0;
        let mut waveform = "sine".to_string();

        // Check if first param is waveform encoding (negative number)
        if let Some(&first) = params.first() {
            if first < 0.0 {
                // It's a waveform encoding
                waveform = match first as i32 {
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
    #[allow(dead_code)]
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

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
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

/// Basic filter module
pub struct Filter {
    cutoff: f32,
    resonance: f32,
    mode: String,
}

impl Filter {
    fn new(params: &[f32]) -> Self {
        let cutoff = params.first().copied().unwrap_or(1000.0);
        let resonance = params.get(1).copied().unwrap_or(0.5);

        Self {
            cutoff,
            resonance,
            mode: "lowpass".to_string(),
        }
    }
}

impl Module for Filter {
    fn process(&mut self, _inputs: &[f64], outputs: &mut [f64]) {
        // For now, just pass through
        // TODO: Implement actual filtering
        for output in outputs.iter_mut() {
            *output = 0.0;
        }
    }

    fn set_param(&mut self, name: &str, value: f32) -> Result<()> {
        match name {
            "cutoff" | "freq" => {
                self.cutoff = value.clamp(20.0, 20_000.0);
                Ok(())
            }
            "resonance" | "res" | "q" => {
                self.resonance = value.clamp(0.0, 1.0);
                Ok(())
            }
            _ => Err(anyhow!("Unknown parameter: {name}")),
        }
    }

    fn get_param(&self, name: &str) -> Option<f32> {
        match name {
            "cutoff" | "freq" => Some(self.cutoff),
            "resonance" | "res" | "q" => Some(self.resonance),
            _ => None,
        }
    }

    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            name: "Filter".to_string(),
            inputs: vec!["in".to_string()],
            outputs: vec!["out".to_string()],
            params: vec![
                ("cutoff".to_string(), 20.0, 20_000.0),
                ("resonance".to_string(), 0.0, 1.0),
            ],
        }
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Filter
    }

    fn as_oscillator(&self) -> Option<OscillatorInfo> {
        None
    }

    fn as_filter(&self) -> Option<FilterInfo> {
        Some(FilterInfo {
            cutoff: self.cutoff,
            resonance: self.resonance,
            filter_type: self.mode.clone(),
        })
    }
}

/// Basic envelope (AD) module
pub struct Envelope {
    attack_time: f32, // in seconds
    decay_time: f32,  // in seconds
    current_value: f32,
    phase: EnvelopePhase,
    sample_rate: f64,
    phase_time: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum EnvelopePhase {
    Idle,
    Attack,
    Decay,
}

impl Envelope {
    fn new(params: &[f32]) -> Self {
        let attack_time = params.first().copied().unwrap_or(0.01); // 10ms default
        let decay_time = params.get(1).copied().unwrap_or(0.1); // 100ms default

        Self {
            attack_time,
            decay_time,
            current_value: 0.0,
            phase: EnvelopePhase::Idle,
            sample_rate: 44_100.0,
            phase_time: 0.0,
        }
    }

    /// Trigger the envelope
    pub fn trigger(&mut self) {
        self.phase = EnvelopePhase::Attack;
        self.phase_time = 0.0;
    }
}

impl Module for Envelope {
    #[allow(clippy::cast_possible_truncation)]
    fn process(&mut self, _inputs: &[f64], outputs: &mut [f64]) {
        if outputs.is_empty() {
            return;
        }

        for output in outputs.iter_mut() {
            match self.phase {
                EnvelopePhase::Idle => {
                    self.current_value = 0.0;
                }
                EnvelopePhase::Attack => {
                    if self.attack_time > 0.0 {
                        self.current_value =
                            (self.phase_time / f64::from(self.attack_time)).min(1.0) as f32;
                        if self.phase_time >= f64::from(self.attack_time) {
                            self.phase = EnvelopePhase::Decay;
                            self.phase_time = 0.0;
                        }
                    } else {
                        self.current_value = 1.0;
                        self.phase = EnvelopePhase::Decay;
                        self.phase_time = 0.0;
                    }
                }
                EnvelopePhase::Decay => {
                    if self.decay_time > 0.0 {
                        self.current_value =
                            (1.0 - (self.phase_time / f64::from(self.decay_time)).min(1.0)) as f32;
                        if self.phase_time >= f64::from(self.decay_time) {
                            self.phase = EnvelopePhase::Idle;
                            self.phase_time = 0.0;
                        }
                    } else {
                        self.current_value = 0.0;
                        self.phase = EnvelopePhase::Idle;
                        self.phase_time = 0.0;
                    }
                }
            }

            *output = f64::from(self.current_value);

            // Advance time
            self.phase_time += 1.0 / self.sample_rate;
        }
    }

    fn set_param(&mut self, name: &str, value: f32) -> Result<()> {
        match name {
            "attack" | "a" => {
                self.attack_time = value.max(0.0);
                Ok(())
            }
            "decay" | "d" => {
                self.decay_time = value.max(0.0);
                Ok(())
            }
            "trigger" | "gate" => {
                if value > 0.0 {
                    self.trigger();
                }
                Ok(())
            }
            _ => Err(anyhow!("Unknown parameter: {name}")),
        }
    }

    fn get_param(&self, name: &str) -> Option<f32> {
        match name {
            "attack" | "a" => Some(self.attack_time),
            "decay" | "d" => Some(self.decay_time),
            "value" => Some(self.current_value),
            _ => None,
        }
    }

    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            name: "Envelope".to_string(),
            inputs: vec!["gate".to_string()],
            outputs: vec!["out".to_string()],
            params: vec![("attack".to_string(), 0.0, 10.0), ("decay".to_string(), 0.0, 10.0)],
        }
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Envelope
    }

    fn as_envelope(&self) -> Option<EnvelopeInfo> {
        Some(EnvelopeInfo {
            attack: self.attack_time,
            decay: self.decay_time,
            current_value: self.current_value,
        })
    }
}

/// Basic VCA (Voltage Controlled Amplifier) module
pub struct Vca {
    gain: f32,
}

impl Vca {
    fn new(params: &[f32]) -> Self {
        let gain = params.first().copied().unwrap_or(1.0);

        Self { gain }
    }
}

impl Module for Vca {
    fn process(&mut self, inputs: &[f64], outputs: &mut [f64]) {
        if outputs.is_empty() || inputs.is_empty() {
            return;
        }

        // VCA multiplies audio input by control voltage (CV) and gain
        // For now, we'll assume:
        // inputs[0] = audio signal
        // inputs[1] = control voltage (if available)

        for (i, output) in outputs.iter_mut().enumerate() {
            let audio = inputs.get(i % inputs.len()).copied().unwrap_or(0.0);
            let cv = if inputs.len() > 1 {
                inputs.get(inputs.len() / 2 + i % (inputs.len() / 2)).copied().unwrap_or(1.0)
            } else {
                1.0
            };

            *output = audio * cv * f64::from(self.gain);
        }
    }

    fn set_param(&mut self, name: &str, value: f32) -> Result<()> {
        match name {
            "gain" | "level" => {
                self.gain = value.max(0.0);
                Ok(())
            }
            _ => Err(anyhow!("Unknown parameter: {name}")),
        }
    }

    fn get_param(&self, name: &str) -> Option<f32> {
        match name {
            "gain" | "level" => Some(self.gain),
            _ => None,
        }
    }

    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            name: "VCA".to_string(),
            inputs: vec!["audio".to_string(), "cv".to_string()],
            outputs: vec!["out".to_string()],
            params: vec![("gain".to_string(), 0.0, 2.0)],
        }
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Vca
    }

    fn as_vca(&self) -> Option<VcaInfo> {
        Some(VcaInfo { gain: self.gain })
    }
}
