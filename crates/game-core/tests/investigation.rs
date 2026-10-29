//! Investigation mechanics tests: how evidence is discovered, deduplicated,
//! displayed (journal + timeline), how objectives complete, and how report
//! fields are recorded and graded — without asserting the case's solution
//! (that lives in `answer_key.rs`).

use game_core::{GameEngine, ReportField};

fn run(engine: &mut GameEngine, input: &str) -> String {
    engine.execute(input).text
}

const EMPTY_JOURNAL_TAIL: &str = "\n\
    \n\
    Objectives:\n\
    \x20 [ ] Reconstruct the last movements of elias.voss\n\
    \x20 [ ] Identify the configuration change\n\
    \x20 [ ] Account for the modified executable\n\
    \x20 [ ] Attribute the change to a person or account";

#[test]
fn evidence_starts_empty_and_lists_objectives_unchecked() {
    let mut engine = GameEngine::case_001();
    assert_eq!(
        run(&mut engine, "evidence"),
        "CASE-001 evidence: 0 of 8\n\
         \n\
         No evidence recorded yet. Examine the case artifacts to record observations."
            .to_string()
            + EMPTY_JOURNAL_TAIL
    );
    assert!(engine.state().discovered_evidence.is_empty());
    assert!(engine.state().completed_objectives.is_empty());
}

#[test]
fn irrelevant_actions_record_nothing() {
    let mut engine = GameEngine::case_001();
    for input in [
        "help",
        "pwd",
        "ls",
        "ls /var/log",
        "cd var",
        "grep -n zzzz /var/log/auth.log",
        "stat /etc/hostname",
        "stat /opt/atlas",
        "sha256sum /opt/atlas/engine.bin",
        "cat /etc/timezone",
        "decode hex 00",
        // The right file but a `-n` so high that the registered content is
        // filtered out: nothing surfaced, nothing recorded.
        "strings -n 40 /opt/atlas/engine.bin",
    ] {
        let output = run(&mut engine, input);
        assert!(
            !output.contains("evidence recorded:"),
            "unexpected discovery from: {input}"
        );
    }
    assert!(engine.state().discovered_evidence.is_empty());
    assert!(engine.state().completed_objectives.is_empty());
}

#[test]
fn displaying_registered_content_records_evidence_exactly_once() {
    let mut engine = GameEngine::case_001();

    // A search that surfaces the registered line records its observation.
    let first = run(
        &mut engine,
        "grep -n \"Accepted password for elias\" /var/log/auth.log",
    );
    assert!(first.starts_with("/var/log/auth.log:7:Oct 27 23:47:12"));
    assert!(first.ends_with(
        "evidence recorded: EV-002 — Password login for elias from an external address"
    ));
    assert_eq!(engine.state().discovered_evidence, vec!["EV-002"]);

    // Repeating the same inspection is silent: no duplicate journal entry.
    let second = run(
        &mut engine,
        "grep -n \"Accepted password for elias\" /var/log/auth.log",
    );
    assert!(second.starts_with("/var/log/auth.log:7:Oct 27 23:47:12"));
    assert!(!second.contains("evidence recorded:"));
    assert_eq!(engine.state().discovered_evidence, vec!["EV-002"]);

    // Reading the whole file with `cat` is a second (already-seen) path:
    // the content is displayed but the entry already exists.
    let whole = run(&mut engine, "cat /var/log/auth.log");
    assert!(!whole.contains("evidence recorded:"));
    assert_eq!(engine.state().discovered_evidence, vec!["EV-002"]);
}

#[test]
fn reading_a_file_whole_can_also_record_observations() {
    let mut engine = GameEngine::case_001();
    // `cat` surfaces every line, including a registered one.
    let out = run(&mut engine, "cat /var/log/badge.log");
    assert!(out.ends_with("evidence recorded: EV-001 — Badge exit after hours"));
    assert_eq!(engine.state().discovered_evidence, vec!["EV-001"]);

    // Metadata views record only the registered file's metadata evidence:
    // stat of some other artifact displays no registered observation.
    let out = run(&mut engine, "stat /opt/atlas/README.txt");
    assert!(!out.contains("evidence recorded:"));
    let out = run(&mut engine, "stat /opt/atlas/engine.bin");
    assert!(
        out.ends_with(
            "evidence recorded: EV-007 — Executable modified after the documented release"
        )
    );
    assert_eq!(engine.state().discovered_evidence, vec!["EV-001", "EV-007"]);
}

#[test]
fn objectives_complete_only_when_all_required_evidence_is_recorded() {
    let mut engine = GameEngine::case_001();

    // One of the two required observations: nothing completes yet.
    let out = run(&mut engine, "grep \"OUT elias.voss\" /var/log/badge.log");
    assert!(out.ends_with("evidence recorded: EV-001 — Badge exit after hours"));
    assert!(engine.state().completed_objectives.is_empty());

    // The second required observation completes the objective on the spot.
    let out = run(
        &mut engine,
        "grep \"Accepted password for elias\" /var/log/auth.log",
    );
    assert!(
        out.ends_with("objective complete: OBJ-001 — Reconstruct the last movements of elias.voss")
    );
    assert_eq!(engine.state().completed_objectives, vec!["OBJ-001"]);

    // Journal shows the completed objective as checked, the rest unchecked.
    let journal = run(&mut engine, "evidence");
    assert!(journal.contains("\n  [x] Reconstruct the last movements of elias.voss"));
    assert!(journal.contains("\n  [ ] Identify the configuration change"));
}

#[test]
fn timeline_lists_dated_evidence_chronologically_with_provenance() {
    let mut engine = GameEngine::case_001();
    assert_eq!(
        run(&mut engine, "timeline"),
        "CASE-001 timeline: 0 dated events\n\nNo dated evidence recorded yet."
    );

    // Discover three items *out of* chronological order: commit (23:51),
    // docs (Oct 26), badge exit (21:28).
    run(
        &mut engine,
        "grep \"enable nightly export\" /home/elias/atlas-repo/.git/logs/HEAD",
    );
    run(
        &mut engine,
        "grep \"Export pipeline: disabled\" /opt/atlas/README.txt",
    );
    run(&mut engine, "grep \"OUT elias.voss\" /var/log/badge.log");

    // Output is sorted ascending by time, each line carrying its source.
    assert_eq!(
        run(&mut engine, "timeline"),
        "CASE-001 timeline: 3 dated events\n\
         2024-10-26 09:14:02 UTC  Release documentation declares export disabled  (/opt/atlas/README.txt)\n\
         2024-10-27 21:28:04 UTC  Badge exit after hours  (/var/log/badge.log)\n\
         2024-10-27 23:51:40 UTC  Export enabled by commit 7c4e0b9  (/home/elias/atlas-repo/.git/logs/HEAD)"
    );
}

#[test]
fn submit_records_fields_and_grades_the_report() {
    let mut engine = GameEngine::case_001();

    // Nothing recorded yet: every field reported as missing.
    assert_eq!(
        run(&mut engine, "submit"),
        "CASE-001 report evaluation\n\
         \n\
         \x20 evidence: 0 of 8 recorded (6 required — key observations missing)\n\
         \x20 who: not recorded — use: submit who TEXT\n\
         \x20 did: not recorded — use: submit did TEXT\n\
         \x20 why: not recorded — use: submit why TEXT\n\
         \n\
         \x20 verdict: INCOMPLETE — revise your report and gather more evidence"
    );

    // Recording a field echoes it back and stores it verbatim.
    assert_eq!(
        run(&mut engine, "submit did built the thing"),
        "recorded did: built the thing"
    );
    assert_eq!(
        engine.state().report.get(ReportField::Did),
        Some("built the thing")
    );
    assert!(engine.state().report.get(ReportField::Who).is_none());

    // Re-submitting a field revises it.
    assert_eq!(
        run(&mut engine, "submit did rebuilt the thing"),
        "recorded did: rebuilt the thing"
    );
    assert_eq!(
        engine.state().report.get(ReportField::Did),
        Some("rebuilt the thing")
    );

    // Now the field is recorded but not supported by anything recorded so
    // far — and the verdict explains that without revealing an answer.
    let out = run(&mut engine, "submit");
    assert!(out.contains("did: does not match the evidence you have recorded"));
    assert!(out.contains("who: not recorded — use: submit who TEXT"));
    assert!(out.contains("evidence: 0 of 8 recorded (6 required — key observations missing)"));
    assert!(out.contains("verdict: INCOMPLETE"));
    assert!(!out.contains("ACCEPTED"));
}

#[test]
fn submit_accepts_an_empty_field_as_unrecorded() {
    let mut engine = GameEngine::case_001();
    assert_eq!(run(&mut engine, "submit who \"\""), "recorded who: ");
    let out = run(&mut engine, "submit");
    assert!(out.contains("who: not recorded — use: submit who TEXT"));
}

#[test]
fn malformed_submissions_are_rejected_without_state_changes() {
    let mut engine = GameEngine::case_001();
    assert_eq!(
        run(&mut engine, "submit who"),
        "submit: missing operand\nusage: submit who|did|why TEXT"
    );
    assert_eq!(
        run(&mut engine, "submit culprit someone"),
        "submit: unknown field: culprit\nusage: submit who|did|why TEXT (or submit alone to evaluate)"
    );
    assert_eq!(
        run(&mut engine, "evidence extra"),
        "evidence: too many arguments\nusage: evidence"
    );
    // Failed submissions never record anything.
    assert!(engine.state().report.is_empty());
    assert!(engine.state().discovered_evidence.is_empty());
}
