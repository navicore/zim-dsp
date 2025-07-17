//! Module implementations for the graph-based engine

#![allow(clippy::pedantic)]
#![allow(clippy::nursery)]

use crate::graph::{GraphModule, PortBuffers, PortDescriptor};
use anyhow::{anyhow, Result};

/// Oscillator module with multiple waveform outputs
pub struct GraphOscillator {
    frequency: f32,
    phase: f32,
    sample_rate: f32,
}

impl GraphOscillator {
    pub fn new(frequency: f32) -> Self {
        Self {
            frequency,
            phase: 0.0,
            sample_rate: 44100.0,
        }
    }
}

impl GraphModule for GraphOscillator {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn inputs(&self) -> Vec<PortDescriptor> {
        vec![
            PortDescriptor {
                name: "fm".to_string(),
                default_value: 0.0,
                description: "Frequency modulation input".to_string(),
            },
            PortDescriptor {
                name: "sync".to_string(),
                default_value: 0.0,
                description: "Sync/reset input".to_string(),
            },
        ]
    }

    fn outputs(&self) -> Vec<PortDescriptor> {
        vec![
            PortDescriptor {
                name: "sine".to_string(),
                default_value: 0.0,
                description: "Sine wave output".to_string(),
            },
            PortDescriptor {
                name: "saw".to_string(),
                default_value: 0.0,
                description: "Sawtooth wave output".to_string(),
            },
            PortDescriptor {
                name: "square".to_string(),
                default_value: 0.0,
                description: "Square wave output".to_string(),
            },
            PortDescriptor {
                name: "triangle".to_string(),
                default_value: 0.0,
                description: "Triangle wave output".to_string(),
            },
        ]
    }

    fn process(&mut self, inputs: &PortBuffers, outputs: &mut PortBuffers, sample_count: usize) {
        let fm_input = inputs.get("fm").map(|b| b.as_slice()).unwrap_or(&[]);
        let sync_input = inputs.get("sync").map(|b| b.as_slice()).unwrap_or(&[]);

        let [sine_out, saw_out, square_out, triangle_out] =
            outputs.get_many_mut(["sine", "saw", "square", "triangle"]);
        let sine_out = sine_out.unwrap();
        let saw_out = saw_out.unwrap();
        let square_out = square_out.unwrap();
        let triangle_out = triangle_out.unwrap();

        for i in 0..sample_count {
            // Handle sync
            if i < sync_input.len() && sync_input[i] > 0.0 && (i == 0 || sync_input[i - 1] <= 0.0) {
                self.phase = 0.0;
            }

            // Calculate frequency with FM
            let fm_amount = if i < fm_input.len() { fm_input[i] } else { 0.0 };
            let instant_freq = self.frequency * (1.0 + fm_amount);

            // Generate waveforms
            sine_out[i] = (self.phase * 2.0 * std::f32::consts::PI).sin();
            saw_out[i] = self.phase * 2.0 - 1.0;
            square_out[i] = if self.phase < 0.5 { 1.0 } else { -1.0 };
            triangle_out[i] =
                if self.phase < 0.5 { self.phase * 4.0 - 1.0 } else { 3.0 - self.phase * 4.0 };

            // Advance phase
            self.phase += instant_freq / self.sample_rate;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
        }
    }

    fn set_param(&mut self, name: &str, value: f32) -> Result<()> {
        match name {
            "frequency" | "freq" => {
                self.frequency = value;
                Ok(())
            }
            _ => Err(anyhow!("Unknown parameter: {}", name)),
        }
    }

    fn get_param(&self, name: &str) -> Option<f32> {
        match name {
            "frequency" | "freq" => Some(self.frequency),
            _ => None,
        }
    }
}

/// VCA module with audio and multiple CV inputs
pub struct GraphVca {
    gain: f32,
}

impl GraphVca {
    pub fn new(gain: f32) -> Self {
        Self { gain }
    }
}

impl GraphModule for GraphVca {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn inputs(&self) -> Vec<PortDescriptor> {
        vec![
            PortDescriptor {
                name: "audio".to_string(),
                default_value: 0.0,
                description: "Audio input".to_string(),
            },
            PortDescriptor {
                name: "cv".to_string(),
                default_value: 0.0,
                description: "Control voltage input (0=closed, 1=open)".to_string(),
            },
            PortDescriptor {
                name: "cv2".to_string(),
                default_value: 1.0,
                description: "Secondary CV input".to_string(),
            },
        ]
    }

    fn outputs(&self) -> Vec<PortDescriptor> {
        vec![PortDescriptor {
            name: "out".to_string(),
            default_value: 0.0,
            description: "Audio output".to_string(),
        }]
    }

    fn process(&mut self, inputs: &PortBuffers, outputs: &mut PortBuffers, sample_count: usize) {
        let audio = inputs.get("audio").map(|b| b.as_slice()).unwrap_or(&[]);
        let cv = inputs.get("cv").map(|b| b.as_slice()).unwrap_or(&[]);
        let cv2 = inputs.get("cv2").map(|b| b.as_slice()).unwrap_or(&[]);

        let out = outputs.get_mut("out").unwrap();

        for i in 0..sample_count {
            // These will always use the buffer values since buffers are pre-initialized
            let audio_sample = if i < audio.len() { audio[i] } else { 0.0 };
            let cv_value = if i < cv.len() { cv[i] } else { 0.0 };
            let cv2_mod = if i < cv2.len() { cv2[i] } else { 1.0 };

            out[i] = audio_sample * cv_value * cv2_mod * self.gain;
        }
    }

    fn set_param(&mut self, name: &str, value: f32) -> Result<()> {
        match name {
            "gain" => {
                self.gain = value;
                Ok(())
            }
            _ => Err(anyhow!("Unknown parameter: {}", name)),
        }
    }

    fn get_param(&self, name: &str) -> Option<f32> {
        match name {
            "gain" => Some(self.gain),
            _ => None,
        }
    }
}

/// Filter module with audio and CV inputs
pub struct GraphFilter {
    cutoff: f32,
    resonance: f32,
    // Simple one-pole lowpass state
    state: f32,
    sample_rate: f32,
}

impl GraphFilter {
    pub fn new(cutoff: f32, resonance: f32) -> Self {
        Self {
            cutoff,
            resonance,
            state: 0.0,
            sample_rate: 44100.0,
        }
    }
}

impl GraphModule for GraphFilter {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn inputs(&self) -> Vec<PortDescriptor> {
        vec![
            PortDescriptor {
                name: "audio".to_string(),
                default_value: 0.0,
                description: "Audio input".to_string(),
            },
            PortDescriptor {
                name: "cutoff".to_string(),
                default_value: 0.0,
                description: "Cutoff frequency CV".to_string(),
            },
            PortDescriptor {
                name: "resonance".to_string(),
                default_value: 0.0,
                description: "Resonance CV".to_string(),
            },
        ]
    }

    fn outputs(&self) -> Vec<PortDescriptor> {
        vec![
            PortDescriptor {
                name: "lp".to_string(),
                default_value: 0.0,
                description: "Lowpass output".to_string(),
            },
            PortDescriptor {
                name: "hp".to_string(),
                default_value: 0.0,
                description: "Highpass output".to_string(),
            },
        ]
    }

    fn process(&mut self, inputs: &PortBuffers, outputs: &mut PortBuffers, sample_count: usize) {
        let audio = inputs.get("audio").map(|b| b.as_slice()).unwrap_or(&[]);
        let cutoff_cv = inputs.get("cutoff").map(|b| b.as_slice()).unwrap_or(&[]);

        let [lp_out, hp_out] = outputs.get_many_mut(["lp", "hp"]);
        let lp_out = lp_out.unwrap();
        let hp_out = hp_out.unwrap();

        for i in 0..sample_count {
            let input = if i < audio.len() { audio[i] } else { 0.0 };
            let cutoff_mod = if i < cutoff_cv.len() { cutoff_cv[i] } else { 0.0 };

            // Calculate cutoff with CV modulation
            let freq = (self.cutoff + cutoff_mod).clamp(20.0, 20000.0);

            // Simple one-pole lowpass
            let cutoff_normalized = (freq / (self.sample_rate * 0.5)).min(0.99);
            self.state += cutoff_normalized * (input - self.state);

            lp_out[i] = self.state;
            hp_out[i] = input - self.state;
        }
    }

    fn set_param(&mut self, name: &str, value: f32) -> Result<()> {
        match name {
            "cutoff" => {
                self.cutoff = value;
                Ok(())
            }
            "resonance" | "res" => {
                self.resonance = value;
                Ok(())
            }
            _ => Err(anyhow!("Unknown parameter: {}", name)),
        }
    }

    fn get_param(&self, name: &str) -> Option<f32> {
        match name {
            "cutoff" => Some(self.cutoff),
            "resonance" | "res" => Some(self.resonance),
            _ => None,
        }
    }
}

/// LFO (Low Frequency Oscillator) module for modulation and clock signals
pub struct GraphLfo {
    frequency: f32,
    phase: f32,
    sample_rate: f32,
}

impl GraphLfo {
    pub fn new(frequency: f32) -> Self {
        Self {
            frequency,
            phase: 0.0,
            sample_rate: 44100.0,
        }
    }
}

impl GraphModule for GraphLfo {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn inputs(&self) -> Vec<PortDescriptor> {
        vec![PortDescriptor {
            name: "sync".to_string(),
            default_value: 0.0,
            description: "Sync/reset input".to_string(),
        }]
    }

    fn outputs(&self) -> Vec<PortDescriptor> {
        vec![
            PortDescriptor {
                name: "sine".to_string(),
                default_value: 0.0,
                description: "Sine wave output (bipolar: -1 to 1)".to_string(),
            },
            PortDescriptor {
                name: "square".to_string(),
                default_value: 0.0,
                description: "Square wave output (bipolar: -1 to 1)".to_string(),
            },
            PortDescriptor {
                name: "gate".to_string(),
                default_value: 0.0,
                description: "Gate output (unipolar: 0 to 1)".to_string(),
            },
            PortDescriptor {
                name: "ramp".to_string(),
                default_value: 0.0,
                description: "Ramp/saw output (unipolar: 0 to 1)".to_string(),
            },
        ]
    }

    fn process(&mut self, inputs: &PortBuffers, outputs: &mut PortBuffers, sample_count: usize) {
        let sync_input = inputs.get("sync").map(|b| b.as_slice()).unwrap_or(&[]);

        let [sine_out, square_out, gate_out, ramp_out] =
            outputs.get_many_mut(["sine", "square", "gate", "ramp"]);
        let sine_out = sine_out.unwrap();
        let square_out = square_out.unwrap();
        let gate_out = gate_out.unwrap();
        let ramp_out = ramp_out.unwrap();

        for i in 0..sample_count {
            // Handle sync
            if i < sync_input.len() && sync_input[i] > 0.0 && (i == 0 || sync_input[i - 1] <= 0.0) {
                self.phase = 0.0;
            }

            // Generate waveforms
            sine_out[i] = (self.phase * 2.0 * std::f32::consts::PI).sin();
            square_out[i] = if self.phase < 0.5 { 1.0 } else { -1.0 };
            gate_out[i] = if self.phase < 0.5 { 1.0 } else { 0.0 }; // Unipolar for gates
            ramp_out[i] = self.phase; // 0 to 1 ramp

            // Advance phase
            self.phase += self.frequency / self.sample_rate;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
        }
    }

    fn set_param(&mut self, name: &str, value: f32) -> Result<()> {
        match name {
            "frequency" | "freq" => {
                self.frequency = value;
                Ok(())
            }
            _ => Err(anyhow!("Unknown parameter: {}", name)),
        }
    }

    fn get_param(&self, name: &str) -> Option<f32> {
        match name {
            "frequency" | "freq" => Some(self.frequency),
            _ => None,
        }
    }
}

/// Manual gate module - outputs gate signal based on keyboard input
pub struct GraphManualGate {
    gate_on: bool,
}

impl GraphManualGate {
    pub fn new() -> Self {
        Self { gate_on: false }
    }

    pub fn set_gate(&mut self, on: bool) {
        self.gate_on = on;
    }
}

impl Default for GraphManualGate {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphModule for GraphManualGate {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn inputs(&self) -> Vec<PortDescriptor> {
        vec![] // No inputs
    }

    fn outputs(&self) -> Vec<PortDescriptor> {
        vec![
            PortDescriptor {
                name: "gate".to_string(),
                default_value: 0.0,
                description: "Gate output (0 or 1)".to_string(),
            },
            PortDescriptor {
                name: "trig".to_string(),
                default_value: 0.0,
                description: "Trigger output (pulse on key press)".to_string(),
            },
        ]
    }

    fn process(&mut self, _inputs: &PortBuffers, outputs: &mut PortBuffers, sample_count: usize) {
        let [gate_out, trig_out] = outputs.get_many_mut(["gate", "trig"]);
        let gate_out = gate_out.unwrap();
        let trig_out = trig_out.unwrap();

        let gate_value = if self.gate_on { 1.0 } else { 0.0 };

        for i in 0..sample_count {
            gate_out[i] = gate_value;
            // Trigger is just a copy of gate for now
            trig_out[i] = gate_value;
        }
    }

    fn set_param(&mut self, name: &str, value: f32) -> Result<()> {
        match name {
            "gate" => {
                self.gate_on = value > 0.5;
                Ok(())
            }
            _ => Err(anyhow!("Unknown parameter: {}", name)),
        }
    }

    fn get_param(&self, name: &str) -> Option<f32> {
        match name {
            "gate" => Some(if self.gate_on { 1.0 } else { 0.0 }),
            _ => None,
        }
    }
}

/// Stereo output module - handles mono to stereo normalization
pub struct GraphStereoOutput {
    left_connected: bool,
    right_connected: bool,
}

impl GraphStereoOutput {
    pub fn new() -> Self {
        Self {
            left_connected: false,
            right_connected: false,
        }
    }

    pub fn set_left_connected(&mut self, connected: bool) {
        self.left_connected = connected;
    }

    pub fn set_right_connected(&mut self, connected: bool) {
        self.right_connected = connected;
    }
}

impl Default for GraphStereoOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphModule for GraphStereoOutput {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn inputs(&self) -> Vec<PortDescriptor> {
        vec![
            PortDescriptor {
                name: "left".to_string(),
                default_value: 0.0,
                description: "Left channel input".to_string(),
            },
            PortDescriptor {
                name: "right".to_string(),
                default_value: 0.0,
                description: "Right channel input".to_string(),
            },
            PortDescriptor {
                name: "mono".to_string(),
                default_value: 0.0,
                description: "Mono input (routed to both channels)".to_string(),
            },
        ]
    }

    fn outputs(&self) -> Vec<PortDescriptor> {
        vec![
            PortDescriptor {
                name: "left".to_string(),
                default_value: 0.0,
                description: "Left channel output".to_string(),
            },
            PortDescriptor {
                name: "right".to_string(),
                default_value: 0.0,
                description: "Right channel output".to_string(),
            },
        ]
    }

    fn process(&mut self, inputs: &PortBuffers, outputs: &mut PortBuffers, sample_count: usize) {
        let left_in = inputs.get("left").map(|b| b.as_slice()).unwrap_or(&[]);
        let right_in = inputs.get("right").map(|b| b.as_slice()).unwrap_or(&[]);
        let mono_in = inputs.get("mono").map(|b| b.as_slice()).unwrap_or(&[]);

        let [left_out, right_out] = outputs.get_many_mut(["left", "right"]);
        let left_out = left_out.unwrap();
        let right_out = right_out.unwrap();

        for i in 0..sample_count {
            // Check if mono input is connected
            let mono_sample = if i < mono_in.len() { mono_in[i] } else { 0.0 };

            // Get stereo inputs
            let left_sample = if i < left_in.len() { left_in[i] } else { 0.0 };
            let right_sample = if i < right_in.len() { right_in[i] } else { 0.0 };

            // If mono is connected, it overrides stereo inputs
            if mono_sample != 0.0 || (!self.left_connected && !self.right_connected) {
                left_out[i] = mono_sample;
                right_out[i] = mono_sample;
            } else {
                // Handle stereo with normalization
                left_out[i] = left_sample;

                // If only left is connected, normalize to right
                if self.left_connected && !self.right_connected {
                    right_out[i] = left_sample;
                } else {
                    right_out[i] = right_sample;
                }
            }
        }
    }

    fn set_param(&mut self, name: &str, _value: f32) -> Result<()> {
        Err(anyhow!("Unknown parameter: {}", name))
    }

    fn get_param(&self, _name: &str) -> Option<f32> {
        None
    }
}

/// Envelope generator
pub struct GraphEnvelope {
    attack: f32,
    decay: f32,
    phase: EnvelopePhase,
    phase_time: f32,
    current_value: f32,
    sample_rate: f32,
    last_gate: f32, // Track previous gate value for edge detection
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum EnvelopePhase {
    Idle,
    Attack,
    Decay,
}

impl GraphEnvelope {
    pub fn new(attack: f32, decay: f32) -> Self {
        Self {
            attack,
            decay,
            phase: EnvelopePhase::Idle, // Start idle, wait for gate
            phase_time: 0.0,
            current_value: 0.0,
            sample_rate: 44100.0,
            last_gate: 0.0,
        }
    }
}

impl GraphModule for GraphEnvelope {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn inputs(&self) -> Vec<PortDescriptor> {
        vec![PortDescriptor {
            name: "gate".to_string(),
            default_value: 0.0,
            description: "Gate/trigger input".to_string(),
        }]
    }

    fn outputs(&self) -> Vec<PortDescriptor> {
        vec![PortDescriptor {
            name: "out".to_string(),
            default_value: 0.0,
            description: "Envelope output".to_string(),
        }]
    }

    fn process(&mut self, inputs: &PortBuffers, outputs: &mut PortBuffers, sample_count: usize) {
        let gate = inputs.get("gate").map(|b| b.as_slice()).unwrap_or(&[]);
        let out = outputs.get_mut("out").unwrap();

        for i in 0..sample_count {
            let current_gate = if i < gate.len() { gate[i] } else { 0.0 };

            // Check for rising edge trigger
            let prev_gate = if i == 0 { self.last_gate } else { gate[i - 1] };

            if current_gate > 0.0 && prev_gate <= 0.0 {
                // Rising edge detected - trigger envelope
                self.phase = EnvelopePhase::Attack;
                self.phase_time = 0.0;
            }

            match self.phase {
                EnvelopePhase::Idle => {
                    self.current_value = 0.0;
                }
                EnvelopePhase::Attack => {
                    if self.attack > 0.0 {
                        self.current_value = (self.phase_time / self.attack).min(1.0);
                        if self.phase_time >= self.attack {
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
                    if self.decay > 0.0 {
                        self.current_value = 1.0 - (self.phase_time / self.decay).min(1.0);
                        if self.phase_time >= self.decay {
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

            out[i] = self.current_value;
            self.phase_time += 1.0 / self.sample_rate;
        }

        // Remember the last gate value for next process call
        if sample_count > 0 && !gate.is_empty() {
            self.last_gate = gate[sample_count - 1];
        }
    }

    fn set_param(&mut self, name: &str, value: f32) -> Result<()> {
        match name {
            "attack" => {
                self.attack = value;
                Ok(())
            }
            "decay" => {
                self.decay = value;
                Ok(())
            }
            _ => Err(anyhow!("Unknown parameter: {}", name)),
        }
    }

    fn get_param(&self, name: &str) -> Option<f32> {
        match name {
            "attack" => Some(self.attack),
            "decay" => Some(self.decay),
            _ => None,
        }
    }
}
