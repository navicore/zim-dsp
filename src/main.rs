use anyhow::Result;
use std::io::{self, BufRead, BufReader};

mod engine;
mod modules;
mod parser;

use engine::Engine;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    
    match args.get(1).map(|s| s.as_str()) {
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
        Some("help") | Some("-h") | Some("--help") => {
            print_help();
        }
        _ => {
            print_help();
        }
    }
    
    Ok(())
}

fn play_patch(patch_file: &str) -> Result<()> {
    println!("Loading patch: {}", patch_file);
    
    let mut engine = Engine::new()?;
    let patch_content = std::fs::read_to_string(patch_file)?;
    
    engine.load_patch(&patch_content)?;
    engine.start()?;
    
    println!("Playing... Press Enter to stop");
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    
    engine.stop();
    Ok(())
}

fn run_repl() -> Result<()> {
    println!("Zim-DSP REPL - Type 'help' for commands, 'quit' to exit");
    
    let mut engine = Engine::new()?;
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin);
    
    loop {
        print!("> ");
        io::Write::flush(&mut io::stdout())?;
        
        let mut input = String::new();
        reader.read_line(&mut input)?;
        let input = input.trim();
        
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
            "clear" => {
                engine.clear_patch();
                println!("Patch cleared");
            }
            _ => {
                // Try to parse as patch command
                match engine.process_line(input) {
                    Ok(msg) => println!("{}", msg),
                    Err(e) => eprintln!("Error: {}", e),
                }
            }
        }
    }
    
    Ok(())
}

fn print_help() {
    println!("Zim-DSP - Text-based modular synthesizer
    
Usage:
    zim-dsp play <patch_file>    Play a patch file
    zim-dsp repl                 Start interactive REPL
    zim-dsp help                 Show this help

Examples:
    zim-dsp play examples/basic_patch.zim
    zim-dsp repl");
}

fn print_repl_help() {
    println!("REPL Commands:
    help     - Show this help
    start    - Start audio processing
    stop     - Stop audio processing  
    clear    - Clear current patch
    quit     - Exit REPL
    
Patch Syntax:
    vco: osc saw 440            - Create oscillator
    vcf: filter moog <- vco     - Create filter with input
    out <- vcf * 0.5            - Route to output");
}