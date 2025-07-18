//! Demonstrate module introspection capabilities

use zim_dsp::graph::{Connection, ConnectionExpr, GraphExecutor};
use zim_dsp::graph_modules::{GraphFilter, GraphOscillator, GraphVca};

fn main() {
    println!("=== Module Introspection Demo ===\n");

    // Create a graph with some modules
    let mut graph = GraphExecutor::new();

    graph.add_module("vco".to_string(), Box::new(GraphOscillator::new(440.0)));
    graph.add_module("vcf".to_string(), Box::new(GraphFilter::new(1000.0, 0.5)));
    graph.add_module("vca".to_string(), Box::new(GraphVca::new(1.0)));

    // List all modules
    println!("Available modules:");
    for module in graph.list_modules() {
        println!("  - {module}");
    }
    println!();

    // Inspect each module
    for module_name in graph.list_modules() {
        if let Some(info) = graph.inspect_module(&module_name) {
            println!("Module: {}", info.name);

            println!("  Inputs:");
            for input in &info.inputs {
                println!(
                    "    - {} (default: {}): {}",
                    input.name, input.default_value, input.description
                );
            }

            println!("  Outputs:");
            for output in &info.outputs {
                println!("    - {}: {}", output.name, output.description);
            }
            println!();
        }
    }

    // Add some connections
    graph.add_connection(Connection {
        to_module: "vcf".to_string(),
        to_port: "audio".to_string(),
        expression: ConnectionExpr::Direct {
            module: "vco".to_string(),
            port: "saw".to_string(),
        },
    });

    // Add an invalid connection for testing
    graph.add_connection(Connection {
        to_module: "vcf".to_string(),
        to_port: "invalid_port".to_string(),
        expression: ConnectionExpr::Direct {
            module: "vco".to_string(),
            port: "invalid_output".to_string(),
        },
    });

    // Validate connections
    println!("Connection validation:");
    let errors = graph.validate_connections();
    if errors.is_empty() {
        println!("  All connections are valid!");
    } else {
        println!("  Found {} errors:", errors.len());
        for error in errors {
            println!("    - {error}");
        }
    }
}

// Example REPL commands this would enable:
// > inspect vco
// Module: vco
//   Inputs:
//     - fm (default: 0.0): Frequency modulation input
//     - sync (default: 0.0): Sync/reset input
//   Outputs:
//     - sine: Sine wave output
//     - saw: Sawtooth wave output
//     - square: Square wave output
//     - triangle: Triangle wave output
//
// > list
// Available modules:
//   - vco
//   - vcf
//   - vca
//
// > validate
// Connection validation:
//   - Module 'vcf' has no input port 'invalid_port'
//   - Module 'vco' has no output port 'invalid_output'
