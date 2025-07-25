//! Module types for the zim-dsp modular synthesizer.

use anyhow::{anyhow, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleType {
    Oscillator,
    Filter,
    Envelope,
    Vca,
    Mixer,
    StereoMixer,
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
    SampleHold,
}

impl std::fmt::Display for ModuleType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Oscillator => write!(f, "osc"),
            Self::Filter => write!(f, "filter"),
            Self::Envelope => write!(f, "env"),
            Self::Vca => write!(f, "vca"),
            Self::Mixer => write!(f, "mix"),
            Self::StereoMixer => write!(f, "stereomix"),
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
            Self::SampleHold => write!(f, "samplehold"),
        }
    }
}

/// Parse module type from string
///
/// # Errors
/// Returns an error if the string doesn't match any known module type
pub fn parse_module_type(s: &str) -> Result<ModuleType> {
    match s {
        "osc" => Ok(ModuleType::Oscillator),
        "filter" => Ok(ModuleType::Filter),
        "env" | "envelope" => Ok(ModuleType::Envelope),
        "vca" => Ok(ModuleType::Vca),
        "mix" | "mixer" | "mono_mixer" => Ok(ModuleType::Mixer),
        "stereomix" | "stereo_mixer" | "stereo_mix" => Ok(ModuleType::StereoMixer),
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
        "samplehold" | "sample_hold" | "sh" => Ok(ModuleType::SampleHold),
        _ => Err(anyhow!("Unknown module type: {s}")),
    }
}
