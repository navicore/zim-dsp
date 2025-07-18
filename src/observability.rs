//! Observability system for zim-dsp
//!
//! Provides a clean interface for monitoring signals, parameters, and events
//! without polluting the core audio processing code.

/// A single signal observation event
#[derive(Debug, Clone)]
pub struct SignalEvent {
    pub module: String,
    pub port: String,
    pub sample_index: usize,
    pub value: f32,
}

/// A gate/trigger event
#[derive(Debug, Clone)]
pub struct GateEvent {
    pub module: String,
    pub gate: String,
    pub sample_index: usize,
    pub triggered: bool,
}

/// A parameter change event
#[derive(Debug, Clone)]
pub struct ParameterEvent {
    pub module: String,
    pub parameter: String,
    pub value: f32,
}

/// Trait for observing signals and events in the audio graph
pub trait SignalObserver {
    /// Called when a signal value is observed
    fn observe_signal(&mut self, event: &SignalEvent);

    /// Called when a gate/trigger fires
    fn observe_gate(&mut self, event: &GateEvent);

    /// Called when a parameter changes
    fn observe_parameter(&mut self, event: &ParameterEvent);

    /// Called at the beginning of each process cycle
    fn begin_process_cycle(&mut self, _cycle: usize) {}

    /// Called at the end of each process cycle
    fn end_process_cycle(&mut self, _cycle: usize) {}
}

/// Collects observations for testing and analysis
pub struct ObservationCollector {
    pub signals: Vec<SignalEvent>,
    pub gates: Vec<GateEvent>,
    pub parameters: Vec<ParameterEvent>,
    pub cycle_count: usize,
}

impl ObservationCollector {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            signals: Vec::new(),
            gates: Vec::new(),
            parameters: Vec::new(),
            cycle_count: 0,
        }
    }

    /// Get all signal values for a specific module and port
    #[must_use]
    pub fn get_signal_values(&self, module: &str, port: &str) -> Vec<f32> {
        self.signals
            .iter()
            .filter(|event| event.module == module && event.port == port)
            .map(|event| event.value)
            .collect()
    }

    /// Get all gate events for a specific module and gate
    #[must_use]
    pub fn get_gate_events(&self, module: &str, gate: &str) -> Vec<&GateEvent> {
        self.gates
            .iter()
            .filter(|event| event.module == module && event.gate == gate)
            .collect()
    }

    /// Check if a gate fired during observation
    #[must_use]
    pub fn gate_fired(&self, module: &str, gate: &str) -> bool {
        self.gates
            .iter()
            .any(|event| event.module == module && event.gate == gate && event.triggered)
    }

    /// Get the range of values for a signal
    #[must_use]
    pub fn signal_range(&self, module: &str, port: &str) -> Option<(f32, f32)> {
        let values = self.get_signal_values(module, port);
        if values.is_empty() {
            return None;
        }

        let min = values.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let max = values.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        Some((min, max))
    }

    /// Check if a signal varied during observation
    #[must_use]
    pub fn signal_varied(&self, module: &str, port: &str) -> bool {
        let values = self.get_signal_values(module, port);
        if values.len() < 2 {
            return false;
        }

        let first = values[0];
        values.iter().any(|&v| (v - first).abs() > 0.001)
    }
}

impl Default for ObservationCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl SignalObserver for ObservationCollector {
    fn observe_signal(&mut self, event: &SignalEvent) {
        self.signals.push(event.clone());
    }

    fn observe_gate(&mut self, event: &GateEvent) {
        self.gates.push(event.clone());
    }

    fn observe_parameter(&mut self, event: &ParameterEvent) {
        self.parameters.push(event.clone());
    }

    fn begin_process_cycle(&mut self, cycle: usize) {
        self.cycle_count = cycle;
    }
}

/// A simple observer that prints events to the console
pub struct ConsoleObserver {
    pub verbose: bool,
}

impl ConsoleObserver {
    #[must_use]
    pub const fn new(verbose: bool) -> Self {
        Self { verbose }
    }
}

impl SignalObserver for ConsoleObserver {
    fn observe_signal(&mut self, event: &SignalEvent) {
        if self.verbose {
            println!("[SIGNAL] {}.{} = {:.3}", event.module, event.port, event.value);
        }
    }

    fn observe_gate(&mut self, event: &GateEvent) {
        if event.triggered {
            println!("[GATE] {}.{} TRIGGERED", event.module, event.gate);
        }
    }

    fn observe_parameter(&mut self, event: &ParameterEvent) {
        println!("[PARAM] {}.{} = {:.3}", event.module, event.parameter, event.value);
    }

    fn begin_process_cycle(&mut self, cycle: usize) {
        if self.verbose {
            println!("[CYCLE] Begin cycle {cycle}");
        }
    }
}

/// Manages multiple observers
pub struct ObserverManager {
    observers: Vec<Box<dyn SignalObserver>>,
}

impl ObserverManager {
    #[must_use]
    pub fn new() -> Self {
        Self { observers: Vec::new() }
    }

    pub fn add_observer(&mut self, observer: Box<dyn SignalObserver>) {
        self.observers.push(observer);
    }

    pub fn observe_signal(&mut self, module: &str, port: &str, sample_index: usize, value: f32) {
        let event = SignalEvent {
            module: module.to_string(),
            port: port.to_string(),
            sample_index,
            value,
        };

        for observer in &mut self.observers {
            observer.observe_signal(&event);
        }
    }

    pub fn observe_gate(&mut self, module: &str, gate: &str, sample_index: usize, triggered: bool) {
        let event = GateEvent {
            module: module.to_string(),
            gate: gate.to_string(),
            sample_index,
            triggered,
        };

        for observer in &mut self.observers {
            observer.observe_gate(&event);
        }
    }

    pub fn observe_parameter(&mut self, module: &str, parameter: &str, value: f32) {
        let event = ParameterEvent {
            module: module.to_string(),
            parameter: parameter.to_string(),
            value,
        };

        for observer in &mut self.observers {
            observer.observe_parameter(&event);
        }
    }

    pub fn begin_process_cycle(&mut self, cycle: usize) {
        for observer in &mut self.observers {
            observer.begin_process_cycle(cycle);
        }
    }

    pub fn end_process_cycle(&mut self, cycle: usize) {
        for observer in &mut self.observers {
            observer.end_process_cycle(cycle);
        }
    }
}

impl Default for ObserverManager {
    fn default() -> Self {
        Self::new()
    }
}
