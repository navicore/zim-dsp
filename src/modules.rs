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
    Slew,
    Seq8,
    Visual,
    Mult,
    Switch,
    ClockDiv,
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
            Self::Slew => write!(f, "slew"),
            Self::Seq8 => write!(f, "seq8"),
            Self::Visual => write!(f, "visual"),
            Self::Mult => write!(f, "mult"),
            Self::Switch => write!(f, "switch"),
            Self::ClockDiv => write!(f, "clockdiv"),
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
        "slew" | "slew_gen" => Ok(ModuleType::Slew),
        "seq8" | "sequencer" => Ok(ModuleType::Seq8),
        "visual" | "scope" | "debug" => Ok(ModuleType::Visual),
        "mult" | "multiple" => Ok(ModuleType::Mult),
        "switch" | "seq_switch" => Ok(ModuleType::Switch),
        "clockdiv" | "clock_div" | "divider" => Ok(ModuleType::ClockDiv),
        _ => Err(anyhow!("Unknown module type: {s}")),
    }
}
