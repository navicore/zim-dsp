# Zim-DSP Design Notes

## Module Import System Design Spike

### Core Concept: Reusable Modular Voices
- Package complete synth voices (VCO+VCF+VCA+ENV) as importable modules
- Enable multiple instances of the same voice in larger patches
- Mirror real modular synthesist workflow: stable voices that rarely get unpatched

### Import Syntax & Search Path
```zim
import noise_snare as snare     # searches for noise_snare.zim
import voice as lead           # searches for voice.zim  
import drums/kick as kick      # relative path + search
```

**Search Order (specific → general):**
1. Relative to current file: `./noise_snare.zim`
2. User modules directory: `~/.config/zim-dsp/modules/`
3. System modules directory: `/usr/local/share/zim-dsp/modules/`
4. Built-in modules: Compiled standard library

### Explicit Patchbay Interface
**Decision**: Explicit patchbay declaration required for importable modules

```zim
# voice.zim - Importable module
patchbay:
  gate: port 1
  pitch: port 2  
  filter_mod: port 3
  audio_out: port 4

# Private implementation - names don't matter externally
main_osc: osc saw
main_filter: filter moog
# ... internal routing
```

**Rejected**: Implicit exposure of unconnected ports (namespace nightmare)

### Module Composition & Hierarchical Imports
- Modules can import other modules (powerful composition)
- Creates natural hierarchy: song.zim → drum_kit.zim → kick.zim
- Each import creates separate instance unless shared

### Circular Dependencies: Fatal by Design
**Core Principle**: Model analog reality - no circular module dependencies exist in hardware

**Physical Reality Check**: You cannot have a filter module that contains a delay module that contains a filter module (impossible to manufacture)

**Solution**: Crash early with clear error messages
- Implement cycle detection during parse phase
- Provide helpful error context showing dependency chain
- Guide users toward internal feedback solutions

### Feedback vs Circular Dependencies
**❌ Circular Dependency (Impossible):**
```zim
# filter.zim imports delay.zim
# delay.zim imports filter.zim  
# Physical impossibility!
```

**✅ Internal Feedback (Natural):**
```zim
# feedback_delay.zim - Single module with patch cable feedback
filter: filter moog
delay: delay 0.5
filter.in <- input + delay.out * 0.3  # Internal feedback routing
delay.in <- filter.out
output <- delay.out
```

### Matrix Mixer: The Ultimate Feedback Machine
- Matrix mixers provide any-to-any routing with level control
- Enable complex feedback networks within single modules
- No need for circular dependencies when you have proper patch bay routing
- Models real modular synthesizer feedback capabilities

### File Type Distinction
- **Modules** (have patchbay): Reusable components, can be imported
- **Patches** (no patchbay): Final compositions, cannot be imported
- Attempting to import file without patchbay = error

### Configuration
```toml
# ~/.config/zim-dsp/config.toml
[modules]
search_paths = [
    "~/my-zim-modules",
    "~/band-shared/zim-modules", 
    "/usr/local/share/zim-dsp/modules"
]
```

### Implementation Notes
- Need dependency cycle detector with clear error reporting
- Deep error context chains for nested module issues
- Performance consideration: efficient instance management
- Import resolution during parse phase

### Benefits
1. **Clean Interface Contract**: Patchbay defines module API
2. **Implementation Hiding**: Internal names can change
3. **Forced Good Design**: Must think about interface to make importable
4. **Zero Namespace Pollution**: Only patchbay ports exposed
5. **Self-Documenting**: Patchbay shows exactly how to use module
6. **Analog Modeling**: Matches real modular synthesizer workflow

---

*Design spike completed: Ready to implement formal grammar with these foundations*