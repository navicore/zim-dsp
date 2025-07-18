//! Test framework for zim-dsp using the observability system
//!
//! Provides utilities for testing module behavior and signal flow

use crate::graph_engine::GraphEngine;
use crate::observability::{ObservationCollector, ObserverManager, SignalObserver};
use std::time::Duration;

/// Result of running a test patch
pub struct TestResult {
    pub collector: ObservationCollector,
    pub duration_seconds: f32,
    pub sample_rate: f32,
}

impl TestResult {
    /// Assert that a signal varied during the test
    /// Assert that a signal varied during the test
    ///
    /// # Errors
    /// Returns an error if the signal did not vary during the test
    pub fn assert_signal_varied(&self, module: &str, port: &str) -> Result<(), String> {
        if self.collector.signal_varied(module, port) {
            Ok(())
        } else {
            Err(format!("Signal {module}.{port} did not vary during test"))
        }
    }

    /// Assert that a gate fired during the test
    /// Assert that a gate fired during the test
    ///
    /// # Errors
    /// Returns an error if the gate did not fire during the test
    pub fn assert_gate_fired(&self, module: &str, gate: &str) -> Result<(), String> {
        if self.collector.gate_fired(module, gate) {
            Ok(())
        } else {
            Err(format!("Gate {module}.{gate} did not fire during test"))
        }
    }

    /// Assert that a signal stayed within a range
    /// Assert that a signal stayed within a range
    ///
    /// # Errors
    /// Returns an error if the signal went outside the expected range
    pub fn assert_signal_range(
        &self,
        module: &str,
        port: &str,
        min: f32,
        max: f32,
    ) -> Result<(), String> {
        if let Some((actual_min, actual_max)) = self.collector.signal_range(module, port) {
            if actual_min >= min && actual_max <= max {
                Ok(())
            } else {
                Err(format!(
                    "Signal {module}.{port} range [{actual_min:.3}, {actual_max:.3}] outside expected [{min:.3}, {max:.3}]"
                ))
            }
        } else {
            Err(format!("No signal data found for {module}.{port}"))
        }
    }

    /// Get the number of times a gate fired
    /// Get the number of times a gate fired
    #[must_use]
    pub fn gate_fire_count(&self, module: &str, gate: &str) -> usize {
        self.collector
            .get_gate_events(module, gate)
            .iter()
            .filter(|event| event.triggered)
            .count()
    }

    /// Get signal values for analysis
    /// Get signal values for analysis
    #[must_use]
    pub fn get_signal_values(&self, module: &str, port: &str) -> Vec<f32> {
        self.collector.get_signal_values(module, port)
    }
}

/// Test runner that can execute patches with observation
pub struct TestRunner {
    engine: GraphEngine,
    observers: ObserverManager,
}

impl TestRunner {
    #[must_use]
    pub fn new() -> Self {
        Self {
            engine: GraphEngine::new(),
            observers: ObserverManager::new(),
        }
    }

    /// Add an observer to collect data during tests
    pub fn add_observer(&mut self, observer: Box<dyn SignalObserver>) {
        self.observers.add_observer(observer);
    }

    /// Run a patch for a specified duration and collect observations
    /// Run a patch for a specified duration and collect observations
    ///
    /// # Errors
    /// Returns an error if the patch fails to load or start
    pub fn run_patch(
        &mut self,
        patch_content: &str,
        duration: Duration,
    ) -> Result<TestResult, String> {
        // Load the patch
        self.engine
            .load_patch(patch_content)
            .map_err(|e| format!("Failed to load patch: {e}"))?;

        // Start the engine
        self.engine.start().map_err(|e| format!("Failed to start engine: {e}"))?;

        // Create a collector to capture observations
        let collector = ObservationCollector::new();

        // TODO: This is where we'd integrate with the actual graph processing
        // For now, we'll simulate some basic behavior
        let sample_rate = 44100.0;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let total_samples = (duration.as_secs_f32() * sample_rate) as usize;
        let buffer_size = 512;

        // Process in chunks
        for cycle in 0..(total_samples / buffer_size) {
            self.observers.begin_process_cycle(cycle);

            // TODO: Process audio and capture observations
            // This is where we'd hook into the actual graph processing

            self.observers.end_process_cycle(cycle);
        }

        // Stop the engine
        self.engine.stop();

        Ok(TestResult {
            collector,
            duration_seconds: duration.as_secs_f32(),
            sample_rate,
        })
    }

    /// Run a test patch from a file
    /// Run a test patch from a file
    ///
    /// # Errors
    /// Returns an error if the file cannot be read or the patch fails to run
    pub fn run_patch_file(&mut self, path: &str, duration: Duration) -> Result<TestResult, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read patch file {path}: {e}"))?;

        self.run_patch(&content, duration)
    }
}

impl Default for TestRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observability::ConsoleObserver;

    #[test]
    fn test_framework_basic() {
        let mut runner = TestRunner::new();

        // Add a console observer for debugging
        runner.add_observer(Box::new(ConsoleObserver::new(false)));

        // Simple test patch
        let patch = r"
            osc: osc sine 440
            out <- osc.sine * 0.5
        ";

        let result = runner.run_patch(patch, Duration::from_secs(1));
        assert!(result.is_ok());

        let test_result = result.unwrap();
        assert!((test_result.duration_seconds - 1.0).abs() < f32::EPSILON);
        assert!((test_result.sample_rate - 44100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_assertion_helpers() {
        let collector = ObservationCollector::new();

        // The collector should work even with no data
        assert!(!collector.signal_varied("test", "port"));
        assert!(!collector.gate_fired("test", "gate"));
        assert!(collector.signal_range("test", "port").is_none());
    }
}
