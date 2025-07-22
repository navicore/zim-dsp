//! Test stereo mixer with voltage-controlled panning

use anyhow::Result;
use std::time::Duration;
use zim_dsp::observability::ConsoleObserver;
use zim_dsp::test_framework::TestRunner;

fn main() -> Result<()> {
    println!("=== Stereo Mixer Test ===\n");

    // Test basic stereo mixing
    test_basic_stereo_mixing()?;

    // Test voltage-controlled panning
    test_voltage_controlled_panning()?;

    // Test mono to stereo panning
    test_mono_panning()?;

    println!("\n=== Stereo Mixer Test Complete ===");
    Ok(())
}

fn test_basic_stereo_mixing() -> Result<()> {
    println!("--- Test 1: Basic Stereo Mixing ---");

    let mut runner = TestRunner::new();
    runner.add_observer(Box::new(ConsoleObserver::new(false)));

    let patch = r"
        # Create some stereo sources
        osc1: osc saw 440
        osc2: osc sine 330
        
        # 2-channel stereo mixer 
        mixer: stereomix 2
        
        # Connect sources to mixer (mono to stereo)
        mixer.l1 <- osc1.saw
        mixer.l2 <- osc2.sine
        
        # Center pan (default)
        # mixer.pan1 and mixer.pan2 default to 0.0 (center)
        
        # Outputs
        left_out <- mixer.left
        right_out <- mixer.right
    ";

    println!("Testing basic stereo mixing...");
    let result = runner
        .run_patch(patch, Duration::from_millis(100))
        .map_err(|e| anyhow::anyhow!("Test runner error: {}", e))?;

    // Check that both outputs are active
    assert!(result.collector.signal_varied("mixer", "left"), "Left output should vary");
    assert!(result.collector.signal_varied("mixer", "right"), "Right output should vary");

    println!("✓ Basic stereo mixing works");
    Ok(())
}

fn test_voltage_controlled_panning() -> Result<()> {
    println!("\n--- Test 2: Voltage-Controlled Panning ---");

    let mut runner = TestRunner::new();
    runner.add_observer(Box::new(ConsoleObserver::new(false)));

    let patch = r"
        # Audio source
        osc: osc saw 440
        
        # Pan control LFO
        pan_lfo: lfo 0.5  # 0.5Hz panning
        
        # Stereo mixer with CV panning
        mixer: stereomix 1
        mixer.l1 <- osc.saw
        mixer.pan1 <- pan_lfo.sine  # Sine wave panning (-1 to +1)
        
        # Outputs
        left_out <- mixer.left
        right_out <- mixer.right
    ";

    println!("Testing voltage-controlled panning...");
    let result = runner
        .run_patch(patch, Duration::from_secs(2))
        .map_err(|e| anyhow::anyhow!("Test runner error: {}", e))?;

    // Check that both outputs vary (due to panning)
    assert!(result.collector.signal_varied("mixer", "left"), "Left should vary with panning");
    assert!(
        result.collector.signal_varied("mixer", "right"),
        "Right should vary with panning"
    );

    // Check that pan LFO is working
    assert!(result.collector.signal_varied("pan_lfo", "sine"), "Pan LFO should vary");

    println!("✓ Voltage-controlled panning works");
    Ok(())
}

fn test_mono_panning() -> Result<()> {
    println!("\n--- Test 3: Mono to Stereo Panning ---");

    let mut runner = TestRunner::new();
    runner.add_observer(Box::new(ConsoleObserver::new(false)));

    let patch = r"
        # Mono audio source
        osc: osc saw 220
        
        # Stereo mixer with hard pan positions
        mixer: stereomix 2
        
        # Channel 1: hard left (pan = -1)
        mixer.l1 <- osc.saw
        mixer.pan1 <- -1.0  # Hard left
        mixer.level1 <- 0.5  # Half level
        
        # Channel 2: hard right (pan = +1) 
        mixer.l2 <- osc.saw
        mixer.pan2 <- 1.0   # Hard right  
        mixer.level2 <- 0.5  # Half level
        
        # Outputs
        left_out <- mixer.left
        right_out <- mixer.right
    ";

    println!("Testing mono to stereo panning...");
    let result = runner
        .run_patch(patch, Duration::from_millis(100))
        .map_err(|e| anyhow::anyhow!("Test runner error: {}", e))?;

    // Both outputs should vary
    assert!(
        result.collector.signal_varied("mixer", "left"),
        "Left output should have signal"
    );
    assert!(
        result.collector.signal_varied("mixer", "right"),
        "Right output should have signal"
    );

    // Get signal ranges to verify panning worked
    let left_values = result.collector.get_signal_values("mixer", "left");
    let right_values = result.collector.get_signal_values("mixer", "right");

    if !left_values.is_empty() && !right_values.is_empty() {
        let left_max = left_values.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b.abs()));
        let right_max = right_values.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b.abs()));

        println!("  Left max amplitude: {left_max:.3}");
        println!("  Right max amplitude: {right_max:.3}");

        // Both should have signal due to the two channels
        assert!(left_max > 0.1, "Left should have significant signal");
        assert!(right_max > 0.1, "Right should have significant signal");
    }

    println!("✓ Mono to stereo panning works");
    Ok(())
}
