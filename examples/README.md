# Zim-DSP Examples

This directory contains example patches demonstrating the zim-dsp modular synthesizer system.

## Patch Files (.zim)

### 01_basic_sine.zim
The simplest possible patch - a sine wave oscillator connected to the output.

### 02_waveforms.zim
Demonstrates the different waveform outputs available from oscillators (sine, saw, square, triangle).

### 03_amplitude_control.zim
Shows how to use a VCA (Voltage Controlled Amplifier) with an envelope to control amplitude over time.

### 04_filter_modulation.zim
Demonstrates using an LFO to modulate a filter's cutoff frequency, creating a classic filter sweep effect.

### 05_complex_routing.zim
A more complex patch with multiple oscillators and envelope-controlled filtering.

### 06_audio_rate_modulation.zim
Demonstrates the Serge philosophy of using audio-rate signals as control voltages for AM synthesis.

## Running Examples

To play an example patch:
```bash
cargo run -- play examples/01_basic_sine.zim
```

To load an example in the REPL:
```bash
cargo run -- repl
> load examples/03_amplitude_control.zim
> start
```

## Key Concepts

1. **Named Ports**: All connections specify both the source and destination ports explicitly (e.g., `vca.audio <- vco.sine`)

2. **Multiple Outputs**: Modules can have multiple outputs. Oscillators provide sine, saw, square, and triangle simultaneously.

3. **Expression Support**: Connections can include scaling and offset operations (e.g., `vcf.cutoff <- lfo.sine * 1000 + 1500`)

4. **Audio-Rate Everything**: Following Serge design philosophy, any signal can be used at audio rate for modulation.

## Code Examples

The `.rs` files demonstrate how to use the graph execution engine programmatically:

- `graph_test.rs` - Basic graph construction and execution
- `introspection_demo.rs` - Module introspection capabilities