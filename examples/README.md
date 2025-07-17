# Zim-DSP Examples

Examples are organized by the primary module being demonstrated:

## Oscillator Examples (`oscillator/`)
- `basic_sine.zim` - Simple sine wave oscillator
- `waveforms.zim` - Different waveform types (sine, saw, square, triangle)

## Filter Examples (`filter/`)
- `filter_modulation.zim` - LFO modulating filter cutoff
- `wind_effect.zim` - Filtered pink noise creating wind sounds

## Envelope Examples (`envelope/`)
- `amplitude_control.zim` - Basic AD envelope shaping amplitude
- `lfo_triggered_envelope.zim` - LFO triggering envelope

## Noise Examples (`noise/`)
- `white_noise.zim` - Pure white noise
- `pink_noise.zim` - Pink noise (-3dB/octave)
- `brown_noise.zim` - Brown noise (-6dB/octave)
- `hihat_clock.zim` - Musical hi-hat using filtered white noise

## Stereo Examples (`stereo/`)
- `stereo_test.zim` - Left/right channel separation
- `mono_compatibility.zim` - Mono output routing to both channels
- `left_normalization.zim` - Left channel auto-normalizing to right

## Manual Gate Examples (`manual_gate/`)
- `manual_gate.zim` - Basic manual gate usage
- `amplitude_control_manual.zim` - Manual gate triggering envelope

## Mixer Examples (`mixer/`)
- `mono_mixer.zim` - Basic mono mixer with level controls
- `polyrhythm_demo.zim` - Complex polyrhythmic patterns using mixer

## Slew Generator Examples (`slew/`)
- `test_slew.zim` - Simple slew generator test smoothing a square wave
- `smooth_filter.zim` - Using slew to smooth filter cutoff changes
- `basic_portamento.zim` - Basic portamento effect using slew

## Sequencer Examples (`sequencer/`)
- `basic_seq.zim` - Basic 8-step sequencer with clock and gate outputs
- `step_values.zim` - Setting custom step values for melodic sequences
- `gate_pattern.zim` - Creating rhythmic patterns with gate enables
- `reset_demo.zim` - Using reset to create different phrase lengths

## Complex Examples (`complex/`)
- `complex_routing.zim` - Multiple modules with complex routing
- `audio_rate_modulation.zim` - Audio-rate modulation examples

## Running Examples

Run any example with:
```bash
cargo run --release -- play examples/noise/white_noise.zim
```

Or load into the REPL for interactive experimentation:
```bash
cargo run --release -- repl
# Then paste the example contents
```
EOF < /dev/null