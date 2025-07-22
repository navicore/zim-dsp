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
                name: "freq".to_string(),
                default_value: 0.0,
                description: "Frequency control input (Hz, 0 = use base freq)".to_string(),
            },
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
        let freq_input = inputs.get("freq").map(|b| b.as_slice()).unwrap_or(&[]);
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

            // Calculate frequency with direct freq control and FM
            let freq_cv = if i < freq_input.len() { freq_input[i] } else { 0.0 };
            let fm_amount = if i < fm_input.len() { fm_input[i] } else { 0.0 };

            // Use freq CV if connected and > 0, otherwise use base frequency
            // Check if freq input is actually connected (not just using default buffer)
            let has_freq_connection =
                inputs.get("freq").is_some() && !inputs.get("freq").unwrap().is_empty();
            let base_freq =
                if has_freq_connection && freq_cv > 0.0 { freq_cv } else { self.frequency };
            let instant_freq = base_freq * (1.0 + fm_amount);

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
            _ => Err(anyhow!("Unknown parameter: {name}")),
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
            _ => Err(anyhow!("Unknown parameter: {name}")),
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
            _ => Err(anyhow!("Unknown parameter: {name}")),
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
            _ => Err(anyhow!("Unknown parameter: {name}")),
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
            _ => Err(anyhow!("Unknown parameter: {name}")),
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
        Err(anyhow!("Unknown parameter: {name}"))
    }

    fn get_param(&self, _name: &str) -> Option<f32> {
        None
    }
}

/// Noise generator with multiple noise colors
pub struct GraphNoiseGen {
    // Random number generator state
    rng_state: u32,
    // Pink noise state (Paul Kellet's method)
    pink_state: [f32; 7],
    // Brown noise state
    brown_state: f32,
}

impl GraphNoiseGen {
    pub fn new() -> Self {
        Self {
            rng_state: 12345, // Seed
            pink_state: [0.0; 7],
            brown_state: 0.0,
        }
    }

    // Linear congruential generator for white noise
    fn next_random(&mut self) -> f32 {
        self.rng_state = self.rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        // Convert to float in range -1 to 1
        (self.rng_state as i32 as f32) / (i32::MAX as f32)
    }

    // Generate pink noise using Paul Kellet's method
    #[allow(clippy::excessive_precision)]
    fn generate_pink(&mut self) -> f32 {
        let white = self.next_random();

        // Update the state variables
        self.pink_state[0] = 0.99886 * self.pink_state[0] + white * 0.0555179;
        self.pink_state[1] = 0.99332 * self.pink_state[1] + white * 0.0750759;
        self.pink_state[2] = 0.96900 * self.pink_state[2] + white * 0.1538520;
        self.pink_state[3] = 0.86650 * self.pink_state[3] + white * 0.3104856;
        self.pink_state[4] = 0.55000 * self.pink_state[4] + white * 0.5329522;
        self.pink_state[5] = -0.7616 * self.pink_state[5] + white * 0.0168980;

        let pink = self.pink_state[0]
            + self.pink_state[1]
            + self.pink_state[2]
            + self.pink_state[3]
            + self.pink_state[4]
            + self.pink_state[5]
            + self.pink_state[6]
            + white * 0.5362;

        self.pink_state[6] = white * 0.115926;

        // Compensate for gain
        pink * 0.11
    }

    // Generate brown noise (red noise) by integrating white noise
    fn generate_brown(&mut self) -> f32 {
        let white = self.next_random();
        self.brown_state += white * 0.02; // Small step size

        // Prevent runaway - soft clip
        if self.brown_state > 1.0 {
            self.brown_state = 1.0 - (self.brown_state - 1.0) * 0.5;
        } else if self.brown_state < -1.0 {
            self.brown_state = -1.0 - (self.brown_state + 1.0) * 0.5;
        }

        self.brown_state
    }
}

impl Default for GraphNoiseGen {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphModule for GraphNoiseGen {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn inputs(&self) -> Vec<PortDescriptor> {
        vec![] // No inputs - noise is a source
    }

    fn outputs(&self) -> Vec<PortDescriptor> {
        vec![
            PortDescriptor {
                name: "white".to_string(),
                default_value: 0.0,
                description: "White noise output (flat spectrum)".to_string(),
            },
            PortDescriptor {
                name: "pink".to_string(),
                default_value: 0.0,
                description: "Pink noise output (-3dB/octave)".to_string(),
            },
            PortDescriptor {
                name: "brown".to_string(),
                default_value: 0.0,
                description: "Brown/red noise output (-6dB/octave)".to_string(),
            },
        ]
    }

    fn process(&mut self, _inputs: &PortBuffers, outputs: &mut PortBuffers, sample_count: usize) {
        let [white_out, pink_out, brown_out] = outputs.get_many_mut(["white", "pink", "brown"]);
        let white_out = white_out.unwrap();
        let pink_out = pink_out.unwrap();
        let brown_out = brown_out.unwrap();

        for i in 0..sample_count {
            // Generate white noise
            let white = self.next_random();
            white_out[i] = white;

            // Generate pink noise
            pink_out[i] = self.generate_pink();

            // Generate brown noise
            brown_out[i] = self.generate_brown();
        }
    }

    fn set_param(&mut self, name: &str, value: f32) -> Result<()> {
        match name {
            "seed" => {
                self.rng_state = value as u32;
                Ok(())
            }
            _ => Err(anyhow!("Unknown parameter: {name}")),
        }
    }

    fn get_param(&self, name: &str) -> Option<f32> {
        match name {
            "seed" => Some(self.rng_state as f32),
            _ => None,
        }
    }
}

/// Mono mixer module - combines multiple audio sources
pub struct GraphMonoMixer {
    input_count: usize,
    levels: Vec<f32>, // Individual input levels
    master_level: f32,
}

impl GraphMonoMixer {
    pub fn new(input_count: usize) -> Self {
        Self {
            input_count,
            levels: vec![1.0; input_count], // Unity gain by default
            master_level: 1.0,
        }
    }

    pub fn new_4input() -> Self {
        Self::new(4)
    }
}

impl Default for GraphMonoMixer {
    fn default() -> Self {
        Self::new_4input()
    }
}

impl GraphModule for GraphMonoMixer {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn inputs(&self) -> Vec<PortDescriptor> {
        let mut inputs = Vec::new();

        // Add numbered inputs
        for i in 1..=self.input_count {
            inputs.push(PortDescriptor {
                name: format!("in{i}"),
                default_value: 0.0,
                description: format!("Audio input {i}"),
            });
        }

        // Add level CV inputs
        for i in 1..=self.input_count {
            inputs.push(PortDescriptor {
                name: format!("level{i}"),
                default_value: 1.0,
                description: format!("Level control for input {i}"),
            });
        }

        // Master level CV
        inputs.push(PortDescriptor {
            name: "master".to_string(),
            default_value: 1.0,
            description: "Master level control".to_string(),
        });

        inputs
    }

    fn outputs(&self) -> Vec<PortDescriptor> {
        vec![PortDescriptor {
            name: "out".to_string(),
            default_value: 0.0,
            description: "Mixed audio output".to_string(),
        }]
    }

    fn process(&mut self, inputs: &PortBuffers, outputs: &mut PortBuffers, sample_count: usize) {
        let master_cv = inputs.get("master").map(|b| b.as_slice()).unwrap_or(&[]);
        let out = outputs.get_mut("out").unwrap();

        // Get all input buffers
        let mut input_buffers = Vec::new();
        let mut level_buffers = Vec::new();

        for i in 1..=self.input_count {
            let input = inputs.get(&format!("in{i}")).map(|b| b.as_slice()).unwrap_or(&[]);
            let level = inputs.get(&format!("level{i}")).map(|b| b.as_slice()).unwrap_or(&[]);
            input_buffers.push(input);
            level_buffers.push(level);
        }

        // Mix all inputs
        for i in 0..sample_count {
            let mut mixed_sample = 0.0;

            // Sum all inputs with their levels
            for (input_idx, (input_buf, level_buf)) in
                input_buffers.iter().zip(level_buffers.iter()).enumerate()
            {
                let input_sample = if i < input_buf.len() { input_buf[i] } else { 0.0 };
                let level_sample =
                    if i < level_buf.len() { level_buf[i] } else { self.levels[input_idx] };

                mixed_sample += input_sample * level_sample;
            }

            // Apply master level
            let master_sample = if i < master_cv.len() { master_cv[i] } else { self.master_level };
            out[i] = mixed_sample * master_sample;
        }
    }

    fn set_param(&mut self, name: &str, value: f32) -> Result<()> {
        if name == "master" {
            self.master_level = value;
            Ok(())
        } else if let Some(stripped) = name.strip_prefix("level") {
            if let Ok(index) = stripped.parse::<usize>() {
                if index > 0 && index <= self.input_count {
                    self.levels[index - 1] = value;
                    Ok(())
                } else {
                    Err(anyhow!("Invalid level index: {index}"))
                }
            } else {
                Err(anyhow!("Invalid level parameter: {name}"))
            }
        } else {
            Err(anyhow!("Unknown parameter: {name}"))
        }
    }

    fn get_param(&self, name: &str) -> Option<f32> {
        if name == "master" {
            Some(self.master_level)
        } else if let Some(stripped) = name.strip_prefix("level") {
            if let Ok(index) = stripped.parse::<usize>() {
                if index > 0 && index <= self.input_count {
                    Some(self.levels[index - 1])
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    }
}

/// Slew generator module - smooths stepped CV signals
pub struct GraphSlewGen {
    rise_time: f32, // Time to rise from 0 to 1
    fall_time: f32, // Time to fall from 1 to 0
    current_value: f32,
    target_value: f32,
    sample_rate: f32,
    curve_type: SlewCurve,
    // Gate state tracking
    previous_value: f32,
    is_rising: bool,
    is_falling: bool,
    reached_target: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum SlewCurve {
    Linear,
    Exponential,
    Logarithmic,
}

impl GraphSlewGen {
    pub fn new(rise_time: f32, fall_time: f32) -> Self {
        Self {
            rise_time,
            fall_time,
            current_value: 0.0,
            target_value: 0.0,
            sample_rate: 44100.0,
            curve_type: SlewCurve::Linear,
            // Initialize gate state
            previous_value: 0.0,
            is_rising: false,
            is_falling: false,
            reached_target: false, // Start as not at target to allow first transition to fire gates
        }
    }

    fn apply_curve(&self, progress: f32) -> f32 {
        match self.curve_type {
            SlewCurve::Linear => progress,
            SlewCurve::Exponential => {
                // Exponential curve: fast at start, slow at end
                1.0 - (-4.0 * progress).exp()
            }
            SlewCurve::Logarithmic => {
                // Logarithmic curve: slow at start, fast at end
                if progress <= 0.0 {
                    0.0
                } else {
                    (1.0 + 4.0 * progress).ln() / (1.0_f32 + 4.0).ln()
                }
            }
        }
    }

    fn set_curve_from_param(&mut self, value: f32) {
        self.curve_type = match value as i32 {
            0 => SlewCurve::Linear,
            1 => SlewCurve::Exponential,
            2 => SlewCurve::Logarithmic,
            _ => SlewCurve::Linear,
        };
    }
}

impl Default for GraphSlewGen {
    fn default() -> Self {
        Self::new(0.1, 0.1) // 100ms rise/fall
    }
}

impl GraphModule for GraphSlewGen {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn inputs(&self) -> Vec<PortDescriptor> {
        vec![
            PortDescriptor {
                name: "in".to_string(),
                default_value: 0.0,
                description: "Input signal to smooth".to_string(),
            },
            PortDescriptor {
                name: "rise".to_string(),
                default_value: 0.0,
                description: "Rise time CV (0V = use parameter, >0V = override)".to_string(),
            },
            PortDescriptor {
                name: "fall".to_string(),
                default_value: 0.0,
                description: "Fall time CV (0V = use parameter, >0V = override)".to_string(),
            },
        ]
    }

    fn outputs(&self) -> Vec<PortDescriptor> {
        vec![
            PortDescriptor {
                name: "out".to_string(),
                default_value: 0.0,
                description: "Smoothed output signal".to_string(),
            },
            PortDescriptor {
                name: "eor".to_string(),
                default_value: 0.0,
                description: "End-of-rise gate (fires when reaching target while rising)"
                    .to_string(),
            },
            PortDescriptor {
                name: "eoc".to_string(),
                default_value: 0.0,
                description: "End-of-cycle gate (fires when reaching target while falling)"
                    .to_string(),
            },
        ]
    }

    fn process(&mut self, inputs: &PortBuffers, outputs: &mut PortBuffers, sample_count: usize) {
        let input_signal = inputs.get("in").map(|b| b.as_slice()).unwrap_or(&[]);
        let rise_cv = inputs.get("rise").map(|b| b.as_slice()).unwrap_or(&[]);
        let fall_cv = inputs.get("fall").map(|b| b.as_slice()).unwrap_or(&[]);
        let [out, eor_out, eoc_out] = outputs.get_many_mut(["out", "eor", "eoc"]);
        let out = out.unwrap();
        let eor_out = eor_out.unwrap();
        let eoc_out = eoc_out.unwrap();

        for i in 0..sample_count {
            // Get the current input value
            let input_value = if i < input_signal.len() { input_signal[i] } else { 0.0 };
            self.target_value = input_value;

            // Get CV values for this sample
            let rise_cv_value = if i < rise_cv.len() { rise_cv[i] } else { 0.0 };
            let fall_cv_value = if i < fall_cv.len() { fall_cv[i] } else { 0.0 };

            // Calculate effective rise and fall times
            // If CV > 0, use CV value; otherwise use parameter value
            let effective_rise_time = if rise_cv_value > 0.001 {
                rise_cv_value.max(0.001) // CV override
            } else {
                self.rise_time.max(0.001) // Parameter default
            };

            let effective_fall_time = if fall_cv_value > 0.001 {
                fall_cv_value.max(0.001) // CV override
            } else {
                self.fall_time.max(0.001) // Parameter default
            };

            // Track previous state for gate detection
            let was_at_target = self.reached_target;
            let previous_value = self.current_value;

            // Calculate the difference between current and target
            let diff = self.target_value - self.current_value;

            // Gates default to HIGH for self-patching bootstrap, but will pulse LOW→HIGH
            // This maintains analog behavior where gates start HIGH until activity
            eor_out[i] = 1.0;
            eoc_out[i] = 1.0;

            if diff.abs() > 0.001 {
                // We're moving - not at target
                self.reached_target = false;

                // Determine if we're rising or falling
                let is_rising = diff > 0.0;
                self.is_rising = is_rising;
                self.is_falling = !is_rising;

                // During slewing: appropriate gate goes LOW
                if is_rising {
                    eor_out[i] = 0.0; // EOR LOW during rising
                } else {
                    eoc_out[i] = 0.0; // EOC LOW during falling
                }

                let time_constant =
                    if is_rising { effective_rise_time } else { effective_fall_time };

                // Calculate step size based on time constant
                let step_size = 1.0 / (time_constant * self.sample_rate);
                let raw_progress = step_size;

                // Apply curve shaping
                let shaped_progress = self.apply_curve(raw_progress);
                let step = diff * shaped_progress.min(1.0);

                self.current_value += step;

                // Check if we reached the target this sample
                let reached_target_this_sample = if is_rising {
                    self.current_value >= self.target_value
                } else {
                    self.current_value <= self.target_value
                };

                // Fire gate pulse ONLY on the sample when we reach target
                if reached_target_this_sample {
                    self.current_value = self.target_value;
                    self.reached_target = true;

                    // Single-sample completion pulse
                    if is_rising {
                        eor_out[i] = 1.0; // EOR pulse on completion of rise
                    } else {
                        eoc_out[i] = 1.0; // EOC pulse on completion of fall
                    }
                }
            } else {
                // Close enough to target - consider reached
                if !was_at_target {
                    // We just reached the target - fire completion pulse
                    let was_rising = self.is_rising;
                    let was_falling = self.is_falling;

                    // Single-sample completion pulse
                    if was_rising {
                        eor_out[i] = 1.0; // EOR pulse
                    } else if was_falling {
                        eoc_out[i] = 1.0; // EOC pulse
                    }
                }

                self.current_value = self.target_value;
                self.reached_target = true;
                self.is_rising = false;
                self.is_falling = false;
            }

            out[i] = self.current_value;
            self.previous_value = previous_value;
        }
    }

    fn set_param(&mut self, name: &str, value: f32) -> Result<()> {
        match name {
            "rise" => {
                self.rise_time = value.max(0.001);
                Ok(())
            }
            "fall" => {
                self.fall_time = value.max(0.001);
                Ok(())
            }
            "curve" => {
                self.set_curve_from_param(value);
                Ok(())
            }
            _ => Err(anyhow!("Unknown parameter: {name}")),
        }
    }

    fn get_param(&self, name: &str) -> Option<f32> {
        match name {
            "rise" => Some(self.rise_time),
            "fall" => Some(self.fall_time),
            "curve" => Some(match self.curve_type {
                SlewCurve::Linear => 0.0,
                SlewCurve::Exponential => 1.0,
                SlewCurve::Logarithmic => 2.0,
            }),
            _ => None,
        }
    }
}

/// Visual debug module - prints input values to stdout
pub struct GraphVisual {
    last_value: f32,
    sample_count: usize,
    sample_rate: f32,
    print_interval: usize, // Print every N samples
}

impl GraphVisual {
    pub fn new() -> Self {
        Self {
            last_value: f32::NAN,
            sample_count: 0,
            sample_rate: 44100.0,
            print_interval: 4410, // Print 10 times per second at 44.1kHz
        }
    }
}

impl Default for GraphVisual {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphModule for GraphVisual {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn inputs(&self) -> Vec<PortDescriptor> {
        vec![PortDescriptor {
            name: "input".to_string(),
            default_value: 0.0,
            description: "Signal to monitor".to_string(),
        }]
    }

    fn outputs(&self) -> Vec<PortDescriptor> {
        vec![] // No outputs - just prints to console
    }

    fn process(&mut self, inputs: &PortBuffers, _outputs: &mut PortBuffers, sample_count: usize) {
        let input_signal = inputs.get("input").map(|b| b.as_slice()).unwrap_or(&[]);

        for i in 0..sample_count {
            let current_value = if i < input_signal.len() { input_signal[i] } else { 0.0 };

            // Print when value changes significantly or at regular intervals
            let value_changed = (current_value - self.last_value).abs() > 0.001;
            let time_to_print = self.sample_count % self.print_interval == 0;

            if value_changed || time_to_print {
                let time_seconds = self.sample_count as f32 / self.sample_rate;
                println!("[VISUAL] t={time_seconds:.2}s: {current_value:.3}");
                self.last_value = current_value;
            }

            self.sample_count += 1;
        }
    }

    fn set_param(&mut self, _name: &str, _value: f32) -> Result<()> {
        Err(anyhow!("Visual module has no parameters"))
    }

    fn get_param(&self, _name: &str) -> Option<f32> {
        None
    }
}

/// Multiple (mult) - 1 input to 4 outputs
pub struct GraphMult {
    // No internal state needed - just copies input to outputs
}

impl GraphMult {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for GraphMult {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphModule for GraphMult {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn inputs(&self) -> Vec<PortDescriptor> {
        vec![PortDescriptor {
            name: "input".to_string(),
            default_value: 0.0,
            description: "Input signal to multiply".to_string(),
        }]
    }

    fn outputs(&self) -> Vec<PortDescriptor> {
        vec![
            PortDescriptor {
                name: "out1".to_string(),
                default_value: 0.0,
                description: "Output 1".to_string(),
            },
            PortDescriptor {
                name: "out2".to_string(),
                default_value: 0.0,
                description: "Output 2".to_string(),
            },
            PortDescriptor {
                name: "out3".to_string(),
                default_value: 0.0,
                description: "Output 3".to_string(),
            },
            PortDescriptor {
                name: "out4".to_string(),
                default_value: 0.0,
                description: "Output 4".to_string(),
            },
        ]
    }

    fn process(&mut self, inputs: &PortBuffers, outputs: &mut PortBuffers, sample_count: usize) {
        let input_signal = inputs.get("input").map(|b| b.as_slice()).unwrap_or(&[]);

        let [out1, out2, out3, out4] = outputs.get_many_mut(["out1", "out2", "out3", "out4"]);
        let out1 = out1.unwrap();
        let out2 = out2.unwrap();
        let out3 = out3.unwrap();
        let out4 = out4.unwrap();

        for i in 0..sample_count {
            let input_value = if i < input_signal.len() { input_signal[i] } else { 0.0 };

            // Copy input to all outputs
            out1[i] = input_value;
            out2[i] = input_value;
            out3[i] = input_value;
            out4[i] = input_value;
        }
    }

    fn set_param(&mut self, _name: &str, _value: f32) -> Result<()> {
        Err(anyhow!("Mult module has no parameters"))
    }

    fn get_param(&self, _name: &str) -> Option<f32> {
        None
    }
}

/// Sequential switch module - switches between multiple inputs based on clock
pub struct GraphSwitch {
    current_input: usize,
    input_count: usize,
    last_clock: f32,
    switch_count: usize,
}

impl GraphSwitch {
    pub fn new(input_count: usize) -> Self {
        Self {
            current_input: 0,
            input_count: input_count.clamp(2, 8), // 2-8 inputs
            last_clock: 0.0,
            switch_count: 0,
        }
    }

    pub fn set_input_count(&mut self, count: usize) {
        self.input_count = count.clamp(2, 8);
        if self.current_input >= self.input_count {
            self.current_input = 0;
        }
    }
}

impl GraphModule for GraphSwitch {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn inputs(&self) -> Vec<PortDescriptor> {
        let mut inputs = vec![
            PortDescriptor {
                name: "clock".to_string(),
                default_value: 0.0,
                description: "Clock input to advance switch position".to_string(),
            },
            PortDescriptor {
                name: "reset".to_string(),
                default_value: 0.0,
                description: "Reset switch to input 1".to_string(),
            },
        ];

        // Add input ports dynamically based on input_count
        for i in 1..=self.input_count {
            inputs.push(PortDescriptor {
                name: format!("in{i}"),
                default_value: 0.0,
                description: format!("Input {i}"),
            });
        }

        inputs
    }

    fn outputs(&self) -> Vec<PortDescriptor> {
        vec![
            PortDescriptor {
                name: "out".to_string(),
                default_value: 0.0,
                description: "Selected input output".to_string(),
            },
            PortDescriptor {
                name: "gate".to_string(),
                default_value: 0.0,
                description: "Gate output when switching".to_string(),
            },
        ]
    }

    fn process(&mut self, inputs: &PortBuffers, outputs: &mut PortBuffers, sample_count: usize) {
        let clock = inputs.get("clock").map(|b| b.as_slice()).unwrap_or(&[]);
        let reset = inputs.get("reset").map(|b| b.as_slice()).unwrap_or(&[]);

        // Get all input signals
        let mut input_signals = Vec::new();
        for i in 1..=self.input_count {
            let signal = inputs.get(&format!("in{i}")).map(|b| b.as_slice()).unwrap_or(&[]);
            input_signals.push(signal);
        }

        let [out, gate_out] = outputs.get_many_mut(["out", "gate"]);
        let out = out.unwrap();
        let gate_out = gate_out.unwrap();

        for i in 0..sample_count {
            let clock_val = if i < clock.len() { clock[i] } else { 0.0 };
            let reset_val = if i < reset.len() { reset[i] } else { 0.0 };

            // Check for reset trigger
            if reset_val > 0.5 {
                self.current_input = 0;
            }

            // Check for clock rising edge
            let rising_edge = clock_val > 0.5 && self.last_clock <= 0.5;
            if rising_edge {
                self.current_input = (self.current_input + 1) % self.input_count;
                self.switch_count += 1;
            }

            // Output the selected input
            let selected_signal = &input_signals[self.current_input];
            out[i] = if i < selected_signal.len() { selected_signal[i] } else { 0.0 };

            // Gate output - brief pulse when switching
            gate_out[i] = if rising_edge { 1.0 } else { 0.0 };

            // Update last_clock for next iteration
            self.last_clock = clock_val;
        }
    }

    fn set_param(&mut self, name: &str, value: f32) -> Result<()> {
        match name {
            "inputs" => {
                self.set_input_count(value as usize);
                Ok(())
            }
            "reset" => {
                if value > 0.5 {
                    self.current_input = 0;
                }
                Ok(())
            }
            _ => Err(anyhow!("Unknown parameter: {}", name)),
        }
    }

    fn get_param(&self, name: &str) -> Option<f32> {
        match name {
            "inputs" => Some(self.input_count as f32),
            "current" => Some(self.current_input as f32 + 1.0), // 1-indexed for user
            "count" => Some(self.switch_count as f32),
            _ => None,
        }
    }
}

/// Clock divider module - divides input clock by configurable ratio
pub struct GraphClockDiv {
    division: usize,
    counter: usize,
    last_clock: f32,
    output_state: bool,
}

impl GraphClockDiv {
    pub fn new(division: usize) -> Self {
        Self {
            division: division.max(1),
            counter: 0,
            last_clock: 0.0,
            output_state: false,
        }
    }

    pub fn set_division(&mut self, division: usize) {
        self.division = division.max(1);
        self.counter = 0;
    }
}

impl GraphModule for GraphClockDiv {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn inputs(&self) -> Vec<PortDescriptor> {
        vec![
            PortDescriptor {
                name: "clock".to_string(),
                default_value: 0.0,
                description: "Input clock signal".to_string(),
            },
            PortDescriptor {
                name: "reset".to_string(),
                default_value: 0.0,
                description: "Reset counter".to_string(),
            },
        ]
    }

    fn outputs(&self) -> Vec<PortDescriptor> {
        vec![
            PortDescriptor {
                name: "out".to_string(),
                default_value: 0.0,
                description: "Divided clock output".to_string(),
            },
            PortDescriptor {
                name: "gate".to_string(),
                default_value: 0.0,
                description: "Gate output for divided clock".to_string(),
            },
        ]
    }

    fn process(&mut self, inputs: &PortBuffers, outputs: &mut PortBuffers, sample_count: usize) {
        let clock = inputs.get("clock").map(|b| b.as_slice()).unwrap_or(&[]);
        let reset = inputs.get("reset").map(|b| b.as_slice()).unwrap_or(&[]);

        let [out, gate_out] = outputs.get_many_mut(["out", "gate"]);
        let out = out.unwrap();
        let gate_out = gate_out.unwrap();

        for i in 0..sample_count {
            let clock_val = if i < clock.len() { clock[i] } else { 0.0 };
            let reset_val = if i < reset.len() { reset[i] } else { 0.0 };

            // Check for reset trigger
            if reset_val > 0.5 {
                self.counter = 0;
                self.output_state = false;
            }

            // Check for clock rising edge
            let rising_edge = clock_val > 0.5 && self.last_clock <= 0.5;
            if rising_edge {
                self.counter += 1;
                if self.counter >= self.division {
                    self.counter = 0;
                    self.output_state = !self.output_state;
                }
            }

            // Output divided clock
            out[i] = if self.output_state { 1.0 } else { 0.0 };

            // Gate output - pulse on divided clock rising edge
            gate_out[i] = if rising_edge && self.counter == 0 { 1.0 } else { 0.0 };

            // Update last_clock for next iteration
            self.last_clock = clock_val;
        }
    }

    fn set_param(&mut self, name: &str, value: f32) -> Result<()> {
        match name {
            "division" | "div" => {
                self.set_division(value as usize);
                Ok(())
            }
            "reset" => {
                if value > 0.5 {
                    self.counter = 0;
                    self.output_state = false;
                }
                Ok(())
            }
            _ => Err(anyhow!("Unknown parameter: {}", name)),
        }
    }

    fn get_param(&self, name: &str) -> Option<f32> {
        match name {
            "division" | "div" => Some(self.division as f32),
            "counter" => Some(self.counter as f32),
            _ => None,
        }
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
    attack_shape: EnvelopeShape,
    decay_shape: EnvelopeShape,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum EnvelopePhase {
    Idle,
    Attack,
    Decay,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum EnvelopeShape {
    Linear,
    Exponential,
    Logarithmic,
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
            attack_shape: EnvelopeShape::Linear,
            decay_shape: EnvelopeShape::Linear,
        }
    }

    /// Apply envelope shaping curve
    fn apply_shape(progress: f32, shape: EnvelopeShape) -> f32 {
        match shape {
            EnvelopeShape::Linear => progress,
            EnvelopeShape::Exponential => {
                // Exponential curve: starts slow, accelerates
                progress * progress
            }
            EnvelopeShape::Logarithmic => {
                // Logarithmic curve: starts fast, decelerates
                (progress * 2.0 - progress * progress).min(1.0)
            }
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
                        let linear_progress = (self.phase_time / self.attack).min(1.0);
                        self.current_value = Self::apply_shape(linear_progress, self.attack_shape);
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
                        let linear_progress = (self.phase_time / self.decay).min(1.0);
                        let shaped_progress = Self::apply_shape(linear_progress, self.decay_shape);
                        self.current_value = 1.0 - shaped_progress;
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
            "attack_shape" => {
                self.attack_shape = match value as i32 {
                    0 => EnvelopeShape::Linear,
                    1 => EnvelopeShape::Exponential,
                    2 => EnvelopeShape::Logarithmic,
                    _ => {
                        return Err(anyhow!(
                            "Invalid attack shape: {} (0=linear, 1=exp, 2=log)",
                            value
                        ))
                    }
                };
                Ok(())
            }
            "decay_shape" => {
                self.decay_shape = match value as i32 {
                    0 => EnvelopeShape::Linear,
                    1 => EnvelopeShape::Exponential,
                    2 => EnvelopeShape::Logarithmic,
                    _ => {
                        return Err(anyhow!(
                            "Invalid decay shape: {} (0=linear, 1=exp, 2=log)",
                            value
                        ))
                    }
                };
                Ok(())
            }
            _ => Err(anyhow!("Unknown parameter: {name}")),
        }
    }

    fn get_param(&self, name: &str) -> Option<f32> {
        match name {
            "attack" => Some(self.attack),
            "decay" => Some(self.decay),
            "attack_shape" => Some(match self.attack_shape {
                EnvelopeShape::Linear => 0.0,
                EnvelopeShape::Exponential => 1.0,
                EnvelopeShape::Logarithmic => 2.0,
            }),
            "decay_shape" => Some(match self.decay_shape {
                EnvelopeShape::Linear => 0.0,
                EnvelopeShape::Exponential => 1.0,
                EnvelopeShape::Logarithmic => 2.0,
            }),
            _ => None,
        }
    }
}

/// 8-step sequencer with CV and gate outputs
pub struct GraphSeq8 {
    steps: [f32; 8],
    gates: [bool; 8],
    current_step: usize,
    last_clock: f32,
    last_reverse: f32,
    clock_count: usize,
    gate_length: f32,
    samples_since_clock: usize,
    sample_rate: f32,
    forward_direction: bool,
    sequence_length: usize,
}

impl GraphSeq8 {
    pub fn new() -> Self {
        Self {
            steps: [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7], // Default ascending pattern
            gates: [true; 8],                                // All gates on by default
            current_step: 0,
            last_clock: 0.0,
            last_reverse: 0.0,
            clock_count: 0,
            gate_length: 0.1, // 100ms gate length
            samples_since_clock: 0,
            sample_rate: 44100.0,
            forward_direction: true,
            sequence_length: 8,
        }
    }

    fn get_gate_length_samples(&self) -> usize {
        (self.gate_length * self.sample_rate) as usize
    }
}

impl Default for GraphSeq8 {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphModule for GraphSeq8 {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn inputs(&self) -> Vec<PortDescriptor> {
        let mut inputs = vec![
            PortDescriptor {
                name: "clock".to_string(),
                default_value: 0.0,
                description: "Clock input to advance sequence".to_string(),
            },
            PortDescriptor {
                name: "reset".to_string(),
                default_value: 0.0,
                description: "Reset to step 1".to_string(),
            },
            PortDescriptor {
                name: "reverse".to_string(),
                default_value: 0.0,
                description: "Reverse direction on rising edge".to_string(),
            },
            PortDescriptor {
                name: "length".to_string(),
                default_value: 8.0,
                description: "Sequence length (1-8 steps)".to_string(),
            },
            PortDescriptor {
                name: "gate_length".to_string(),
                default_value: 0.1,
                description: "Gate length in seconds".to_string(),
            },
        ];

        // Add step value inputs
        for i in 0..8 {
            inputs.push(PortDescriptor {
                name: format!("step{}", i + 1),
                default_value: (i as f32) / 7.0, // 0 to 1 range
                description: format!("CV value for step {}", i + 1),
            });
        }

        // Add gate enable inputs
        for i in 0..8 {
            inputs.push(PortDescriptor {
                name: format!("gate{}", i + 1),
                default_value: 1.0,
                description: format!("Gate enable for step {} (>0.5 = on)", i + 1),
            });
        }

        inputs
    }

    fn outputs(&self) -> Vec<PortDescriptor> {
        vec![
            PortDescriptor {
                name: "cv".to_string(),
                default_value: 0.0,
                description: "CV output for current step".to_string(),
            },
            PortDescriptor {
                name: "gate".to_string(),
                default_value: 0.0,
                description: "Gate output for current step".to_string(),
            },
            PortDescriptor {
                name: "step".to_string(),
                default_value: 0.0,
                description: "Current step number (0-7)".to_string(),
            },
        ]
    }

    fn process(&mut self, inputs: &PortBuffers, outputs: &mut PortBuffers, sample_count: usize) {
        let clock = inputs.get("clock").map(|b| b.as_slice()).unwrap_or(&[]);
        let reset = inputs.get("reset").map(|b| b.as_slice()).unwrap_or(&[]);
        let reverse = inputs.get("reverse").map(|b| b.as_slice()).unwrap_or(&[]);
        let _length_cv = inputs.get("length").map(|b| b.as_slice()).unwrap_or(&[]);
        let gate_length_cv = inputs.get("gate_length").map(|b| b.as_slice()).unwrap_or(&[]);

        // Get step values - use input connections if available, otherwise use parameter values
        let step_values = self.steps; // Use parameter values directly
                                      // TODO: Add proper input connection detection when we need CV inputs to step values

        // Get gate enables - use input connections if available, otherwise use parameter values
        let mut gate_enables = self.gates; // Start with parameter values
        for (i, enable) in gate_enables.iter_mut().enumerate() {
            if let Some(buffer) = inputs.get(&format!("gate{}", i + 1)) {
                if let Some(value) = buffer.as_slice().first() {
                    *enable = *value > 0.5; // Override with input if connected
                }
            }
        }

        let [cv_out, gate_out, step_out] = outputs.get_many_mut(["cv", "gate", "step"]);
        let cv_out = cv_out.unwrap();
        let gate_out = gate_out.unwrap();
        let step_out = step_out.unwrap();

        for i in 0..sample_count {
            let current_clock = if i < clock.len() { clock[i] } else { 0.0 };
            let current_reset = if i < reset.len() { reset[i] } else { 0.0 };
            let current_reverse = if i < reverse.len() { reverse[i] } else { 0.0 };
            let current_gate_length =
                if i < gate_length_cv.len() { gate_length_cv[i] } else { self.gate_length };

            // Use parameter value for sequence length (ignore CV input for now)
            // TODO: Add proper input connection detection when we need CV control of length

            // Update gate length
            self.gate_length = current_gate_length.max(0.001);

            // Check for reset trigger
            if current_reset > 0.5 {
                self.current_step = 0;
                self.samples_since_clock = 0;
                self.clock_count = 0;
                self.forward_direction = true;
            }

            // Check for reverse trigger (rising edge)
            if current_reverse > 0.5 && self.last_reverse <= 0.5 {
                self.forward_direction = !self.forward_direction;
            }

            // Check for clock trigger (rising edge)
            if current_clock > 0.5 && self.last_clock <= 0.5 {
                if self.forward_direction {
                    self.current_step = (self.current_step + 1) % self.sequence_length;
                } else {
                    self.current_step = if self.current_step == 0 {
                        self.sequence_length - 1
                    } else {
                        self.current_step - 1
                    };
                }
                self.samples_since_clock = 0;
                self.clock_count += 1;
            }

            // Note: step_values and gate_enables are local arrays that combine
            // parameter values with any connected inputs. We don't update the
            // module's internal arrays here to preserve parameter settings.

            // Generate outputs
            cv_out[i] = step_values[self.current_step];
            step_out[i] = self.current_step as f32;

            // Gate output depends on gate enable and timing
            let gate_samples = self.get_gate_length_samples();
            let gate_active =
                gate_enables[self.current_step] && self.samples_since_clock < gate_samples;
            gate_out[i] = if gate_active { 1.0 } else { 0.0 };

            self.last_clock = current_clock;
            self.last_reverse = current_reverse;
            self.samples_since_clock += 1;
        }
    }

    fn set_param(&mut self, name: &str, value: f32) -> Result<()> {
        match name {
            "length" => {
                self.sequence_length = (value as usize).clamp(1, 8);
                Ok(())
            }
            "gate_length" => {
                self.gate_length = value.max(0.001);
                Ok(())
            }
            _ => {
                // Check for step parameters
                if let Some(step_str) = name.strip_prefix("step") {
                    if let Ok(step_num) = step_str.parse::<usize>() {
                        if (1..=8).contains(&step_num) {
                            self.steps[step_num - 1] = value;
                            return Ok(());
                        }
                    }
                }

                // Check for gate parameters
                if let Some(gate_str) = name.strip_prefix("gate") {
                    if let Ok(gate_num) = gate_str.parse::<usize>() {
                        if (1..=8).contains(&gate_num) {
                            self.gates[gate_num - 1] = value > 0.5;
                            return Ok(());
                        }
                    }
                }

                Err(anyhow!("Unknown parameter: {}", name))
            }
        }
    }

    fn get_param(&self, name: &str) -> Option<f32> {
        match name {
            "length" => Some(self.sequence_length as f32),
            "gate_length" => Some(self.gate_length),
            _ => {
                // Check for step parameters
                if let Some(step_str) = name.strip_prefix("step") {
                    if let Ok(step_num) = step_str.parse::<usize>() {
                        if (1..=8).contains(&step_num) {
                            return Some(self.steps[step_num - 1]);
                        }
                    }
                }

                // Check for gate parameters
                if let Some(gate_str) = name.strip_prefix("gate") {
                    if let Ok(gate_num) = gate_str.parse::<usize>() {
                        if (1..=8).contains(&gate_num) {
                            return Some(if self.gates[gate_num - 1] { 1.0 } else { 0.0 });
                        }
                    }
                }

                None
            }
        }
    }
}
