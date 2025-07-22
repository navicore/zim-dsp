//! Demonstration of slew generator CV control capabilities

#![allow(clippy::wildcard_imports)]

use anyhow::Result;
use std::time::Duration;
use zim_dsp::observability::ConsoleObserver;
use zim_dsp::test_framework::TestRunner;

fn main() -> Result<()> {
    println!("=== Debugging Slew Generator ===\n");

    // Test 1: Basic slew functionality
    test_basic_slew_functionality()?;

    // Test 2: CV-controlled slew rates
    test_cv_controlled_slew_rates()?;

    // Test 3: Gate outputs (EOR and EOC)
    test_gate_outputs()?;

    println!("\n=== Slew Generator Debug Complete ===");
    Ok(())
}

fn test_basic_slew_functionality() -> Result<()> {
    println!("--- Test 1: Basic Slew Functionality ---");

    let mut runner = TestRunner::new();
    runner.add_observer(Box::new(ConsoleObserver::new(false)));

    // Simple test: step input should be smoothed by slew
    let patch = r"
        # Manual gate to create step input
        gate: gate manual
        
        # Slew generator with medium times
        slew: slew 0.2 0.3
        slew.in <- gate.out
        
        # Output for monitoring
        out <- slew.out
    ";

    println!("Running basic slew test...");
    let result = runner
        .run_patch(patch, Duration::from_secs(2))
        .map_err(|e| anyhow::anyhow!(e))?;

    // Check that the slew output varies (should be smoothing the step)
    if result.assert_signal_varied("slew", "out") == Ok(()) {
        println!("✓ Slew output varies (smoothing is working)");
    } else {
        println!("✗ Slew output doesn't vary (smoothing broken)");
    }

    // Check that output is bounded
    if result.assert_signal_range("slew", "out", -0.1, 1.1) == Ok(()) {
        println!("✓ Slew output is within expected range");
    } else {
        println!("✗ Slew output out of range");
    }

    println!();
    Ok(())
}

fn test_cv_controlled_slew_rates() -> Result<()> {
    println!("--- Test 2: CV-Controlled Slew Rates ---");

    let mut runner = TestRunner::new();
    runner.add_observer(Box::new(ConsoleObserver::new(false)));

    // Test CV control of slew rates using a sequencer
    let patch = r"
        # Clock to drive changes
        clock: lfo 1.0
        
        # Sequencer to provide different slew rates
        rate_seq: seq8
        rate_seq.clock <- clock.gate
        rate_seq.step1 <- 0.1   # Fast slew
        rate_seq.step2 <- 1.0   # Slow slew
        
        # Slew generator with CV-controlled rise time
        slew: slew 0.5 0.5
        slew.in <- clock.square
        slew.rise <- rate_seq.cv
        
        # Output for monitoring
        out <- slew.out
    ";

    println!("Running CV control test...");
    let result = runner
        .run_patch(patch, Duration::from_secs(4))
        .map_err(|e| anyhow::anyhow!(e))?;

    // Check that both the slew output and rate CV vary
    if result.assert_signal_varied("slew", "out") == Ok(()) {
        println!("✓ Slew output varies");
    } else {
        println!("✗ Slew output doesn't vary");
    }

    if result.assert_signal_varied("rate_seq", "cv") == Ok(()) {
        println!("✓ Rate CV varies");
    } else {
        println!("✗ Rate CV doesn't vary");
    }

    // The rate CV should affect the slew behavior
    // This is hard to test automatically, but we can at least verify signal variation
    println!("Note: Visual/audio inspection needed to verify CV control is working");

    println!();
    Ok(())
}

fn test_gate_outputs() -> Result<()> {
    println!("--- Test 3: Gate Outputs (EOR and EOC) ---");

    let mut runner = TestRunner::new();
    runner.add_observer(Box::new(ConsoleObserver::new(true))); // Verbose for gate debugging

    // Test gate outputs with manual trigger
    let patch = r"
        # Manual gate to create step changes
        trigger: gate manual
        
        # Slew generator with short times to ensure completion
        slew: slew 0.1 0.1
        slew.in <- trigger.out
        
        # Output the main signal and gates
        out <- slew.out
        eor_out <- slew.eor
        eoc_out <- slew.eoc
    ";

    println!("Running gate output test...");
    let result = runner
        .run_patch(patch, Duration::from_secs(2))
        .map_err(|e| anyhow::anyhow!(e))?;

    // Check if gates fired at all
    if result.assert_gate_fired("slew", "eor") == Ok(()) {
        println!("✓ End-of-rise gate fired");
    } else {
        println!("✗ End-of-rise gate never fired");
    }

    if result.assert_gate_fired("slew", "eoc") == Ok(()) {
        println!("✓ End-of-cycle gate fired");
    } else {
        println!("✗ End-of-cycle gate never fired");
    }

    // Count gate fires
    let end_of_rise_count = result.gate_fire_count("slew", "eor");
    let end_of_cycle_count = result.gate_fire_count("slew", "eoc");

    println!("EOR gate fired {end_of_rise_count} times");
    println!("EOC gate fired {end_of_cycle_count} times");

    if end_of_rise_count > 0 && end_of_cycle_count > 0 {
        println!("✓ Both gate types fired");
    } else {
        println!("✗ One or both gate types never fired");
    }

    println!();
    Ok(())
}
