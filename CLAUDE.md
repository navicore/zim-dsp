# Claude Code Context for Zim-DSP

## Project Overview
Zim-DSP is a text-based modular synthesizer environment inspired by hardware modular synths and Pure Data, but designed text-first. It's a sibling project to zim-sequencer, sharing the `.zim` file format with eventual convergence planned.

## Key Design Decisions

### Why This Project Exists
- Hardware modular synths are expensive and space-limited
- Pure Data has terrible UX and complex build system  
- VCV Rack has problematic "open source theater" licensing
- We want TRUE open source (MIT all the way down)
- Text-first enables version control, sharing, live coding

### Architecture
```
┌─────────────────────┐
│   Neovim Plugin     │  Development/Live Coding (future)
│  ┌───────┬────────┐ │
│  │ Edit  │ Patch  │ │
│  │ Buffer│ View   │ │
│  └───────┴────────┘ │
└──────────┬──────────┘
           │ stdin/stdout
┌──────────▼──────────┐
│   zim-dsp engine    │  Core DSP Process
│  - Hot reload       │
│  - State management │
│  - Audio processing │
└──────────┬──────────┘
           │ Also speaks OSC/MIDI
┌──────────▼──────────┐
│   Standalone CLI    │  Performance/Installation
│  - Load patch       │
│  - Basic controls   │
│  - Optional TUI     │
└─────────────────────┘
```

### Current Implementation Status
- ✅ Basic CLI with REPL and file playback
- ✅ DSL parser for modules, connections, parameters
- ✅ Module trait system
- ✅ Engine structure with cpal audio output
- ✅ Basic graph building from modules/connections
- ✅ Oscillator module (sine, saw, square, triangle)
- ✅ Filter module (placeholder - just gain stage)
- ✅ Envelope module (AD type)
- ✅ VCA module
- ✅ Simple envelope triggering (restarts graph when envelope present)
- ⚠️  Module trait not fully integrated with fundsp
- ❌ Neovim plugin not started
- ❌ Hot reload not implemented
- ❌ Multi-input routing not implemented

### DSL Syntax
```
# Module creation
vco: osc saw 440

# Connections
vcf <- vco
out <- vcf * 0.5

# Parameter setting
vcf.cutoff <- 800
vcf.res <- 0.7

# Modulation
vcf.cutoff <- lfo * 800 + 1200
```

## Technical Foundation
- **fundsp** - Core DSP library (MIT licensed)
- **cpal** - Cross-platform audio I/O
- **anyhow** - Error handling

## Next Steps

### Immediate (to make it actually work)
1. Implement graph building from modules/connections in `engine.rs`
2. Create fundsp-based implementations for basic modules
3. Make the audio actually respond to the patch

### Short Term
1. Add more module types (filter, envelope, vca, mixer)
2. Implement hot reload (diff patches, crossfade)
3. Better error messages
4. Simple visualization in REPL

### Medium Term
1. Neovim plugin (like zim-sequencer)
2. MIDI input support
3. More complex modules (sequencer, sample player)
4. Performance optimizations

### Long Term
1. Convergence with zim-sequencer
2. OSC support for external control
3. Plugin API for custom modules
4. Optional TUI with waveform display

## Key Files

- `src/main.rs` - CLI entry point
- `src/engine.rs` - Audio engine and patch management
- `src/modules.rs` - Module trait and implementations
- `src/parser.rs` - DSL parser
- `examples/basic_patch.zim` - Example patch file

## Building and Running

```bash
cd /Users/navicore/git/navicore/zim-dsp
cargo build
cargo run -- repl
```

## Design Philosophy

1. **Modular-first** - Everything is a patchable module
2. **Text-native** - Designed for text files, not GUI
3. **Live-codeable** - Change patches while running
4. **Surprising** - Enable happy accidents and feedback patches
5. **Unencumbered** - MIT licensed, no corporate strings

## Relationship to Zim-Sequencer

- Both use `.zim` files
- Context-aware parsing (modules with `:` = dsp, notes = sequencer)
- Future convergence planned where sequences can drive synthesis
- Shared philosophy: text-based musical tools

## Current Challenges

1. Module trait and fundsp AudioUnit integration - currently using a hack where presence of envelope module adds AD envelope to oscillators
2. Need proper module graph execution that processes our Module trait instances
3. Multi-input routing (e.g., envelope -> VCA control input)
4. Hot reload will require careful state management
5. Performance considerations for real-time audio

## Recent Progress

### Modules Implemented
- **Oscillator**: Supports sine, saw, square, triangle waveforms
- **Filter**: Placeholder that applies gain based on cutoff frequency
- **Envelope**: AD envelope generator with attack/decay times
- **VCA**: Voltage controlled amplifier with gain control

### Envelope Integration
Currently, when an envelope module is present in the patch, oscillators automatically get an AD envelope applied. This is a temporary solution - proper implementation needs:
1. A way to execute our Module trait's process() method in the audio callback
2. Multi-input routing so envelope output can modulate VCA/filter
3. Trigger mechanism (currently using engine restart)

### Next Steps for Envelope->VCA
1. Create a module graph executor that can process Module trait objects
2. Implement connection routing between module inputs/outputs
3. Add proper trigger mechanism for envelopes
4. Test with oscillator->VCA (modulated by envelope) patches

## Example of Future Convergence

```zim
# From zim-sequencer
melody: [C4, E4, G4, E4] @ 120bpm

# Used in zim-dsp
vco: osc saw
vco.freq <- melody.hz
env <- melody.gate

vcf: filter moog <- vco
vcf.cutoff <- melody.velocity * 2000
out <- vcf * env
```

This would allow musical sequences to drive modular synthesis parameters.