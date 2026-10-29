//! Adapter behavior for the exported [`PaperTrails`] handle.
//!
//! The adapter must be a pass-through to the shared engine: for every
//! command the native CLI can run, the handle returns byte-identical output
//! and identical investigation state (see also `transcript.rs`, which pins
//! the same sequence against a golden file shared with the JS smoke test).

use game_core::GameEngine;
use paper_trails_wasm::PaperTrails;

/// Shared representative sequence from `tests/fixtures/commands.txt`.
fn fixture_commands() -> Vec<String> {
    let path = format!("{}/tests/fixtures/commands.txt", env!("CARGO_MANIFEST_DIR"));
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("cannot read {path}: {err}"));
    text.lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

#[test]
fn fresh_session_reports_case_header_and_clean_state() {
    let mut game = PaperTrails::new();
    assert_eq!(game.case_id(), "CASE-001");
    assert_eq!(game.case_title(), "The Last Commit");
    assert_eq!(game.cwd(), "/");
    assert_eq!(game.execute("pwd"), "/");
    assert!(game.discovered_evidence().is_empty());
    assert!(game.completed_objectives().is_empty());
    assert_eq!(
        game.execute("evidence"),
        "CASE-001 evidence: 0 of 8\n\nNo evidence recorded yet. Examine the case artifacts to record observations.\n\nObjectives:\n  [ ] Reconstruct the last movements of elias.voss\n  [ ] Identify the configuration change\n  [ ] Account for the modified executable\n  [ ] Attribute the change to a person or account"
    );
}

#[test]
fn empty_input_returns_empty_output() {
    let mut game = PaperTrails::new();
    assert_eq!(game.execute(""), "");
    assert_eq!(game.execute("   "), "");
}

#[test]
fn adapter_output_is_identical_to_the_engine_for_the_same_sequence() {
    let mut adapter = PaperTrails::new();
    let mut engine = GameEngine::case_001();
    for command in fixture_commands() {
        let via_adapter = adapter.execute(&command);
        let via_engine = engine.execute(&command).text;
        assert_eq!(
            via_adapter, via_engine,
            "adapter diverged for command: {command}"
        );
    }
    // Same final investigation state, not just the same printed text.
    assert_eq!(adapter.cwd(), engine.cwd());
    assert_eq!(
        adapter.discovered_evidence(),
        engine.state().discovered_evidence
    );
    assert_eq!(
        adapter.completed_objectives(),
        engine.state().completed_objectives
    );
}

#[test]
fn discoveries_surface_once_with_notices_and_state() {
    let mut game = PaperTrails::new();

    let auth = game.execute(r#"grep -n "Accepted password for elias" /var/log/auth.log"#);
    assert!(
        auth.contains(
            "evidence recorded: EV-002 — Password login for elias from an external address"
        ),
        "unexpected auth discovery output: {auth}"
    );
    assert_eq!(game.discovered_evidence(), ["EV-002"]);

    let badge = game.execute(r#"grep -n "OUT elias.voss" /var/log/badge.log"#);
    assert!(
        badge.contains("evidence recorded: EV-001 — Badge exit after hours"),
        "unexpected badge discovery output: {badge}"
    );
    assert!(
        badge
            .contains("objective complete: OBJ-001 — Reconstruct the last movements of elias.voss"),
        "missing objective notice: {badge}"
    );
    assert_eq!(game.discovered_evidence(), ["EV-002", "EV-001"]);
    assert_eq!(game.completed_objectives(), ["OBJ-001"]);

    // Re-running the same discoveries must not duplicate state or notices.
    let auth_again = game.execute(r#"grep -n "Accepted password for elias" /var/log/auth.log"#);
    assert!(
        !auth_again.contains("evidence recorded:"),
        "duplicate notice: {auth_again}"
    );
    let badge_again = game.execute(r#"grep -n "OUT elias.voss" /var/log/badge.log"#);
    assert!(
        !badge_again.contains("evidence recorded:"),
        "duplicate notice: {badge_again}"
    );
    assert!(
        !badge_again.contains("objective complete:"),
        "duplicate objective notice: {badge_again}"
    );
    assert_eq!(game.discovered_evidence(), ["EV-002", "EV-001"]);
    assert_eq!(game.completed_objectives(), ["OBJ-001"]);
}

#[test]
fn journal_and_timeline_commands_reflect_recorded_state() {
    let mut game = PaperTrails::new();
    game.execute(r#"grep -n "Accepted password for elias" /var/log/auth.log"#);
    game.execute(r#"grep -n "OUT elias.voss" /var/log/badge.log"#);

    // Journal: discovery order, full observation text, objective checkboxes.
    let journal = game.execute("evidence");
    assert!(
        journal.starts_with("CASE-001 evidence: 2 of 8"),
        "header: {journal}"
    );
    let ev002 = journal.find("EV-002").expect("EV-002 listed");
    let ev001 = journal.find("EV-001").expect("EV-001 listed");
    assert!(ev002 < ev001, "journal must follow discovery order");
    assert!(journal.contains("source: /var/log/auth.log"));
    assert!(journal.contains("[x] Reconstruct the last movements of elias.voss"));

    // Timeline: only dated discoveries, ascending, with source provenance.
    let timeline = game.execute("timeline");
    assert!(
        timeline.starts_with("CASE-001 timeline: 2 dated events"),
        "header: {timeline}"
    );
    let badge_line = timeline
        .find("Badge exit after hours")
        .expect("badge event");
    let login_line = timeline
        .find("Password login for elias")
        .expect("login event");
    // Badge exit (21:28:04) precedes the external login (23:47:12).
    assert!(
        badge_line < login_line,
        "timeline not chronological: {timeline}"
    );
    assert!(timeline.contains("(/var/log/badge.log)"));

    // Structured state agrees with the commands.
    assert_eq!(game.discovered_evidence(), ["EV-002", "EV-001"]);
}

#[test]
fn neutral_report_submission_is_incomplete_and_reset_cleans_everything() {
    let mut game = PaperTrails::new();
    game.execute(r#"grep -n "Accepted password for elias" /var/log/auth.log"#);
    game.execute("cd var/log");

    game.execute("submit who someone");
    game.execute("submit did nothing yet");
    game.execute("submit why unknown");
    let evaluation = game.execute("submit");
    assert!(
        evaluation.contains("CASE-001 report evaluation"),
        "header: {evaluation}"
    );
    assert!(
        evaluation.contains("verdict: INCOMPLETE — revise your report and gather more evidence"),
        "neutral report must not be accepted: {evaluation}"
    );
    assert!(evaluation.contains("who: does not match the evidence you have recorded"));
    assert!(
        evaluation.contains("evidence: 1 of 8 recorded (6 required — key observations missing)")
    );

    // Reset: identical to a brand-new session, everywhere.
    game.reset();
    assert_eq!(game.cwd(), "/");
    assert!(game.discovered_evidence().is_empty());
    assert!(game.completed_objectives().is_empty());
    let mut fresh = PaperTrails::new();
    for command in fixture_commands() {
        assert_eq!(
            game.execute(&command),
            fresh.execute(&command),
            "post-reset replay diverged for: {command}"
        );
    }
}

#[test]
fn commands_never_reach_the_host_filesystem() {
    let mut game = PaperTrails::new();
    // Paths that exist on a host (WSL) machine but not in the case image:
    // they must fail inside the VFS, leaving state untouched.
    for probe in [
        "cat /etc/shadow",
        "ls /mnt/c/Users",
        "cat /proc/self/maps",
        "cd /tmp",
        "sha256sum /etc/passwd",
    ] {
        let out = game.execute(probe);
        assert!(!out.is_empty(), "host path silently succeeded: {probe}");
        assert_eq!(game.cwd(), "/", "cwd changed by host path: {probe}");
    }
    assert!(game.discovered_evidence().is_empty());
    // A host-style absolute path is simply absent from the case image.
    assert!(
        game.execute("cat /etc/shadow")
            .starts_with("cat: /etc/shadow:")
    );
}

#[test]
fn errors_are_plain_text_not_failures() {
    let mut game = PaperTrails::new();
    let unknown = game.execute("unknowncmd");
    assert!(!unknown.is_empty(), "unknown command must print an error");
    let above_root = game.execute("cd ../..");
    assert!(
        !above_root.is_empty(),
        "traversal above root must print an error"
    );
    assert_eq!(game.cwd(), "/");
    let missing = game.execute("cat /nope");
    assert!(
        missing.starts_with("cat: /nope:"),
        "missing file: {missing}"
    );
    // The session stays usable after every error.
    assert_eq!(game.execute("pwd"), "/");
}
