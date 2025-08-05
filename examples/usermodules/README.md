# User Module Examples

User modules allow you to create reusable components by defining them in `.zim` files in the `usermodules/` directory.

## Basic Usage

1. **Create a user module** in `usermodules/my_module.zim`
2. **Use the module** in any patch with `instance_name: my_module`
3. **Connect to module ports** using `instance_name.input` and `instance_name.output`

## Example: Simple Gain

The `simple_gain` user module is defined in `usermodules/simple_gain.zim`:

```zim
module simple_gain {
    inputs: audio
    outputs: out
    
    # Internal modules
    vca: vca 0.5
    
    # Internal connections
    vca.audio <- $audio
    $out <- vca.out
}
```

### Using the module:

```zim
# Create source
vco: osc sine 440

# Create user module instance  
gain1: simple_gain

# Connect to user module
vco.sine -> gain1.audio
out <- gain1.out
```

### What happens internally:

When you use `gain1: simple_gain`, the system:

1. **Expands the template** to create internal modules with prefixed names
2. **Resolves connections** from user module ports to internal module ports
3. **Creates the audio graph** using the expanded modules

The above example becomes:
```zim
vco: osc sine 440
gain1_vca: vca 0.5              # Expanded from template
vco.sine -> gain1_vca.audio     # Resolved connection
out <- gain1_vca.out            # Resolved connection
```

## Files

- `basic.zim` - Simple example using the gain module
- `multiple.zim` - Using multiple instances of the same user module
- `chain.zim` - Chaining user modules together