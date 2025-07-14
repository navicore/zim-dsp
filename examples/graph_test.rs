//! Test the new graph-based module system

use zim_dsp::graph::{Connection, ConnectionExpr, GraphExecutor};
use zim_dsp::graph_modules::{GraphEnvelope, GraphFilter, GraphOscillator, GraphVca};

fn main() {
    println!("Testing graph-based module system...");

    // Create modules
    let mut graph = GraphExecutor::new();

    // Add oscillator
    graph.add_module("vco".to_string(), Box::new(GraphOscillator::new(440.0)));

    // Add LFO for filter modulation
    graph.add_module("lfo".to_string(), Box::new(GraphOscillator::new(2.0)));

    // Add filter
    graph.add_module("vcf".to_string(), Box::new(GraphFilter::new(1000.0, 0.5)));

    // Add envelope
    graph.add_module("env".to_string(), Box::new(GraphEnvelope::new(0.01, 0.5)));

    // Add VCA
    graph.add_module("vca".to_string(), Box::new(GraphVca::new(1.0)));

    // Connect modules
    // vcf.audio <- vco.saw
    graph.add_connection(Connection {
        to_module: "vcf".to_string(),
        to_port: "audio".to_string(),
        expression: ConnectionExpr::Direct {
            module: "vco".to_string(),
            port: "saw".to_string(),
        },
    });

    // vcf.cutoff <- lfo.sine * 2000 + 3000
    graph.add_connection(Connection {
        to_module: "vcf".to_string(),
        to_port: "cutoff".to_string(),
        expression: ConnectionExpr::Offset {
            expr: Box::new(ConnectionExpr::Scaled {
                expr: Box::new(ConnectionExpr::Direct {
                    module: "lfo".to_string(),
                    port: "sine".to_string(),
                }),
                factor: 2000.0,
            }),
            offset: 3000.0,
        },
    });

    // vca.audio <- vcf.lp
    graph.add_connection(Connection {
        to_module: "vca".to_string(),
        to_port: "audio".to_string(),
        expression: ConnectionExpr::Direct {
            module: "vcf".to_string(),
            port: "lp".to_string(),
        },
    });

    // vca.cv <- env.out
    graph.add_connection(Connection {
        to_module: "vca".to_string(),
        to_port: "cv".to_string(),
        expression: ConnectionExpr::Direct {
            module: "env".to_string(),
            port: "out".to_string(),
        },
    });

    // Create a gate signal to trigger the envelope
    // We'll create a manual gate buffer
    // Gate buffer would be used here to trigger the envelope
    // let gate_buffer = [1.0; 1]; // Single sample high to trigger

    // For now, let's just process without the gate
    // (we'd need to implement a way to inject gate signals)

    // Process some samples
    let sample_count = 1024;
    graph.process(sample_count);

    // Get output
    if let Some(output) = graph.get_output("vca", "out") {
        println!("Generated {} samples", output.len());
        println!("First few samples: {:?}", &output[..10.min(output.len())]);
    }

    println!("Graph test complete!");
}
