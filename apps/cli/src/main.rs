//! Native CLI adapter for the Paper Trails game engine.
//!
//! This is intentionally thin: read a line, hand it to the shared engine,
//! print the result. No command is ever executed on the host system.

use std::io::{self, BufRead, Write};

use game_core::GameEngine;
use game_core::case::{CASE_ID, CASE_TITLE};

fn main() -> io::Result<()> {
    let mut engine = GameEngine::case_001();
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    println!("paper-trails — native CLI (Milestone 1)");
    println!("{CASE_ID}: {CASE_TITLE} — recovered workstation image");
    println!("Type 'help' for commands. The simulated filesystem is separate from your real one.");
    println!();

    loop {
        print!("investigator:{}$ ", engine.cwd());
        stdout.flush()?;

        let mut line = String::new();
        if stdin.lock().read_line(&mut line)? == 0 {
            // EOF (Ctrl-D).
            println!();
            break;
        }

        let output = engine.execute(&line);
        if !output.text.is_empty() {
            println!("{}", output.text);
        }
    }

    Ok(())
}
