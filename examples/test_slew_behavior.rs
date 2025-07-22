//! Example demonstrating how to use the test framework to verify slew behavior
//!
//! This shows how we can write tests that verify specific module behaviors
//! without having to listen to audio output.

use std::time::Duration;
use zim_dsp::observability::ConsoleObserver;
use zim_dsp::test_framework::TestRunner;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing slew generator behavior...");

    // Create a test runner
    let mut runner = TestRunner::new();

    // Add observers
    runner.add_observer(Box::new(ConsoleObserver::new(false)));

    // Test patch: slew generator with CV control
    let patch = r"
        # Clock to drive changes
        clock: lfo 1.0
        
        # Sequencer to provide different slew rates
        rate_seq: seq8
        rate_seq.clock <- clock.gate
        rate_seq.step1 <- 0.1   # Fast slew
        rate_seq.step2 <- 1.0   # Slow slew
        
        # Slew generator with CV control
        slew1: slew 0.5 0.5
        slew1.in <- clock.square
        slew1.rise <- rate_seq.cv
        
        # Output for monitoring
        out <- slew1.out
    ";

    println!("Running patch for 3 seconds...");
    let result = runner.run_patch(patch, Duration::from_secs(3))?;

    println!("Test completed!");
    println!("Duration: {:.1}s", result.duration_seconds);
    println!("Sample rate: {:.0}Hz", result.sample_rate);

    // These are the kinds of assertions we want to be able to make:
    // (They won't work yet because we haven't integrated with the actual audio processing)

    /*
    // Test that slew rate actually changed
    result.assert_signal_varied("rate_seq", "cv")
        .expect("Rate sequencer should output different values");

    // Test that slew output shows different behavior
    result.assert_signal_varied("slew1", "out")
        .expect("Slew output should vary");

    // Test that gates fire
    result.assert_gate_fired("slew1", "eoc")
        .expect("End-of-cycle gate should fire");

    result.assert_gate_fired("slew1", "eor")
        .expect("End-of-rise gate should fire");

    println!("All assertions passed!");
    */

    println!("Framework is ready - next step is to integrate with actual audio processing");

    Ok(())
}
