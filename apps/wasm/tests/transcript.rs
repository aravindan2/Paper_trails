//! Golden-transcript parity for the WASM adapter.
//!
//! The fixture pair (`tests/fixtures/commands.txt` + `expected.txt`) is the
//! single source of truth shared by two runners:
//!
//! - this test, driving the adapter natively;
//! - `smoke/smoke.js`, driving the compiled WASM module in Node.
//!
//! If both produce the same transcript, the native and WASM builds of the
//! adapter behave identically for the representative sequence. The native
//! side additionally checks the adapter against a directly-driven
//! `GameEngine` in `session.rs`, so the chain is:
//! `GameEngine (native) == PaperTrails (native) == expected.txt == PaperTrails (wasm)`.

use paper_trails_wasm::PaperTrails;

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("cannot read {path}: {err}"))
}

/// Commands from `commands.txt`: blank lines and `#` comments skipped.
fn commands() -> Vec<String> {
    fixture("commands.txt")
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// Parse the golden transcript into `(command, output)` pairs.
///
/// Format: `>>> <command>` then the output lines (possibly none) then a
/// `=== end ===` marker line. Neither marker text can appear in command
/// output, so the format is unambiguous.
fn golden() -> Vec<(String, String)> {
    let text = fixture("expected.txt");
    let mut pairs = Vec::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let command = line
            .strip_prefix(">>> ")
            .unwrap_or_else(|| panic!("expected a '>>> ' command line, found: {line:?}"));
        let mut output = Vec::new();
        loop {
            let next = lines.next().unwrap_or_else(|| {
                panic!("transcript ended without '=== end ===' for {command:?}")
            });
            if next == "=== end ===" {
                break;
            }
            output.push(next);
        }
        pairs.push((command.to_string(), output.join("\n")));
    }
    pairs
}

#[test]
fn native_adapter_reproduces_the_golden_transcript() {
    let commands = commands();
    let golden = golden();

    // The fixture's command lines and the transcript's must agree, in order.
    let golden_commands: Vec<&str> = golden.iter().map(|(cmd, _)| cmd.as_str()).collect();
    assert_eq!(
        golden_commands, commands,
        "expected.txt is stale vs commands.txt"
    );

    let mut game = PaperTrails::new();
    for (command, expected_output) in &golden {
        let actual = game.execute(command);
        assert_eq!(
            actual, *expected_output,
            "native output diverged from golden transcript for command: {command}"
        );
    }
}

#[test]
fn structured_state_after_the_sequence_matches_the_transcript() {
    // The transcript embeds discovery notices in command output; the
    // structured getters must agree with that history.
    let mut game = PaperTrails::new();
    for command in commands() {
        game.execute(&command);
    }

    assert_eq!(game.cwd(), "/");
    assert_eq!(
        game.discovered_evidence(),
        [
            "EV-002", "EV-001", "EV-004", "EV-003", "EV-006", "EV-005", "EV-007",
        ],
        "discovery order must follow the command sequence"
    );
    assert_eq!(
        game.completed_objectives(),
        ["OBJ-001", "OBJ-002", "OBJ-003"],
        "three objectives complete after the sequence (OBJ-004 needs the decoded note)"
    );
}

#[test]
fn fixture_contains_no_solution_submission() {
    // The shared fixture must stay a mechanics-only sequence: the accepted
    // report path is covered by game-core's answer_key tests, not by the
    // browser-bound adapter fixtures.
    let transcript = fixture("expected.txt");
    assert!(
        !transcript.contains("verdict: ACCEPTED"),
        "shared fixtures must not embed the accepted report"
    );
}
