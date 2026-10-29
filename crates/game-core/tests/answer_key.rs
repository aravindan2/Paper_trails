//! Answer-key tests (development-only): the brief explicitly allows test code
//! to know the intended solution while keeping it out of public case assets
//! and the shipped client.
//!
//! Covered here:
//! - a full clean-state progression to all eight evidence items and four
//!   objectives through the intended discovery paths,
//! - accepted / incomplete / unsupported report evaluations,
//! - consistency between the authored artifacts and the recorded timeline,
//! - separation: the private design doc and solution text never appear in
//!   case assets, the README, or the shipped source tree.

use std::path::{Path, PathBuf};

use game_core::case::{CASE_001, EVIDENCE, HIDDEN_PLAINTEXT, case_001_fs};
use game_core::{GameEngine, ReportField};

fn run(engine: &mut GameEngine, input: &str) -> String {
    engine.execute(input).text
}

/// Every file in the Case 001 image. Pinned so an accidental extra asset
/// cannot smuggle solution text into the virtual filesystem (or a future
/// browser bundle).
const CASE_INVENTORY: &[&str] = &[
    "/etc/hostname",
    "/etc/timezone",
    "/home/elias/.fragment.b64",
    "/home/elias/atlas-repo/.git/HEAD",
    "/home/elias/atlas-repo/.git/logs/HEAD",
    "/home/elias/atlas-repo/.git/refs/heads/main",
    "/home/elias/atlas-repo/config/export.conf",
    "/home/elias/atlas-repo/src/pipeline.rs",
    "/home/elias/note.txt",
    "/home/elias/worklog.txt",
    "/opt/atlas/README.txt",
    "/opt/atlas/engine.bin",
    "/var/log/auth.log",
    "/var/log/badge.log",
    "/var/log/syslog",
];

/// Run the intended discovery path from a clean state and return the engine.
fn progress_to_all_evidence() -> GameEngine {
    let mut engine = GameEngine::case_001();
    assert!(engine.state().discovered_evidence.is_empty());
    assert!(engine.state().completed_objectives.is_empty());

    // 1/8: badge exit.
    let out = run(&mut engine, "grep -n \"OUT elias.voss\" /var/log/badge.log");
    assert!(out.ends_with("evidence recorded: EV-001 — Badge exit after hours"));

    // 2/8: external password login — also completes OBJ-001.
    let out = run(
        &mut engine,
        "grep -n \"Accepted password for elias\" /var/log/auth.log",
    );
    assert!(
        out.ends_with("objective complete: OBJ-001 — Reconstruct the last movements of elias.voss")
    );

    // 3/8: the export commit in the reflog.
    let out = run(
        &mut engine,
        "grep \"enable nightly export\" /home/elias/atlas-repo/.git/logs/HEAD",
    );
    assert!(out.ends_with("evidence recorded: EV-003 — Export enabled by commit 7c4e0b9"));

    // 4/8: the reload in syslog.
    let out = run(
        &mut engine,
        "grep \"export pipeline enabled\" /var/log/syslog",
    );
    assert!(out.ends_with("evidence recorded: EV-004 — Host reloaded with export enabled"));

    // 5/8: strings of the installed executable.
    let out = run(&mut engine, "strings /opt/atlas/engine.bin");
    assert!(
        out.ends_with("evidence recorded: EV-005 — Executable contains enabled-export strings")
    );

    // 6/8: the release documentation — also completes OBJ-002.
    let out = run(&mut engine, "cat /opt/atlas/README.txt");
    assert!(out.ends_with("objective complete: OBJ-002 — Identify the configuration change"));

    // 7/8: executable metadata — also completes OBJ-003.
    let out = run(&mut engine, "stat /opt/atlas/engine.bin");
    assert!(out.ends_with("objective complete: OBJ-003 — Account for the modified executable"));

    // 8/8: read the concealed fragment, then decode what was read — the
    // final step completes OBJ-004.
    let blob = run(&mut engine, "cat /home/elias/.fragment.b64");
    assert!(
        !blob.contains("evidence recorded:"),
        "the asset is base64 only"
    );
    let decoded = run(&mut engine, &format!("decode base64 {blob}"));
    assert!(decoded.starts_with(HIDDEN_PLAINTEXT));
    assert!(
        decoded
            .ends_with("objective complete: OBJ-004 — Attribute the change to a person or account")
    );

    assert_eq!(
        engine.state().discovered_evidence,
        [
            "EV-001", "EV-002", "EV-003", "EV-004", "EV-005", "EV-006", "EV-007", "EV-008"
        ]
    );
    assert_eq!(
        engine.state().completed_objectives,
        ["OBJ-001", "OBJ-002", "OBJ-003", "OBJ-004"]
    );
    engine
}

#[test]
fn full_clean_state_progression_records_every_item_and_objective() {
    let mut engine = progress_to_all_evidence();

    let journal = run(&mut engine, "evidence");
    assert!(journal.starts_with("CASE-001 evidence: 8 of 8"));
    for def in EVIDENCE {
        assert!(
            journal.contains(&format!("\n\n{}  {}", def.id, def.title)),
            "journal is missing entry {}",
            def.id
        );
        assert!(journal.contains(&format!("\n        source: {}", def.source)));
    }
    // All four objectives checked.
    assert_eq!(journal.matches("[x]").count(), 4);
    assert!(!journal.contains("[ ]"));

    // The full timeline is chronological across seven dated items (EV-005
    // carries no timestamp and is excluded).
    assert_eq!(
        run(&mut engine, "timeline"),
        "CASE-001 timeline: 7 dated events\n\
         2024-10-26 09:14:02 UTC  Release documentation declares export disabled  (/opt/atlas/README.txt)\n\
         2024-10-27 18:42:11 UTC  Concealed note decoded  (/home/elias/.fragment.b64)\n\
         2024-10-27 21:28:04 UTC  Badge exit after hours  (/var/log/badge.log)\n\
         2024-10-27 23:47:12 UTC  Password login for elias from an external address  (/var/log/auth.log)\n\
         2024-10-27 23:51:40 UTC  Export enabled by commit 7c4e0b9  (/home/elias/atlas-repo/.git/logs/HEAD)\n\
         2024-10-27 23:53:31 UTC  Executable modified after the documented release  (/opt/atlas/engine.bin)\n\
         2024-10-27 23:54:09 UTC  Host reloaded with export enabled  (/var/log/syslog)"
    );
}

#[test]
fn complete_report_with_full_evidence_is_accepted() {
    let mut engine = progress_to_all_evidence();
    run(&mut engine, "submit who dreyes");
    run(
        &mut engine,
        "submit did enabled the pinehill nightly export",
    );
    run(
        &mut engine,
        "submit why the partner pilot go-live without compliance sign-off",
    );

    assert_eq!(
        run(&mut engine, "submit"),
        "CASE-001 report evaluation\n\
         \n\
         \x20 evidence: 8 of 8 recorded (6 required — key observations covered)\n\
         \x20 who: ok\n\
         \x20 did: ok\n\
         \x20 why: ok\n\
         \n\
         \x20 verdict: ACCEPTED — case reconstruction complete"
    );
}

#[test]
fn supported_fields_without_a_key_observation_are_incomplete_and_leak_nothing() {
    let mut engine = GameEngine::case_001();
    // Seven of eight items — everything except the concealed note.
    for input in [
        "grep \"OUT elias.voss\" /var/log/badge.log",
        "grep \"Accepted password for elias\" /var/log/auth.log",
        "grep \"enable nightly export\" /home/elias/atlas-repo/.git/logs/HEAD",
        "grep \"export pipeline enabled\" /var/log/syslog",
        "strings /opt/atlas/engine.bin",
        "cat /opt/atlas/README.txt",
        "stat /opt/atlas/engine.bin",
    ] {
        run(&mut engine, input);
    }
    assert_eq!(engine.state().discovered_evidence.len(), 7);

    run(&mut engine, "submit who dreyes");
    run(&mut engine, "submit did enabled the pinehill export");
    run(&mut engine, "submit why the partner pilot go-live");
    let out = run(&mut engine, "submit");

    assert_eq!(
        out,
        "CASE-001 report evaluation\n\
         \n\
         \x20 evidence: 7 of 8 recorded (6 required — key observations missing)\n\
         \x20 who: ok\n\
         \x20 did: ok\n\
         \x20 why: ok\n\
         \n\
         \x20 verdict: INCOMPLETE — revise your report and gather more evidence"
    );
    // Feedback must not disclose which observation is missing or why.
    for spoiler in ["EV-008", "fragment", "note", "decode", "dreyes"] {
        assert!(!out.contains(spoiler), "evaluation leaked: {spoiler}");
    }
}

#[test]
fn attribution_unsupported_by_the_evidence_is_flagged_not_accepted() {
    let mut engine = progress_to_all_evidence();
    // The CI service account is a red herring: full evidence cannot rescue
    // an unsupported attribution.
    run(&mut engine, "submit who buildsvc");
    run(&mut engine, "submit did enabled the pinehill export");
    run(&mut engine, "submit why the partner pilot go-live");
    let out = run(&mut engine, "submit");

    assert!(out.contains("evidence: 8 of 8 recorded (6 required — key observations covered)"));
    assert!(out.contains("who: does not match the evidence you have recorded"));
    assert!(out.contains("did: ok"));
    assert!(out.contains("why: ok"));
    assert!(out.contains("verdict: INCOMPLETE — revise your report and gather more evidence"));
    assert!(!out.contains("ACCEPTED"));

    // Swapping in the supported attribution then passes.
    run(&mut engine, "submit who Dana Reyes");
    assert!(run(&mut engine, "submit").contains("verdict: ACCEPTED"));
    assert_eq!(
        engine.state().report.get(ReportField::Who),
        Some("Dana Reyes")
    );
}

#[test]
fn submitted_text_is_recorded_verbatim_until_evaluated() {
    let mut engine = progress_to_all_evidence();
    run(&mut engine, "submit who  someone   else ");
    // The parser collapses whitespace between tokens; evaluation still sees
    // the stored text (and this one is unsupported).
    let stored = engine.state().report.get(ReportField::Who);
    assert_eq!(stored, Some("someone else"));
    let out = run(&mut engine, "submit");
    assert!(out.contains("who: does not match the evidence you have recorded"));

    // Malformed input never reaches the report.
    assert_eq!(
        run(&mut engine, "submit culpritt someone"),
        "submit: unknown field: culpritt\nusage: submit who|did|why TEXT (or submit alone to evaluate)"
    );
    assert_eq!(
        engine.state().report.get(ReportField::Who),
        Some("someone else")
    );
}

#[test]
fn authored_artifacts_match_the_ground_truth_timeline() {
    // Registry timestamps against the pinned ground truth (UTC epochs).
    let pinned: &[(&str, i64)] = &[
        ("EV-006", 1_729_934_042), // 2024-10-26 09:14:02 docs
        ("EV-008", 1_730_054_531), // 2024-10-27 18:42:11 concealed note
        ("EV-001", 1_730_064_484), // 2024-10-27 21:28:04 badge exit
        ("EV-002", 1_730_072_832), // 2024-10-27 23:47:12 password login
        ("EV-003", 1_730_073_100), // 2024-10-27 23:51:40 commit
        ("EV-007", 1_730_073_211), // 2024-10-27 23:53:31 executable swap
        ("EV-004", 1_730_073_249), // 2024-10-27 23:54:09 export enabled
    ];
    for (id, epoch) in pinned {
        let def = EVIDENCE.iter().find(|e| e.id == *id).expect("registered");
        assert_eq!(def.time, Some(*epoch), "{id} drifted from ground truth");
    }
    // The `pinned` list is written in chronological order.
    for pair in pinned.windows(2) {
        assert!(
            pair[0].1 < pair[1].1,
            "timeline order broken at {}",
            pair[0].0
        );
    }
    assert!(
        EVIDENCE
            .iter()
            .find(|e| e.id == "EV-005")
            .expect("registered")
            .time
            .is_none()
    );

    // The same facts must be visible inside the artifacts themselves.
    let fs = case_001_fs();
    let text = |path: &str| -> String {
        String::from_utf8(
            fs.read(path, "/")
                .expect("inventory file")
                .contents
                .to_vec(),
        )
        .expect("text asset")
    };

    let badge = text("/var/log/badge.log");
    assert!(badge.contains("Oct 27 21:28:04 badge[311]: OUT elias.voss"));
    assert!(badge.contains("Oct 27 21:28:04") && !badge.contains("Oct 27 21:28:05"));

    let auth = text("/var/log/auth.log");
    assert!(auth.contains(
        "Oct 27 23:47:12 atlas sshd[991]: Accepted password for elias from 203.0.113.77"
    ));
    // The external session is bracketed by open/close lines the same night.
    assert!(auth.contains("Oct 27 21:15:33") && auth.contains("Oct 27 23:56:40"));
    // The password-reset mechanism that made the login possible.
    assert!(auth.contains("sudo: dreyes") && auth.contains("COMMAND=/usr/bin/passwd elias"));

    let syslog = text("/var/log/syslog");
    assert!(syslog.contains("Oct 27 23:54:09 atlas kernel: atlasd[1201]: export pipeline enabled"));

    let reflog = text("/home/elias/atlas-repo/.git/logs/HEAD");
    // The commit entry carries the registry's epoch verbatim (git reflog
    // format: `... <epoch> +0000<TAB>commit: <message>`).
    assert!(reflog.contains("1730073100 +0000\tcommit: config: enable nightly export"));
    // ...and the docs entry carries EV-006's epoch, matching README.txt.
    let readme_epoch = EVIDENCE
        .iter()
        .find(|e| e.id == "EV-006")
        .and_then(|e| e.time)
        .expect("EV-006 has a time");
    assert!(reflog.contains(&format!(" {readme_epoch} +0000")));

    let readme = text("/opt/atlas/README.txt");
    assert!(readme.contains("Export pipeline: disabled"));
    let conf = text("/home/elias/atlas-repo/config/export.conf");
    assert!(conf.contains("export.enabled = true"));

    // Filesystem metadata agrees with the narrative: the swap file was
    // modified after the documentation, exactly when syslog says it was
    // installed, and the config edit precedes the commit.
    let readme_mtime = fs.stat("/opt/atlas/README.txt", "/").unwrap().modified_at;
    let bin_mtime = fs.stat("/opt/atlas/engine.bin", "/").unwrap().modified_at;
    let conf_mtime = fs
        .stat("/home/elias/atlas-repo/config/export.conf", "/")
        .unwrap()
        .modified_at;
    assert_eq!(readme_mtime, readme_epoch);
    assert_eq!(bin_mtime, 1_730_073_211);
    assert_eq!(
        game_core::format_timestamp(bin_mtime),
        "2024-10-27 23:53:31 UTC"
    );
    assert!(syslog.contains(
        "Oct 27 23:53:31 atlas systemd[1]: atlas-build[1205]: installed /opt/atlas/engine.bin"
    ));
    assert!(readme_mtime < conf_mtime && conf_mtime < bin_mtime);

    // Two independent artifacts keep the attribution reachable (worklog
    // narrative and the password-reset line above).
    let worklog = text("/home/elias/worklog.txt");
    assert!(worklog.contains("dreyes") && worklog.contains("pinehill"));
}

#[test]
fn answer_key_stays_out_of_assets_docs_and_shipped_sources() {
    let fs = case_001_fs();
    let needle = HIDDEN_PLAINTEXT.as_bytes();

    // 1. No virtual file contains the decoded solution text (the fragment
    //    asset carries base64 only). The inventory is pinned, so no new
    //    asset can slip in unnoticed.
    let mut seen = Vec::new();
    for path in CASE_INVENTORY {
        let file = fs.read(path, "/").expect("pinned inventory file exists");
        assert!(
            !file.contents.windows(needle.len()).any(|w| w == needle),
            "hidden plaintext leaked into {path}"
        );
        seen.push(*path);
    }
    assert_eq!(seen.len(), CASE_INVENTORY.len());
    // The engine.bin content is non-UTF-8, so also make sure the plaintext
    // is not hiding as raw bytes split across the whole FS (already checked
    // per file above).

    // 2. The private design document stays untracked.
    let root = repo_root();
    let gitignore = std::fs::read_to_string(root.join(".gitignore")).expect(".gitignore exists");
    assert!(
        gitignore.lines().any(|line| line.trim() == "authoring/"),
        ".gitignore must exclude authoring/"
    );

    // 3. When the private doc exists (a dev machine), it lives outside the
    //    crates/apps trees that a future browser build would compile.
    let doc = root.join("authoring/case-001-solution.md");
    if doc.exists() {
        let doc = doc.canonicalize().expect("canonical doc");
        let crates = root
            .join("crates")
            .canonicalize()
            .expect("canonical crates");
        let apps = root.join("apps").canonicalize().expect("canonical apps");
        assert!(!doc.starts_with(&crates) && !doc.starts_with(&apps));
    }

    // 4. Shipped source (engine + CLI) does not carry the private doc's
    //    distinctive answer-key prose. (The engine must contain grading
    //    keywords and the decode trigger by design — evaluation runs in the
    //    client — but never the ground-truth narrative.)
    let markers = [
        "Ground truth (what actually happened)",
        "private case design",
        "Dana Reyes used Elias's account",
    ];
    let mut shipped = Vec::new();
    collect_rs_sources(&root.join("crates/game-core/src"), &mut shipped);
    collect_rs_sources(&root.join("apps/cli/src"), &mut shipped);
    assert!(!shipped.is_empty());
    for file in &shipped {
        let source = std::fs::read_to_string(file).expect("readable source");
        for marker in markers {
            assert!(
                !source.contains(marker),
                "answer-key prose in {}",
                file.display()
            );
        }
    }

    // 5. The public README documents commands without story spoilers.
    let readme = std::fs::read_to_string(root.join("README.md")).expect("README exists");
    let readme_lower = readme.to_ascii_lowercase();
    for spoiler in ["dreyes", "reyes", "dana", "pinehill", "elias"] {
        assert!(!readme_lower.contains(spoiler), "README spoils: {spoiler}");
    }
    // It does document the investigation commands.
    for documented in ["evidence", "timeline", "submit"] {
        assert!(readme.contains(documented), "README missing: {documented}");
    }
}

// ---------------------------------------------------------------------------
// Helpers.
// ---------------------------------------------------------------------------

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn collect_rs_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            collect_rs_sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn case_definition_is_small_and_consistent() {
    // Stable identifiers: eight items, four objectives, sensible gate.
    assert_eq!(CASE_001.id, "CASE-001");
    assert_eq!(EVIDENCE.len(), 8);
    assert_eq!(CASE_001.objectives.len(), 4);
    assert!(CASE_001.rules.min_evidence <= EVIDENCE.len());
    assert_eq!(CASE_001.rules.required_evidence.len(), 3);
}
