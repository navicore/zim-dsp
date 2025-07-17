//! Module types for the zim-dsp modular synthesizer.

use anyhow::{anyhow, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleType {
    Oscillator,
    Filter,
    Envelope,
    Vca,
    Mixer,
    Output,
    Lfo,
    ManualGate,
    StereoOutput,
    Noise,
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
            Self::Lfo => write!(f, "lfo"),
            Self::ManualGate => write!(f, "gate"),
            Self::StereoOutput => write!(f, "stereo_out"),
            Self::Noise => write!(f, "noise"),
        }
    }
}

/// Parse module type from string
pub fn parse_module_type(s: &str) -> Result<ModuleType> {
    match s {
        "osc" => Ok(ModuleType::Oscillator),
        "filter" => Ok(ModuleType::Filter),
        "env" | "envelope" => Ok(ModuleType::Envelope),
        "vca" => Ok(ModuleType::Vca),
        "mix" | "mixer" | "mono_mixer" => Ok(ModuleType::Mixer),
        "out" | "output" => Ok(ModuleType::Output),
        "lfo" => Ok(ModuleType::Lfo),
        "gate" | "manual" => Ok(ModuleType::ManualGate),
        "noise" | "noise_gen" => Ok(ModuleType::Noise),
        _ => Err(anyhow!("Unknown module type: {s}")),
    }
}
