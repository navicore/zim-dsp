//! Zim-DSP - Text-based modular synthesizer
//!
//! This is the main entry point for the zim-dsp CLI application.

#![allow(clippy::multiple_crate_versions)] // Dependencies have conflicting sub-dependencies

use anyhow::Result;
use rustyline::error::ReadlineError;
use rustyline::{Config, EditMode, Editor};

mod graph;
mod graph_engine;
mod graph_modules;
mod modules;
mod parser;

use graph_engine::GraphEngine;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(String::as_str) {
        Some("play") => {
            if let Some(patch_file) = args.get(2) {
                play_patch(patch_file)?;
            } else {
                eprintln!("Usage: zim-dsp play <patch_file>");
            }
        }
        Some("repl") => {
            run_repl()?;
        }
        Some("help" | "-h" | "--help") | None => {
            print_help();
        }
        Some(_) => {
            eprintln!("Unknown command. Use 'help' to see available commands.");
            print_help();
        }
    }

    Ok(())
}

fn play_patch(patch_file: &str) -> Result<()> {
    println!("Loading patch: {patch_file}");

    let mut engine = GraphEngine::new();
    let patch_content = std::fs::read_to_string(patch_file)?;

    engine.load_patch(&patch_content)?;
    engine.start()?;

    println!("Playing... Press Enter to stop");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    engine.stop();
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn run_repl() -> Result<()> {
    println!("Zim-DSP REPL - Type 'help' for commands, 'quit' to exit");
    println!("Vi mode enabled: ESC for normal mode, 'i' for insert mode");

    // Configure rustyline with Vi mode
    let config = Config::builder()
        .edit_mode(EditMode::Vi)
        .history_ignore_space(true)
        .max_history_size(1000)?
        .build();

    let mut rl = Editor::<(), _>::with_config(config)?;

    // Load history if it exists
    let history_path = dirs::home_dir().map(|mut path| {
        path.push(".zim_dsp_history");
        path
    });

    if let Some(ref path) = history_path {
        let _ = rl.load_history(path);
    }

    let mut engine = GraphEngine::new();

    loop {
        let readline = rl.readline("> ");

        match readline {
            Ok(line) => {
                let input = line.trim();

                if input.is_empty() {
                    continue;
                }

                // Add to history
                let _ = rl.add_history_entry(&line);

                match input {
                    "quit" | "exit" => break,
                    "help" => print_repl_help(),
                    "start" => {
                        engine.start()?;
                        println!("Audio started");
                    }
                    "stop" => {
                        engine.stop();
                        println!("Audio stopped");
                    }
                    "gate" | "g" => {
                        // Activate manual gate modules
                        if engine.activate_manual_gates() > 0 {
                            println!("Manual gates activated");
                        } else {
                            println!("No manual gate modules found");
                        }
                    }
                    "release" | "r" => {
                        // Release manual gate modules
                        if engine.release_manual_gates() > 0 {
                            println!("Manual gates released");
                        } else {
                            println!("No manual gate modules found");
                        }
                    }
                    "clear" => {
                        engine.clear_patch();
                        println!("Patch cleared");
                    }
                    "list" => {
                        let modules = engine.list_modules();
                        if modules.is_empty() {
                            println!("No modules loaded");
                        } else {
                            println!("Modules:");
                            for module in modules {
                                println!("  - {module}");
                            }
                        }
                    }
                    "validate" => {
                        let errors = engine.validate_connections();
                        if errors.is_empty() {
                            println!("✓ All connections are valid");
                        } else {
                            println!("Connection errors:");
                            for error in errors {
                                println!("  - {error}");
                            }
                        }
                    }
                    _ => {
                        // Check for inspect command
                        if let Some(module_name) = input.strip_prefix("inspect ") {
                            let module_name = module_name.trim();
                            if let Some(info) = engine.inspect_module(module_name) {
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
                            } else {
                                eprintln!("Module '{module_name}' not found");
                            }
                        } else {
                            // Try to parse as patch command
                            match engine.process_line(input) {
                                Ok(msg) => println!("{msg}"),
                                Err(e) => eprintln!("Error: {e}"),
                            }
                        }
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            }
            Err(err) => {
                println!("Error: {err:?}");
                break;
            }
        }
    }

    // Save history
    if let Some(ref path) = history_path {
        let _ = rl.save_history(path);
    }

    Ok(())
}

fn print_help() {
    println!(
        "Zim-DSP - Text-based modular synthesizer
    
Usage:
    zim-dsp play <patch_file>    Play a patch file
    zim-dsp repl                 Start interactive REPL
    zim-dsp help                 Show this help

Examples:
    zim-dsp play examples/basic_patch.zim
    zim-dsp repl
"
    );
}

fn print_repl_help() {
    println!(
        "REPL Commands:
    help      - Show this help
    start     - Start audio processing
    stop      - Stop audio processing
    gate/g    - Turn on manual gates
    release/r - Turn off manual gates
    clear     - Clear current patch
    list      - List all modules
    inspect   - Inspect a module's ports
    validate  - Validate all connections
    quit      - Exit REPL
    
Patch Syntax:
    vco: osc sine 440           - Create oscillator
    vcf: filter moog            - Create filter
    env: envelope 0.01 0.1      - Create envelope
    vca: vca 1.0                - Create VCA
    clock: lfo 0.5              - Create LFO (0.5 Hz)
    gate: manual                - Create manual gate
    noise: noise                - Create noise generator
    mix: mixer                  - Create 4-input mono mixer
    mix: mixer 3                - Create 3-input mono mixer
    slew: slew_gen 0.1 0.1      - Create slew generator (rise/fall times)
    slew: slew_gen 0.2          - Create slew generator (same rise/fall)
    
Connections:
    vcf.audio <- vco.sine       - Simple connection
    vca.cv <- env.output        - Control voltage
    env.gate <- clock.gate      - Clock triggers envelope
    vcf.cutoff <- lfo.sine * 2000 + 1000  - Scaled/offset
    out <- vca.out              - Mono to stereo output
    out.left <- vca1.out        - Left channel only
    out.right <- vca2.out       - Right channel only"
    );
}
