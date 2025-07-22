# Zim-DSP Design Document

## Core Concept: Text-Based Modular Synthesis

### The Problem
- Hardware modular synths are expensive and space-limited
- Software modulars (VCV Rack, etc) are GUI-heavy and mouse-driven  
- Pure Data is powerful but has terrible UX and complex build system
- Most "open source" audio projects aren't truly open (VCV Rack's licensing games)

### Our Solution
A text-first modular synthesizer that:
1. Uses familiar module names and concepts from hardware
2. Patches with simple, readable syntax
3. Enables live coding and experimentation
4. Built on genuinely open source foundations

## Technical Architecture

### Module System
```rust
// Every module implements this trait
trait Module {
    fn process(&mut self, inputs: &[f32], outputs: &mut [f32]);
    fn set_param(&mut self, param: &str, value: f32);
    fn get_param(&self, param: &str) -> Option<f32>;
}

// Modules can have multiple inputs/outputs
struct ModuleInfo {
    name: String,
    inputs: Vec<String>,   // e.g., ["audio", "fm", "sync"]
    outputs: Vec<String>,  // e.g., ["out", "inv"]
    params: Vec<String>,   // e.g., ["freq", "wave", "pwm"]
}
```

### Patch Language Design

#### Basic Syntax
```
# Module declaration
module_name: module_type [params]

# Connections
destination <- source
module.input <- source
destination <- module.output

# Parameter modulation
module.param <- value
module.param <- source + offset
module.param <- source * scale + offset
```

#### Examples
```
# Simple oscillator
vco1: osc saw 440

# Filter with modulated cutoff
vcf: filter moog
vcf.cutoff <- lfo * 800 + 1000
vcf.res <- 0.7
vcf <- vco1

# Output
out <- vcf * 0.5
```

### Signal Types

1. **Audio Rate** (~44.1kHz) - Default for most connections
2. **Control Rate** (~1kHz) - For modulation signals
3. **Trigger/Gate** - Binary signals for events

The system automatically handles rate conversion.

### FunDSP Integration

FunDSP provides excellent building blocks:
```rust
use fundsp::prelude::*;

// Map our modules to fundsp components
match module_type {
    "osc" => {
        match wave {
            "sine" => sine_hz(freq),
            "saw" => saw_hz(freq),
            "square" => square_hz(freq),
        }
    }
    "filter" => {
        match filter_type {
            "moog" => moog_hz(cutoff, resonance),
            "butterworth" => butterpass_hz(cutoff),
        }
    }
}
```

## Module Library

### Oscillators
- `osc` - Multi-wave oscillator (sine, tri, saw, square, pulse)
- `wavetable` - Wavetable oscillator
- `fm` - FM operator with built-in envelope

### Filters  
- `filter` - Multi-mode (lp, hp, bp, notch, moog, ms20)
- `svf` - State variable filter
- `comb` - Comb filter

### Modulators
- `env` - ADSR envelope
- `lfo` - Low frequency oscillator  
- `seq` - Step sequencer
- `random` - Random voltage generator

### Utilities
- `vca` - Voltage controlled amplifier
- `mix` - Multi-input mixer
- `mult` - Multiple (signal splitter)
- `attenuvert` - Attenuator/inverter
- `s&h` - Sample and hold
- `slew` - Slew limiter
- `compare` - Comparator

### Effects
- `delay` - Digital delay
- `reverb` - Algorithmic reverb
- `distort` - Various distortion types
- `chorus` - Chorus/ensemble

## Live Coding Features

### Hot Reloading
- Watch patch files for changes
- Smoothly crossfade between old and new patches
- Preserve module state where possible

### Parameter Automation
```
# Time-based automation
vco.freq <- sine(0.1) * 100 + 440

# Pattern-based
vco.freq <- [220, 330, 440, 330] @ 120bpm

# Random/generative
vco.freq <- random(100, 500) @ 2hz
```

### Performance Controls
```
# Global commands
!tempo 120
!volume 0.8
!panic  # stop all sound

# Module commands  
vco1!mute
vcf!solo
```

## Integration Points

### With Zim-Sequencer
```
# Import sequences from zim
seq <- zim("my_sequence.zim")
vco.freq <- seq.pitch
env <- seq.gate
```

### MIDI
```
# MIDI input as modulation source
vco.freq <- midi.note
vcf.cutoff <- midi.cc(74) * 2000
env <- midi.gate
```

### OSC
```
# Send/receive OSC messages
osc_send("/vco/freq", vco.freq)
lfo.rate <- osc_receive("/1/fader1")
```

## Implementation Strategy

1. **Start minimal** - Just osc, filter, env, vca
2. **Get the patching syntax right** - This is crucial
3. **Build on fundsp** - Don't reinvent DSP
4. **Focus on live coding** - Hot reload from day one
5. **Text-first, GUI never** - Maybe TUI visualization later

## Why This Will Work

- **Leverages existing knowledge** - Modular synth users already understand the concepts
- **Text is powerful** - Version control, sharing, live coding
- **Standing on giants** - FunDSP does the hard DSP work
- **True open source** - No corporate strings attached
- **Focused scope** - Do modular synthesis well, nothing else

## Next Steps

1. Prototype the module trait system
2. Create basic oscillator and filter modules  
3. Implement the patch parser
4. Build the audio graph engine
5. Add hot reloading
6. Create Neovim plugin

This is achievable because we're not building a DAW or competing with commercial products. We're building a tool for musicians who think in signal flow and want to experiment freely.