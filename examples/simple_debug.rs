//! Simple debug to verify our test framework is working

#![allow(clippy::wildcard_imports)]

use anyhow::Result;
use std::time::Duration;
use zim_dsp::observability::ConsoleObserver;
use zim_dsp::test_framework::TestRunner;

fn main() -> Result<()> {
    println!("=== Debugging the Debugger ===\n");

    let mut runner = TestRunner::new();
    runner.add_observer(Box::new(ConsoleObserver::new(true))); // Verbose output

    // Test 1: Simplest possible case - oscillator
    println!("--- Test 1: Basic Oscillator ---");
    let patch = r"
        osc: osc sine 440
        out <- osc.sine
    ";

    println!("Loading patch...");
    let result = runner
        .run_patch(patch, Duration::from_secs(1))
        .map_err(|e| anyhow::anyhow!(e))?;

    println!("Checking if oscillator varies...");

    // Debug: check what signals we actually captured
    let signal_values = result.get_signal_values("osc", "sine");
    println!("Debug: Captured {} signal values", signal_values.len());
    if !signal_values.is_empty() {
        println!("Debug: First few values: {:?}", &signal_values[..signal_values.len().min(5)]);
    }

    match result.assert_signal_varied("osc", "sine") {
        Ok(()) => println!("✓ Oscillator sine output varies (test framework works!)"),
        Err(e) => println!("✗ Oscillator problem: {e}"),
    }

    Ok(())
}
