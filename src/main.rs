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
mod observability;
mod parser;
mod test_framework;

use graph_engine::GraphEngine;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(String::as_str) {
        Some("help" | "-h" | "--help") => {
            print_help();
        }
        Some(file_path) => {
            // First argument is a file path
            play_patch(file_path)?;
        }
        None => {
            // No arguments - go to REPL
            run_repl()?;
        }
    }

    Ok(())
}

fn play_patch(patch_file: &str) -> Result<()> {
    println!("Loading patch: {patch_file}");

    let mut engine = GraphEngine::new();
    let patch_content = std::fs::read_to_string(patch_file)?;

    // Check if the patch contains a "start" command
    let has_start_command = patch_content.lines().any(|line| line.trim() == "start");

    // Filter out "start" command from patch content since it's a control command, not DSL
    let filtered_patch_content: String = patch_content
        .lines()
        .filter(|line| line.trim() != "start")
        .collect::<Vec<_>>()
        .join("\n");

    engine.load_patch(&filtered_patch_content)?;

    if has_start_command {
        // Auto-play mode for scripts with explicit "start"
        engine.start()?;
        println!("Playing... Press Enter to stop");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        engine.stop();
    } else {
        // Interactive mode for scripts without "start"
        println!("Patch loaded successfully.");
        println!();
        println!("Patch contents:");
        println!("===============");
        for line in filtered_patch_content.lines() {
            if !line.trim().is_empty() && !line.trim().starts_with('#') {
                println!("  {}", line);
            }
        }
        println!("===============");
        println!();
        println!("Commands: 'start' to begin audio, 'stop' to stop, 'quit' to exit");
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

        loop {
            let readline = rl.readline("> ");

            match readline {
                Ok(line) => {
                    let command = line.trim();

                    if command.is_empty() {
                        continue;
                    }

                    // Add to history
                    let _ = rl.add_history_entry(&line);

            match command {
                "start" => {
                    engine.start()?;
                    println!("Audio started");
                }
                "stop" => {
                    engine.stop();
                    println!("Audio stopped");
                }
                "quit" | "exit" => {
                    engine.stop();
                    println!("Goodbye!");
                    break;
                }
                "help" => {
                    println!("Available commands:");
                    println!("  start - Start audio playback");
                    println!("  stop  - Stop audio playback");
                    println!("  inspect <name> - Inspect module ports");
                    println!("  quit  - Exit program");
                }
                "" => {} // Empty input, continue
                _ if command.starts_with("inspect ") => {
                    let module_name = command.strip_prefix("inspect ").unwrap().trim();
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
                    } else if let Some(info) = GraphEngine::inspect_module_type(module_name) {
                        println!("Module Type: {module_name}");
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
                        eprintln!("Module or module type '{module_name}' not found");
                    }
                }
                _ => {
                    println!("Unknown command: '{command}'. Type 'help' for available commands.");
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
    }

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
                            } else if let Some(info) = GraphEngine::inspect_module_type(module_name)
                            {
                                println!("Module Type: {module_name}");
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
                                eprintln!("Module or module type '{module_name}' not found");
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
    zim-dsp <patch_file>    Load and play a patch file 
    zim-dsp                 Start interactive mode
    zim-dsp help            Show this help

Behavior:
    • Files with 'start' command auto-play
    • Files without 'start' enter interactive mode  
    • Interactive mode: type 'start', 'stop', 'quit'

Examples:
    zim-dsp examples/simple_test.zim     # Auto-plays
    zim-dsp examples/stereo_test.zim     # Interactive
    zim-dsp                              # REPL mode
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
    inspect <name> - Inspect module ports (e.g., 'inspect osc1' or 'inspect osc')
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
    seq: seq8                   - Create 8-step sequencer
    
Connections:
    vcf.audio <- vco.sine       - Simple connection
    vca.cv <- env.output        - Control voltage
    env.gate <- clock.gate      - Clock triggers envelope
    vco.freq <- seq.cv          - Sequencer controls pitch
    vcf.cutoff <- lfo.sine * 2000 + 1000  - Scaled/offset
    out <- vca.out              - Mono to stereo output
    out.left <- vca1.out        - Left channel only
    out.right <- vca2.out       - Right channel only"
    );
}
