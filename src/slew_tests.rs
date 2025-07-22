//! Comprehensive tests for slew generator functionality

#[cfg(test)]
mod tests {
    use crate::test_framework::TestRunner;
    use std::time::Duration;

    #[test]
    fn test_slew_basic_smoothing() {
        let mut runner = TestRunner::new();

        // Basic smoothing with LFO square wave
        let patch = r"
            lfo: lfo 2.0
            slew: slew 0.1 0.1
            slew.in <- lfo.square
            out <- slew.out
        ";

        let result = runner
            .run_patch(patch, Duration::from_secs(2))
            .expect("Patch should load and run successfully");

        // Assertions
        assert!(result.collector.signal_varied("lfo", "square"), "LFO square wave should vary");
        assert!(
            result.collector.signal_varied("slew", "out"),
            "Slew output should vary (smoothing working)"
        );

        // Check range is reasonable
        let slew_range = result.collector.signal_range("slew", "out");
        assert!(slew_range.is_some(), "Slew should produce signal values");

        let (min, max) = slew_range.unwrap();
        assert!(min < max, "Slew output should have a range");
        assert!(min > -1.1 && max < 1.1, "Slew output should stay in reasonable bounds");
    }

    #[test]
    fn test_slew_cv_override() {
        let mut runner = TestRunner::new();

        // Test that CV values override parameters
        let patch = r"
            lfo: lfo 2.0
            
            # Constant CV to override rise time (slower than parameter)
            slow_cv: lfo 0.0  # 0Hz = constant
            
            slew: slew 0.05 0.05  # Fast parameters
            slew.in <- lfo.square
            slew.rise <- slow_cv.sine * 0.1 + 0.2  # 0.2V constant = 200ms rise
            out <- slew.out
        ";

        let result = runner
            .run_patch(patch, Duration::from_secs(2))
            .expect("CV control patch should work");

        // Basic functionality
        assert!(
            result.collector.signal_varied("slew", "out"),
            "CV-controlled slew should still produce varying output"
        );

        // The CV-controlled version should behave differently than pure parameters
        // (This is more of a smoke test - detailed behavior verification would need more complex analysis)
        let values = result.collector.get_signal_values("slew", "out");
        assert!(!values.is_empty(), "Should capture slew output values");
    }

    #[test]
    fn test_slew_dynamic_cv() {
        let mut runner = TestRunner::new();

        // Test dynamic CV control
        let patch = r"
            lfo: lfo 1.0
            rate_lfo: lfo 0.5  # Slow modulation of slew rate
            
            slew: slew 0.1 0.1
            slew.in <- lfo.square
            slew.rise <- rate_lfo.sine * 0.1 + 0.15  # 0.05V to 0.25V range
            out <- slew.out
        ";

        let result = runner
            .run_patch(patch, Duration::from_secs(3))
            .expect("Dynamic CV patch should work");

        // All signals should vary
        assert!(result.collector.signal_varied("lfo", "square"), "Main LFO should vary");
        assert!(
            result.collector.signal_varied("rate_lfo", "sine"),
            "Rate control LFO should vary"
        );
        assert!(
            result.collector.signal_varied("slew", "out"),
            "Dynamically controlled slew should vary"
        );
    }

    #[test]
    fn test_slew_cv_fallback_to_parameters() {
        let mut runner = TestRunner::new();

        // Test that 0V CV falls back to using parameters
        let patch = r"
            lfo: lfo 2.0
            zero_cv: lfo 0.0  # Always outputs 0
            
            slew: slew 0.08 0.12  # Asymmetric rise/fall
            slew.in <- lfo.square
            slew.rise <- zero_cv.sine  # Should be 0V, fallback to parameter
            slew.fall <- zero_cv.sine  # Should be 0V, fallback to parameter  
            out <- slew.out
        ";

        let result = runner
            .run_patch(patch, Duration::from_secs(2))
            .expect("CV fallback patch should work");

        // Should still work normally when CV is 0
        assert!(
            result.collector.signal_varied("slew", "out"),
            "Slew with 0V CV should fallback to parameters and work"
        );

        // Verify the CV is actually near zero
        let cv_values = result.collector.get_signal_values("zero_cv", "sine");
        assert!(!cv_values.is_empty(), "Should have CV values");

        // Most values should be very close to 0 (sine of 0Hz LFO)
        let near_zero_count = cv_values.iter().filter(|&&v| v.abs() < 0.01).count();
        assert!(
            near_zero_count > cv_values.len() / 2,
            "Most CV values should be near zero for 0Hz LFO"
        );
    }

    #[test]
    fn test_slew_backward_compatibility() {
        let mut runner = TestRunner::new();

        // Test that old patches without CV inputs still work
        let patch = r"
            lfo: lfo 1.5
            slew: slew 0.15 0.1  # Just parameters, no CV connections
            slew.in <- lfo.square
            out <- slew.out
        ";

        let result = runner
            .run_patch(patch, Duration::from_secs(2))
            .expect("Backward compatibility patch should work");

        // Should work exactly like before CV inputs were added
        assert!(
            result.collector.signal_varied("slew", "out"),
            "Slew without CV connections should work like before"
        );

        let range = result.collector.signal_range("slew", "out");
        assert!(range.is_some(), "Should produce reasonable output range");
    }

    #[test]
    fn test_slew_gate_outputs() {
        let mut runner = TestRunner::new();

        // Test gate outputs with slow LFO and fast slew to ensure target reaching
        // The key insight: slew time must be much shorter than LFO period for reliable gate firing
        let patch = r"
            lfo: lfo 0.5  
            slew: slew 0.05 0.05  
            slew.in <- lfo.square
            out <- slew.out
            eor_out <- slew.eor
            eoc_out <- slew.eoc
        ";

        let result = runner
            .run_patch(patch, Duration::from_secs(4))
            .expect("Patch should load and run successfully");

        // Basic functionality should still work
        assert!(result.collector.signal_varied("slew", "out"), "Slew output should vary");

        // Gate outputs should fire
        assert!(result.collector.gate_fired("slew", "eor"), "End-of-rise gate should fire");
        assert!(result.collector.gate_fired("slew", "eoc"), "End-of-cycle gate should fire");

        // Count gate fires
        let rise_gate_fires = result.gate_fire_count("slew", "eor");
        let cycle_gate_fires = result.gate_fire_count("slew", "eoc");

        assert!(rise_gate_fires > 0, "EOR should fire at least once (got {rise_gate_fires})");
        assert!(cycle_gate_fires > 0, "EOC should fire at least once (got {cycle_gate_fires})");

        // With a 0.5Hz LFO over 4 seconds, we should get roughly 1-2 complete cycles
        assert!(
            (1..=3).contains(&rise_gate_fires),
            "EOR count should be 1-3 for 4 second test (got {rise_gate_fires})"
        );
        assert!(
            (1..=3).contains(&cycle_gate_fires),
            "EOC count should be 1-3 for 4 second test (got {cycle_gate_fires})"
        );
    }

    /// Test the critical self-patching oscillator behavior
    /// This test verifies that slew.in <- slew.eor creates a self-sustaining triangle oscillator
    /// This functionality is essential for audio-rate modular synthesis applications
    #[test]
    fn test_slew_self_patching_oscillator() {
        let patch = r"
            # Self-patching slew oscillator - the core test for bootstrap behavior
            slew: slew 0.01 0.01
            slew.in <- slew.eor  # This is the key self-patching connection
            
            # Output for monitoring
            out <- slew.out
            gates <- slew.eor
        ";

        let mut runner = TestRunner::new();
        let result = runner
            .run_patch(patch, Duration::from_millis(100))
            .expect("Self-patching slew oscillator should run successfully");

        // The critical test: self-patching must create oscillation
        assert!(
            result.collector.signal_varied("slew", "out"),
            "Self-patching slew generator should create oscillating output"
        );

        // Gates must fire for self-patching to work
        assert!(
            result.collector.gate_fired("slew", "eor"),
            "Self-patching requires EOR gates to fire"
        );

        // Should see multiple gate fires indicating sustained oscillation
        let gate_count = result.gate_fire_count("slew", "eor");
        assert!(
            gate_count > 2,
            "Self-patching oscillator should fire multiple gates (got {gate_count})"
        );

        // Verify we get reasonable signal range for triangle wave
        let values = result.collector.get_signal_values("slew", "out");
        assert!(!values.is_empty(), "Should capture slew output values");

        let min = values.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let max = values.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        let range = max - min;

        assert!(
            range > 0.5,
            "Self-patching triangle should have reasonable amplitude range (got {range})"
        );
    }
}
