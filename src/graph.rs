//! Graph execution engine for audio-rate modular synthesis
//!
//! This module implements a proper graph executor that:
//! - Supports named inputs/outputs per module
//! - Allows multiple connections to the same input (mixing)
//! - Processes everything at audio rate (Serge philosophy)
//! - Can wrap fundsp nodes or use custom processing

#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::module_name_repetitions)]
#![allow(dead_code)] // Many parts are not yet used but will be

use crate::observability::{ObserverManager, SignalObserver};
use anyhow::{anyhow, Result};
use std::collections::HashMap;

/// Describes a module input or output port
#[derive(Debug, Clone)]
pub struct PortDescriptor {
    pub name: String,
    pub default_value: f32,
    pub description: String,
}

/// Buffer of audio samples for a single port
pub type PortBuffer = Vec<f32>;

/// Collection of buffers for all ports of a module
pub struct PortBuffers {
    buffers: HashMap<String, PortBuffer>,
}

impl PortBuffers {
    #[must_use]
    pub fn new() -> Self {
        Self { buffers: HashMap::new() }
    }

    #[must_use]
    pub fn get(&self, port: &str) -> Option<&PortBuffer> {
        self.buffers.get(port)
    }

    pub fn get_mut(&mut self, port: &str) -> Option<&mut PortBuffer> {
        self.buffers.get_mut(port)
    }

    pub fn get_or_default(&mut self, port: &str, size: usize, default: f32) -> &mut PortBuffer {
        self.buffers.entry(port.to_string()).or_insert_with(|| vec![default; size])
    }

    /// Get multiple mutable references at once
    pub fn get_many_mut<const N: usize>(
        &mut self,
        ports: [&str; N],
    ) -> [Option<&mut PortBuffer>; N] {
        let mut results = [(); N].map(|()| None);
        let mut used_indices = Vec::new();

        for (i, port) in ports.iter().enumerate() {
            if let Some(index) = self.buffers.keys().position(|k| k == port) {
                if !used_indices.contains(&index) {
                    used_indices.push(index);
                    results[i] = Some(unsafe {
                        // This is safe because we ensure each key is accessed only once
                        &mut *(self.buffers.get_mut(*port).unwrap() as *mut _)
                    });
                }
            }
        }

        results
    }
}

impl Default for PortBuffers {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for audio modules with named ports
pub trait GraphModule: Send {
    /// Get descriptors for all input ports
    fn inputs(&self) -> Vec<PortDescriptor>;

    /// Get descriptors for all output ports
    fn outputs(&self) -> Vec<PortDescriptor>;

    /// Process audio buffers
    fn process(&mut self, inputs: &PortBuffers, outputs: &mut PortBuffers, sample_count: usize);

    /// Set a parameter by name
    fn set_param(&mut self, name: &str, value: f32) -> Result<()>;

    /// Get current parameter value
    fn get_param(&self, name: &str) -> Option<f32>;

    /// Allow downcasting to concrete types
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// Represents a connection expression with potential scaling/offset
#[derive(Debug, Clone)]
pub enum ConnectionExpr {
    /// Direct connection from a module output
    Direct { module: String, port: String },
    /// Scaled connection (e.g., lfo * 1000)
    Scaled { expr: Box<ConnectionExpr>, factor: f32 },
    /// Offset connection (e.g., lfo + 200)
    Offset { expr: Box<ConnectionExpr>, offset: f32 },
    /// Sum of multiple connections
    Sum { exprs: Vec<ConnectionExpr> },
}

impl ConnectionExpr {
    /// Evaluate this expression given output buffers from all modules
    pub fn evaluate(&self, outputs: &HashMap<String, PortBuffers>, buffer: &mut PortBuffer) {
        match self {
            Self::Direct { module, port } => {
                if let Some(module_outputs) = outputs.get(module) {
                    if let Some(source) = module_outputs.get(port) {
                        buffer.copy_from_slice(source);
                    }
                }
            }
            Self::Scaled { expr, factor } => {
                expr.evaluate(outputs, buffer);
                for sample in buffer.iter_mut() {
                    *sample *= factor;
                }
            }
            Self::Offset { expr, offset } => {
                expr.evaluate(outputs, buffer);
                for sample in buffer.iter_mut() {
                    *sample += offset;
                }
            }
            Self::Sum { exprs } => {
                // Clear buffer first
                for sample in buffer.iter_mut() {
                    *sample = 0.0;
                }

                let mut temp_buffer = vec![0.0; buffer.len()];
                for expr in exprs {
                    expr.evaluate(outputs, &mut temp_buffer);
                    for (i, sample) in temp_buffer.iter().enumerate() {
                        buffer[i] += sample;
                    }
                }
            }
        }
    }
}

/// Represents a connection to a module input
#[derive(Debug, Clone)]
pub struct Connection {
    pub to_module: String,
    pub to_port: String,
    pub expression: ConnectionExpr,
}

/// The main graph executor
pub struct GraphExecutor {
    modules: HashMap<String, Box<dyn GraphModule>>,
    connections: Vec<Connection>,
    output_buffers: HashMap<String, PortBuffers>,
    input_buffers: HashMap<String, PortBuffers>,
    execution_order: Vec<String>,
    observers: ObserverManager,
    current_cycle: usize,
    // Gate state tracking for edge detection
    gate_states: HashMap<String, f32>, // module.port -> previous value
}

impl GraphExecutor {
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
            connections: Vec::new(),
            output_buffers: HashMap::new(),
            input_buffers: HashMap::new(),
            execution_order: Vec::new(),
            observers: ObserverManager::new(),
            current_cycle: 0,
            gate_states: HashMap::new(),
        }
    }

    pub fn add_module(&mut self, name: String, module: Box<dyn GraphModule>) {
        self.modules.insert(name, module);
        self.update_execution_order();
    }

    pub fn add_connection(&mut self, connection: Connection) {
        self.connections.push(connection);
    }

    /// Add an observer to monitor the graph execution
    pub fn add_observer(&mut self, observer: Box<dyn SignalObserver>) {
        self.observers.add_observer(observer);
    }

    /// Get mutable access to the observer manager
    pub fn observer_manager_mut(&mut self) -> &mut ObserverManager {
        &mut self.observers
    }

    pub fn process(&mut self, sample_count: usize) {
        // Notify observers of cycle start
        self.observers.begin_process_cycle(self.current_cycle);

        // Initialize buffers
        self.prepare_buffers(sample_count);

        // Process each module in order
        for module_name in &self.execution_order {
            if let Some(module) = self.modules.get_mut(module_name) {
                // Prepare input buffers for this module
                let module_inputs = self.input_buffers.get_mut(module_name).unwrap();

                // Evaluate all connections to this module
                for conn in &self.connections {
                    if conn.to_module == *module_name {
                        let buffer = module_inputs.get_or_default(&conn.to_port, sample_count, 0.0);
                        conn.expression.evaluate(&self.output_buffers, buffer);
                    }
                }

                // Process the module
                let module_outputs = self.output_buffers.get_mut(module_name).unwrap();
                module.process(module_inputs, module_outputs, sample_count);

                // Observe output signals (sample some values, not all for performance)
                // We need to collect the observation data first to avoid borrowing issues
                let mut signal_observations = Vec::new();
                let mut gate_observations = Vec::new();
                let sample_step = if sample_count > 128 { 64 } else { 1 };

                for (port_name, buffer) in &module_outputs.buffers {
                    // Check if this is a gate output (commonly named eor, eoc, gate, trigger, etc.)
                    let is_gate_output = port_name.contains("eor")
                        || port_name.contains("eoc")
                        || port_name.contains("gate")
                        || port_name.contains("trigger")
                        || port_name.contains("clock");

                    if is_gate_output {
                        // For gate outputs, observe as both signals and gates
                        for (i, &value) in buffer.iter().enumerate() {
                            let sample_index = self.current_cycle * sample_count + i;

                            // Observe as signal (sampled for performance)
                            if i % sample_step == 0 {
                                signal_observations.push((port_name.clone(), sample_index, value));
                            }

                            // Observe as gate event - detect LOW→HIGH transitions (edge detection)
                            let gate_key = format!("{module_name}.{port_name}");
                            let prev_value =
                                self.gate_states.get(&gate_key).copied().unwrap_or(0.0);
                            let current_high = value > 0.5;
                            let prev_high = prev_value > 0.5;

                            // Only trigger on LOW→HIGH transitions (rising edge)
                            if current_high && !prev_high {
                                gate_observations.push((port_name.clone(), sample_index, true));
                            }

                            // Update gate state for next sample
                            self.gate_states.insert(gate_key, value);
                        }
                    } else {
                        // For regular signals, sample for performance
                        for (i, &value) in buffer.iter().enumerate().step_by(sample_step) {
                            let sample_index = self.current_cycle * sample_count + i;
                            signal_observations.push((port_name.clone(), sample_index, value));
                        }
                    }
                }

                // Now observe the collected data
                for (port_name, sample_index, value) in signal_observations {
                    self.observers.observe_signal(module_name, &port_name, sample_index, value);
                }

                for (port_name, sample_index, triggered) in gate_observations {
                    self.observers.observe_gate(module_name, &port_name, sample_index, triggered);
                }
            }
        }

        // Notify observers of cycle end
        self.observers.end_process_cycle(self.current_cycle);
        self.current_cycle += 1;
    }

    fn prepare_buffers(&mut self, sample_count: usize) {
        // Initialize output buffers for all modules
        for (name, module) in &self.modules {
            let outputs = self.output_buffers.entry(name.clone()).or_default();
            for port in module.outputs() {
                outputs.get_or_default(&port.name, sample_count, 0.0);
            }

            let inputs = self.input_buffers.entry(name.clone()).or_default();
            for port in module.inputs() {
                inputs.get_or_default(&port.name, sample_count, port.default_value);
            }
        }
    }

    fn update_execution_order(&mut self) {
        // Simple topological sort
        // For now, just process in the order modules were added
        // TODO: Implement proper topological sort
        self.execution_order = self.modules.keys().cloned().collect();
    }

    pub fn get_output(&self, module: &str, port: &str) -> Option<&PortBuffer> {
        self.output_buffers.get(module)?.get(port)
    }

    pub fn get_module_mut(&mut self, name: &str) -> Option<&mut Box<dyn GraphModule>> {
        self.modules.get_mut(name)
    }

    pub fn set_module_param(
        &mut self,
        module_name: &str,
        param_name: &str,
        value: f32,
    ) -> Result<()> {
        let result = self.modules.get_mut(module_name).map_or_else(
            || Err(anyhow!("Module '{module_name}' not found")),
            |module| module.set_param(param_name, value),
        );

        // Observe parameter changes
        if result.is_ok() {
            self.observers.observe_parameter(module_name, param_name, value);
        }

        result
    }

    /// Get information about a module's ports
    pub fn inspect_module(&self, name: &str) -> Option<ModuleInfo> {
        let module = self.modules.get(name)?;
        Some(ModuleInfo {
            name: name.to_string(),
            inputs: module.inputs(),
            outputs: module.outputs(),
        })
    }

    /// List all modules in the graph
    pub fn list_modules(&self) -> Vec<String> {
        self.modules.keys().cloned().collect()
    }

    /// Get all connections in the graph
    pub fn list_connections(&self) -> &[Connection] {
        &self.connections
    }

    /// Validate that all connections reference valid modules and ports
    pub fn validate_connections(&self) -> Vec<String> {
        let mut errors = Vec::new();

        for conn in &self.connections {
            // Check if destination module exists
            if let Some(module) = self.modules.get(&conn.to_module) {
                // Check if destination port exists
                let inputs = module.inputs();
                if !inputs.iter().any(|p| p.name == conn.to_port) {
                    errors.push(format!(
                        "Module '{to_module}' has no input port '{to_port}'",
                        to_module = conn.to_module,
                        to_port = conn.to_port
                    ));
                }
            } else {
                errors.push(format!("Module '{to_module}' not found", to_module = conn.to_module));
            }

            // Validate the connection expression references valid modules/ports
            self.validate_expression(&conn.expression, &mut errors);
        }

        errors
    }

    fn validate_expression(&self, expr: &ConnectionExpr, errors: &mut Vec<String>) {
        match expr {
            ConnectionExpr::Direct { module, port } => {
                if let Some(src_module) = self.modules.get(module) {
                    let outputs = src_module.outputs();
                    if !outputs.iter().any(|p| p.name == *port) {
                        errors.push(format!("Module '{module}' has no output port '{port}'"));
                    }
                } else {
                    errors.push(format!("Source module '{module}' not found"));
                }
            }
            ConnectionExpr::Scaled { expr, .. } | ConnectionExpr::Offset { expr, .. } => {
                self.validate_expression(expr, errors);
            }
            ConnectionExpr::Sum { exprs } => {
                for expr in exprs {
                    self.validate_expression(expr, errors);
                }
            }
        }
    }

    /// Activate all manual gate modules
    pub fn activate_manual_gates(&mut self) -> usize {
        use crate::graph_modules::GraphManualGate;
        let mut count = 0;

        for module in self.modules.values_mut() {
            // Try to downcast to GraphManualGate
            if let Some(gate) = module.as_any_mut().downcast_mut::<GraphManualGate>() {
                gate.set_gate(true);
                count += 1;
            }
        }

        count
    }

    /// Release all manual gate modules
    pub fn release_manual_gates(&mut self) -> usize {
        use crate::graph_modules::GraphManualGate;
        let mut count = 0;

        for module in self.modules.values_mut() {
            // Try to downcast to GraphManualGate
            if let Some(gate) = module.as_any_mut().downcast_mut::<GraphManualGate>() {
                gate.set_gate(false);
                count += 1;
            }
        }

        count
    }

    /// Observe a gate event from a module
    pub fn observe_gate(
        &mut self,
        module_name: &str,
        gate_name: &str,
        sample_index: usize,
        triggered: bool,
    ) {
        self.observers.observe_gate(module_name, gate_name, sample_index, triggered);
    }
}

impl Default for GraphExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Information about a module for introspection
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub name: String,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
}
