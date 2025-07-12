# 🎚️ Zim-DSP

A text-based modular synthesizer environment built on Rust's open audio ecosystem. Like patching hardware modules, but with the power of text and code.

## 🎯 Vision

Bring the modular synthesis paradigm to text:
- First-class modules: VCO, VCF, VCA, LFO, ENV, etc.
- Patch with simple syntax: `vco -> vcf -> vca -> out`
- Discover emergent behaviors through experimentation
- True open source - MIT licensed all the way down

## 🏗️ Architecture Ideas

### Core Concepts
```
# A simple patch
vco: saw 440
lfo: sine 0.5
vcf: moog 1000 0.7
  <- vco
  <- lfo * 800 + 1000  # modulate cutoff

out <- vcf * 0.5
```

### Design Principles
1. **Modular-first** - Everything is a patchable module
2. **Text-native** - Designed for text files, not GUI
3. **Live-codeable** - Change patches while running
4. **Surprising** - Enable happy accidents and feedback patches
5. **Unencumbered** - Built on truly open source foundations

## 🛠️ Technical Foundation

Built on MIT/Apache licensed crates:
- **fundsp** - Core DSP graph engine
- **cpal** - Cross-platform audio I/O  
- **ratatui** - (Optional) TUI visualization
- **Neovim integration** - Like zim-sequencer

## 📝 Module Ideas

### Essential Modules
- `osc~` - Multi-waveform oscillator (sine, saw, square, tri)
- `filter~` - Various types (moog, ms20, svf)
- `env~` - ADSR envelope generator
- `lfo` - Low frequency oscillator
- `vca` - Voltage controlled amplifier
- `noise~` - White, pink, brown
- `mix~` - Multi-input mixer
- `seq` - Step sequencer
- `clock` - Master clock/divider

### Creative Modules  
- `compare~` - Comparator for generative patches
- `sample~` - Sample and hold
- `delay~` - Digital delay line
- `reverb~` - Algorithmic reverb
- `granular~` - Granular synthesis
- `fold~` - Wavefolding distortion

## 🎮 Usage Examples

### Basic Subtractive Patch
```
vco1: saw 110
vco2: saw 110.5  # slight detune
mix: vco1 + vco2

lfo: tri 0.2
vcf: moog 800 0.8
  <- mix
  <- lfo * 600 + 800

env: adsr 10 100 0.7 500
vca: vcf * env

out <- vca * 0.5
```

### Generative Chaos Patch
```
noise: white
sh: sample_hold
  <- noise
  <- clock 8

vco: square
  <- sh * 200 + 200  # random pitches

compare: noise > 0.3
gate: vco * compare  # gated by noise

out <- gate -> delay 250 0.6
```

## 🚀 Development Phases

### Phase 1: Core Engine
- [ ] Basic module trait system
- [ ] Audio graph processing  
- [ ] Text parser for patch notation
- [ ] Essential modules (osc, filter, env)

### Phase 2: Live Environment
- [ ] Hot-reload patches
- [ ] Neovim plugin
- [ ] Parameter automation
- [ ] MIDI input

### Phase 3: Extended Modules
- [ ] Effects (delay, reverb, distortion)
- [ ] Sequencing modules  
- [ ] CV utilities (attenuverters, logic, etc)
- [ ] Sample playback

### Phase 4: Integration
- [ ] Zim-sequencer interop
- [ ] OSC support
- [ ] Plugin API for custom modules

## 🤝 Philosophy

This project embraces:
- True open source collaboration
- Modular synthesis as a creative practice
- Text as a powerful interface for music
- Emergent complexity from simple components

## 📄 License

MIT - Because audio tools should be truly free

---

*For musicians who think in signal flow and love surprises*