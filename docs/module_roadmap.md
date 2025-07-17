# Zim-DSP Module Roadmap

## Core Infrastructure Changes

### 1. Stereo Output Support
- **Priority**: High
- **Changes needed**:
  - Modify audio engine to support stereo output
  - Update `out` to accept both `out.left` and `out.right` 
  - Auto-normalize: if only `out.left` is patched, copy to right channel
  - Support mono compatibility: `out <- signal` routes to both channels
- **Implementation**:
  ```zim
  # Mono output (duplicated to both channels)
  out <- vca.out
  
  # Stereo output
  out.left <- vca1.out
  out.right <- vca2.out
  
  # Just left (auto-normalized to right)
  out.left <- vca.out
  ```

## New Modules

### 2. Noise Generator Module
- **Priority**: High (foundational!)
- **Outputs**:
  - **white**: Full spectrum white noise
  - **pink**: Pink noise (-3dB/octave)
  - **brown**: Brown/red noise (-6dB/octave)
  - **blue**: Blue noise (+3dB/octave)
  - **violet**: Violet noise (+6dB/octave)
- **Usage examples**:
  ```zim
  noise: noise_gen
  
  # White noise for snare drum
  vca.audio <- noise.white
  
  # Pink noise for wind sounds
  vcf.audio <- noise.pink
  
  # Sample & hold randomness
  sh.in <- noise.white
  
  # Mix noise with oscillator
  mix.in1 <- vco.saw
  mix.in2 <- noise.white * 0.1  # Just a touch of noise
  ```

### 3. Mixer Modules
- **mono_mixer**: Simple mono mixer (2-4 inputs)
  ```zim
  mix1: mono_mixer
  mix1.in1 <- vco1.sine
  mix1.in2 <- vco2.saw
  mix1.in3 <- noise.out
  vcf.audio <- mix1.out
  ```
  
- **stereo_mixer**: Stereo mixer with pan controls
  ```zim
  mix: stereo_mixer
  mix.in1.left <- vco1.sine
  mix.in1.right <- vco1.sine
  mix.pan1 <- lfo.sine * 0.5 + 0.5  # LFO panning
  out.left <- mix.left
  out.right <- mix.right
  ```

### 3. Sequencer Module
- **8-step sequencer** with CV and gate outputs
- **Features**:
  - 8 steps with individual CV values
  - Gate output for each step
  - Clock input
  - Reset input
  - Direction control (forward/backward/pendulum/random)
- **Integration opportunity**: This is where we could bring in zim-sequencer DSL
  ```zim
  seq: sequencer
  seq.clock <- clock.gate
  seq.reset <- manual_reset.gate
  seq.values <- [C4, E4, G4, C5, G4, E4, C4, rest]  # zim-sequencer syntax?
  vco.freq <- seq.cv
  env.gate <- seq.gate
  ```

### 4. Slew Generator / Lag Processor
- **Purpose**: Smooth out stepped CV signals, create portamento
- **Features**:
  - Separate rise and fall times
  - Linear and exponential modes
  ```zim
  slew: slew_gen 0.1 0.05  # 100ms rise, 50ms fall
  slew.in <- seq.cv
  vco.freq <- slew.out
  ```

### 5. Additional Utility Modules

#### Sample & Hold
```zim
sh: sample_hold
sh.in <- noise.out
sh.trig <- clock.gate
vco.freq <- sh.out * 1000 + 440
```

#### Attenuverter
```zim
atten: attenuverter -1 1  # min/max range
atten.in <- lfo.sine
atten.cv <- knob.value  # -1 to 1, 0 = no signal
vcf.cutoff <- atten.out * 2000 + 1000
```

#### Comparator
```zim
comp: comparator 0.5  # threshold
comp.in <- lfo.triangle
env.gate <- comp.out
```

#### Clock Divider
```zim
div: clock_div 4  # divide by 4
div.in <- master_clock.gate
seq.clock <- div.out
```

### 6. Distortion & Character Modules

#### Wavefolder
```zim
folder: wavefolder 0.5  # fold amount
folder.in <- vco.sine
folder.cv <- lfo.triangle  # CV control of fold amount
vcf.audio <- folder.out
```

#### Waveshaper
```zim
shaper: waveshaper tanh  # tanh, clip, soft, tube, etc.
shaper.in <- vco.saw
shaper.drive <- 2.0  # drive amount
shaper.cv <- env.out  # CV control of drive
vca.audio <- shaper.out
```

#### Bitcrusher
```zim
crush: bitcrusher 8 8000  # 8 bits, 8kHz sample rate
crush.in <- vco.square
crush.bits <- 4  # Reduce to 4 bits
crush.rate <- 4000  # Reduce to 4kHz
# CV control
crush.bits_cv <- lfo.sine * 8 + 8  # 4-12 bits
crush.rate_cv <- lfo.triangle * 4000 + 4000  # 2-8kHz
out <- crush.out
```

## Implementation Priority

1. **Phase 1** (Foundation):
   - Stereo output support
   - Noise generator (white, pink, brown minimum)
   - Mono mixer (essential for basic patches)
   - Slew generator (essential for musical sequences)

2. **Phase 2** (Musical Features):
   - 8-step sequencer
   - Sample & hold
   - Clock divider

3. **Phase 3** (Distortion & Character):
   - Bitcrusher (most digital-native)
   - Wavefolder (classic analog behavior)
   - Waveshaper (multiple algorithms)

4. **Phase 4** (Advanced):
   - Stereo mixer with panning
   - Attenuverter
   - Comparator
   - Integration with zim-sequencer DSL

## Technical Considerations

### Stereo Signal Flow
- All modules currently output mono signals
- Need to decide on stereo signal representation:
  - Option 1: Separate left/right ports on modules
  - Option 2: Stereo signal type that carries both channels
  - Recommendation: Use separate ports for maximum flexibility

### Sequencer Integration
- Could parse zim-sequencer note syntax within sequencer module
- Convert note names to frequencies
- Support rest/tie notation
- Potential for embedding full zim-sequencer patterns

### Performance
- Mixers need efficient summing
- Sequencer needs efficient step tracking
- Consider SIMD optimizations for stereo processing

## Example Patch Goals

### Classic Mono Synth with Sequencer
```zim
clock: lfo 2.0  # 2 Hz clock
seq: sequencer
seq.clock <- clock.gate
seq.values <- [C3, C3, Eb3, G3, C4, Bb3, G3, Eb3]

slew: slew_gen 0.05 0.05  # portamento
slew.in <- seq.cv
vco: osc saw
vco.freq <- slew.out

env: envelope 0.001 0.2
env.gate <- seq.gate

vcf: filter moog
vcf.audio <- vco.saw
vcf.cutoff <- env.out * 4000 + 200

vca: vca
vca.audio <- vcf.lp
vca.cv <- env.out

out <- vca.out
```

### Classic Drum Sounds with Noise
```zim
# Snare drum
noise: noise_gen
env_snare: envelope 0.001 0.15
vca_snare: vca

# Trigger snare with manual gate
gate: manual
env_snare.gate <- gate.gate

# High-passed white noise
hpf: filter
hpf.audio <- noise.white
hpf.cutoff <- 2000

# Shape with envelope
vca_snare.audio <- hpf.hp
vca_snare.cv <- env_snare.out

# Mix with a pitched component
osc_snap: osc sine 200
vca_snap: vca
vca_snap.audio <- osc_snap.sine
vca_snap.cv <- env_snare.out

mix: mono_mixer
mix.in1 <- vca_snare.out
mix.in2 <- vca_snap.out * 0.3
out <- mix.out
```

### Wind and Atmosphere
```zim
noise: noise_gen
lfo_wind: lfo 0.1  # Slow modulation

# Use pink noise for more natural sound
vcf: filter
vcf.audio <- noise.pink

# Modulate filter cutoff for wind effect
vcf.cutoff <- lfo_wind.sine * 500 + 800

# Add some resonance for whistling
vcf.res <- 0.7

out <- vcf.lp * 0.5
```

### Random Sample & Hold Melody
```zim
noise: noise_gen
clock: lfo 4  # 4 Hz clock
sh: sample_hold

# Sample white noise
sh.in <- noise.white
sh.trig <- clock.gate

# Scale and offset to musical range
vco: osc saw
vco.freq <- sh.out * 500 + 200  # 200-700 Hz range

# Simple envelope
env: envelope 0.01 0.1
env.gate <- clock.gate

vca: vca
vca.audio <- vco.saw
vca.cv <- env.out

out <- vca.out
```

### Stereo Ambient Patch
```zim
# Two detuned oscillators
vco1: osc sine 220
vco2: osc sine 220.5

# Stereo mixer with LFO panning
mix: stereo_mixer
mix.in1 <- vco1.sine
mix.in2 <- vco2.sine
mix.pan1 <- lfo1.sine * 0.5 + 0.5
mix.pan2 <- lfo2.sine * 0.5 + 0.5

# Different LFO rates for movement
lfo1: lfo 0.1
lfo2: lfo 0.13

# Output
out.left <- mix.left
out.right <- mix.right
```