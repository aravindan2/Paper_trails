//! Regenerate the golden transcript fixture (`tests/fixtures/expected.txt`).
//!
//! Run from the repository root:
//!
//! ```text
//! cargo run -q -p paper-trails-wasm --example transcript > apps/wasm/tests/fixtures/expected.txt
//! ```
//!
//! Reads `tests/fixtures/commands.txt` (resolved against this crate's
//! manifest directory, so the working directory does not matter for input)
//! and prints the transcript on stdout. Output is deterministic: the case
//! image is static, so re-running must reproduce the file byte for byte
//! (`tests/transcript.rs` enforces that, and the JavaScript smoke test
//! diffs the same file against the WASM build).

use paper_trails_wasm::PaperTrails;

fn main() {
    let commands_path = format!("{}/tests/fixtures/commands.txt", env!("CARGO_MANIFEST_DIR"));
    let commands = std::fs::read_to_string(&commands_path)
        .unwrap_or_else(|err| panic!("cannot read {commands_path}: {err}"));

    let mut game = PaperTrails::new();
    let mut transcript = String::new();
    for line in commands.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        transcript.push_str(">>> ");
        transcript.push_str(line);
        transcript.push('\n');
        let output = game.execute(line);
        if !output.is_empty() {
            transcript.push_str(&output);
            transcript.push('\n');
        }
        transcript.push_str("=== end ===\n");
    }
    print!("{transcript}");
}
