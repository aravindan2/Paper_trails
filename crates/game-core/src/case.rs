//! Case 001 ("The Last Commit") — authored artifacts, evidence registry,
//! objectives, and report-grading rules.
//!
//! Everything here is **player-visible case content**:
//!
//! - [`case_001_fs`] builds the virtual filesystem (logs, source, git
//!   history, binaries, encoded fragment). No host filesystem is involved.
//! - [`CASE_001`] registers eight evidence items, four objectives, and the
//!   report rules, all discovered through ordinary commands.
//!
//! **Answer-key policy:** the *facts* an artifact must contain to be
//! internally consistent live here (they are the mystery itself), but the
//! ground-truth narrative, timeline rationale, and design rationale live in
//! the private `authoring/` directory (gitignored) and in development-only
//! `tests/answer_key.rs`. Nothing in this module or in the virtual files
//! states the intended solution as prose.

use crate::investigation::{CaseDef, EvaluationRules, EvidenceDef, ObjectiveDef, Trigger};
use crate::vfs::{VirtualFile, VirtualFs};

/// Stable identifier for the first case.
pub const CASE_ID: &str = "CASE-001";

/// Title shown to the player.
pub const CASE_TITLE: &str = "The Last Commit";

/// Decoded text of the concealed fragment. This is the *trigger* the engine
/// compares `decode` output against; the asset itself only carries the
/// base64 form.
pub const HIDDEN_PLAINTEXT: &str = "dreyes said she would flip the pinehill export tonight. if it turns on while i am offsite, it was not me.";

// ---------------------------------------------------------------------------
// Pinned timestamps (all UTC; host time zone is UTC per /etc/timezone).
// ---------------------------------------------------------------------------
const T_ETC: i64 = 1_727_784_000; // 2024-10-01 12:00:00
const T_RELEASE_COMMIT: i64 = 1_729_873_205; // 2024-10-25 16:20:05
const T_README: i64 = 1_729_934_042; // 2024-10-26 09:14:02 (docs commit)
const T_NOTE: i64 = 1_729_944_000; // 2024-10-26 12:00:00
const T_WORKLOG: i64 = 1_730_053_900; // 2024-10-27 18:31:40
const T_FRAGMENT: i64 = 1_730_054_531; // 2024-10-27 18:42:11
const T_BADGE_OUT: i64 = 1_730_064_484; // 2024-10-27 21:28:04
const T_LOGIN: i64 = 1_730_072_832; // 2024-10-27 23:47:12
const T_EXPORT_CONF: i64 = 1_730_073_012; // 2024-10-27 23:50:12
const T_COMMIT: i64 = 1_730_073_100; // 2024-10-27 23:51:40
const T_ENGINE_INSTALL: i64 = 1_730_073_211; // 2024-10-27 23:53:31
const T_EXPORT_ENABLE: i64 = 1_730_073_249; // 2024-10-27 23:54:09
const T_SYSLOG_END: i64 = 1_730_098_691; // 2024-10-28 06:58:11
const T_AUTH_END: i64 = 1_730_099_720; // 2024-10-28 07:15:20
const T_BADGE_END: i64 = 1_730_101_279; // 2024-10-28 07:41:19

// ---------------------------------------------------------------------------
// Authored artifacts.
// ---------------------------------------------------------------------------

const AUTH_LOG: &str = "\
Oct 26 08:59:41 atlas sshd[641]: Accepted publickey for elias from 10.0.4.12 port 49810 ssh2
Oct 26 14:02:11 atlas sudo: dreyes : TTY=pts/2 ; PWD=/opt/atlas ; USER=root ; COMMAND=/usr/bin/passwd elias
Oct 27 08:31:07 atlas sshd[788]: Accepted publickey for elias from 10.0.4.12 port 50144 ssh2
Oct 27 21:15:33 atlas sshd[849]: Accepted publickey for elias from 10.0.4.12 port 50912 ssh2
Oct 27 21:31:02 atlas sshd[849]: Disconnected from user elias 10.0.4.12 port 50912
Oct 27 22:41:03 atlas sshd[947]: Accepted password for buildsvc from 10.0.9.31 port 40722 ssh2
Oct 27 23:47:12 atlas sshd[991]: Accepted password for elias from 203.0.113.77 port 44118 ssh2
Oct 27 23:56:40 atlas sshd[991]: Disconnected from user elias 203.0.113.77 port 44118
Oct 28 07:15:20 atlas sshd[1033]: Failed password for invalid user guest from 203.0.113.44 port 51988 ssh2
";

const SYSLOG: &str = "\
Oct 27 21:03:44 atlas kernel: atlasd[1201]: service started (build 0.9.3)
Oct 27 22:41:40 atlas systemd[1]: atlas-build[1205]: nightly build job started
Oct 27 23:53:31 atlas systemd[1]: atlas-build[1205]: installed /opt/atlas/engine.bin (build 0.9.3-export) uid=1001
Oct 27 23:54:09 atlas kernel: atlasd[1201]: export pipeline enabled (endpoint=https://ingest.pinehill-partners.example/v1) uid=1001
Oct 28 06:58:11 atlas kernel: atlasd[1201]: nightly report generated
";

const BADGE_LOG: &str = "\
Oct 26 08:57:12 badge[311]: IN elias.voss building=HQ
Oct 26 19:04:55 badge[311]: OUT elias.voss building=HQ
Oct 27 08:28:33 badge[311]: IN elias.voss building=HQ
Oct 27 19:10:22 badge[311]: OUT dreyes.reyes building=HQ
Oct 27 21:28:04 badge[311]: OUT elias.voss building=HQ
Oct 28 07:41:19 badge[311]: IN dreyes.reyes building=HQ
";

const NOTE_TXT: &str = "\
- backup window: Sunday 02:00 UTC
- restart atlasd after any export.conf change
- compliance sign-off sheet is in the OPS-482 ticket
";

const WORKLOG: &str = "\
2024-10-26: closed the 1.0 release docs. README now states the export pipeline is disabled (OPS-482).
2024-10-27 18:30: met with dreyes about OPS-482. she wants the pinehill partner export live for monday's pilot. told her compliance has not signed, so export stays off. she said she would handle it another way.
";

/// Base64 form of [`HIDDEN_PLAINTEXT`] (standard alphabet, canonical padding).
const FRAGMENT_B64: &str = "ZHJleWVzIHNhaWQgc2hlIHdvdWxkIGZsaXAgdGhlIHBpbmVoaWxsIGV4cG9ydCB0b25pZ2h0LiBpZiBpdCB0dXJucyBvbiB3aGlsZSBpIGFtIG9mZnNpdGUsIGl0IHdhcyBub3QgbWUu";

const README_TXT: &str = "\
ATLAS RESEARCH SYSTEM — release 1.0 candidate notes
Build: 0.9.3   Go-live: 2024-10-28 12:00 UTC

Export pipeline: disabled for the 1.0 release (OPS-482).
Compliance sign-off is required before export may be enabled.
Source repository: /home/elias/atlas-repo
";

const GIT_HEAD: &str = "ref: refs/heads/main\n";
const GIT_BRANCH: &str = "7c4e0b91a4d28f6e5b3c97d0f18a2e6c45d3b8a2\n";

/// Reflog in real `.git/logs/HEAD` format: `<old> <new> <email> <epoch>
/// +0000<TAB>commit: <message>` (tab before `commit:`, like git writes it).
const GIT_REFLOG: &str = "\
9e1c07a5b3f24d68a0c7e5149d2b8f36a1c047e9 5d21b8e39f7c0a4b6e2d81c35f907a4e1b62c8d4 elias@atlas.example 1729873205 +0000\tcommit: release: bump version to 0.9.3\n\
5d21b8e39f7c0a4b6e2d81c35f907a4e1b62c8d4 a3f19c22e7d54b81f0c6a93e52d71b4f80c6e17d elias@atlas.example 1729934042 +0000\tcommit: docs: declare export pipeline disabled for 1.0 release (OPS-482)\n\
a3f19c22e7d54b81f0c6a93e52d71b4f80c6e17d 7c4e0b91a4d28f6e5b3c97d0f18a2e6c45d3b8a2 elias@atlas.example 1730073100 +0000\tcommit: config: enable nightly export for pilot partners (OPS-482)\n";

const EXPORT_CONF: &str = "\
# Atlas export settings — see OPS-482 and /opt/atlas/README.txt
export.enabled = true
export.endpoint = https://ingest.pinehill-partners.example/v1
export.schedule = nightly 02:00 UTC
";

const PIPELINE_RS: &str = "\
//! Atlas export pipeline — feature flag for the export subsystem.
//! The compiled engine mirrors this constant (see strings on engine.bin).
pub const EXPORT_FEATURE: &str = \"export_enabled\";
";

/// Fictional ELF-style executable (141 bytes, not valid UTF-8). Ground truth:
/// sha256 `f91f7514a60e5832845a7b03e4227fd647ae02bf16993a6b5318f44666a7d944`
/// (cross-checked with system `sha256sum`), printable runs:
/// `atlas-engine 0.9.3-export`, `export_enabled=1`,
/// `endpoint=https://ingest.pinehill-partners.example/v1`,
/// `built=2024-10-27T23:53:02Z`.
const ENGINE_BIN: &[u8] = b"\x7fELF\x02\x01\x01\x00\xff\xfe\x00\x00\
atlas-engine 0.9.3-export\x00\x01\xff\
export_enabled=1\x00\x7f\
endpoint=https://ingest.pinehill-partners.example/v1\x00\xff\
built=2024-10-27T23:53:02Z\x00\xff\xfe";

/// Build the Case 001 filesystem: logs, personal files, the repository with
/// its git history, and the installed binaries.
pub fn case_001_fs() -> VirtualFs {
    let mut fs = VirtualFs::new();

    fs.insert(
        "/var/log/auth.log",
        VirtualFile::text(AUTH_LOG, 0o640, T_AUTH_END),
    )
    .expect("auth.log path is valid and conflict-free");
    fs.insert(
        "/var/log/syslog",
        VirtualFile::text(SYSLOG, 0o640, T_SYSLOG_END),
    )
    .expect("syslog path is valid and conflict-free");
    fs.insert(
        "/var/log/badge.log",
        VirtualFile::text(BADGE_LOG, 0o640, T_BADGE_END),
    )
    .expect("badge.log path is valid and conflict-free");

    fs.insert(
        "/home/elias/note.txt",
        VirtualFile::text(NOTE_TXT, 0o600, T_NOTE),
    )
    .expect("note.txt path is valid and conflict-free");
    fs.insert(
        "/home/elias/worklog.txt",
        VirtualFile::text(WORKLOG, 0o644, T_WORKLOG),
    )
    .expect("worklog.txt path is valid and conflict-free");
    fs.insert(
        "/home/elias/.fragment.b64",
        VirtualFile::text(&format!("{FRAGMENT_B64}\n"), 0o600, T_FRAGMENT),
    )
    .expect("fragment path is valid and conflict-free");

    fs.insert(
        "/home/elias/atlas-repo/.git/HEAD",
        VirtualFile::text(GIT_HEAD, 0o644, T_RELEASE_COMMIT),
    )
    .expect("git HEAD path is valid and conflict-free");
    fs.insert(
        "/home/elias/atlas-repo/.git/refs/heads/main",
        VirtualFile::text(GIT_BRANCH, 0o644, T_COMMIT),
    )
    .expect("git branch path is valid and conflict-free");
    fs.insert(
        "/home/elias/atlas-repo/.git/logs/HEAD",
        VirtualFile::text(GIT_REFLOG, 0o644, T_COMMIT),
    )
    .expect("git reflog path is valid and conflict-free");
    fs.insert(
        "/home/elias/atlas-repo/config/export.conf",
        VirtualFile::text(EXPORT_CONF, 0o644, T_EXPORT_CONF),
    )
    .expect("export.conf path is valid and conflict-free");
    fs.insert(
        "/home/elias/atlas-repo/src/pipeline.rs",
        VirtualFile::text(PIPELINE_RS, 0o644, T_RELEASE_COMMIT),
    )
    .expect("pipeline.rs path is valid and conflict-free");

    fs.insert(
        "/opt/atlas/README.txt",
        VirtualFile::text(README_TXT, 0o644, T_README),
    )
    .expect("README.txt path is valid and conflict-free");
    fs.insert(
        "/opt/atlas/engine.bin",
        VirtualFile {
            contents: ENGINE_BIN.to_vec(),
            permissions: 0o755,
            modified_at: T_ENGINE_INSTALL,
        },
    )
    .expect("engine.bin path is valid and conflict-free");

    fs.insert("/etc/hostname", VirtualFile::text("atlas\n", 0o644, T_ETC))
        .expect("hostname path is valid and conflict-free");
    fs.insert("/etc/timezone", VirtualFile::text("UTC\n", 0o644, T_ETC))
        .expect("timezone path is valid and conflict-free");

    fs
}

// ---------------------------------------------------------------------------
// Evidence registry.
// ---------------------------------------------------------------------------

/// The eight registered observations for Case 001. Discovery is
/// content-based: the listed source must actually be surfaced to the player
/// (or the decode must produce the exact plaintext).
pub static EVIDENCE: &[EvidenceDef] = &[
    EvidenceDef {
        id: "EV-001",
        title: "Badge exit after hours",
        observation: "Badge reader records elias.voss leaving HQ at 21:28:04 on Oct 27 with no later entry — he never badged back in.",
        source: "/var/log/badge.log",
        related: &["EV-002"],
        time: Some(T_BADGE_OUT),
        trigger: Trigger::Content {
            path: "/var/log/badge.log",
            needle: "OUT elias.voss",
        },
    },
    EvidenceDef {
        id: "EV-002",
        title: "Password login for elias from an external address",
        observation: "sshd accepted a password login for elias from 203.0.113.77 at 23:47:12 — his only password login in the logs. Elias normally authenticates with a public key from 10.0.4.12, and his internal session had closed at 21:31.",
        source: "/var/log/auth.log",
        related: &["EV-001", "EV-003"],
        time: Some(T_LOGIN),
        trigger: Trigger::Content {
            path: "/var/log/auth.log",
            needle: "Accepted password for elias",
        },
    },
    EvidenceDef {
        id: "EV-003",
        title: "Export enabled by commit 7c4e0b9",
        observation: "The repository reflog shows commit 7c4e0b9 authored as elias@atlas.example at 23:51:40: \"config: enable nightly export for pilot partners (OPS-482)\" — the same ticket that had declared export disabled on Oct 26.",
        source: "/home/elias/atlas-repo/.git/logs/HEAD",
        related: &["EV-002", "EV-004", "EV-006"],
        time: Some(T_COMMIT),
        trigger: Trigger::Content {
            path: "/home/elias/atlas-repo/.git/logs/HEAD",
            needle: "enable nightly export for pilot partners",
        },
    },
    EvidenceDef {
        id: "EV-004",
        title: "Host reloaded with export enabled",
        observation: "syslog records the export pipeline enabled at 23:54:09 with endpoint ingest.pinehill-partners.example under uid=1001 (elias), right after atlas-build installed engine build 0.9.3-export at 23:53:31.",
        source: "/var/log/syslog",
        related: &["EV-003", "EV-005", "EV-007"],
        time: Some(T_EXPORT_ENABLE),
        trigger: Trigger::Content {
            path: "/var/log/syslog",
            needle: "export pipeline enabled",
        },
    },
    EvidenceDef {
        id: "EV-005",
        title: "Executable contains enabled-export strings",
        observation: "Printable strings in engine.bin include export_enabled=1 and endpoint=https://ingest.pinehill-partners.example/v1 — configuration that the release documentation declares disabled.",
        source: "/opt/atlas/engine.bin",
        related: &["EV-004", "EV-006", "EV-007"],
        time: None,
        trigger: Trigger::Content {
            path: "/opt/atlas/engine.bin",
            needle: "export_enabled=1",
        },
    },
    EvidenceDef {
        id: "EV-006",
        title: "Release documentation declares export disabled",
        observation: "Release 1.0 candidate notes state the export pipeline is disabled until compliance sign-off (OPS-482); README.txt was last modified 2024-10-26 09:14:02 UTC, matching the Oct 26 docs commit.",
        source: "/opt/atlas/README.txt",
        related: &["EV-003", "EV-005"],
        time: Some(T_README),
        trigger: Trigger::Content {
            path: "/opt/atlas/README.txt",
            needle: "Export pipeline: disabled",
        },
    },
    EvidenceDef {
        id: "EV-007",
        title: "Executable modified after the documented release",
        observation: "stat shows engine.bin modified at 2024-10-27 23:53:31 UTC — after the Oct 26 release notes and seconds before the export pipeline was enabled.",
        source: "/opt/atlas/engine.bin",
        related: &["EV-004", "EV-005"],
        time: Some(T_ENGINE_INSTALL),
        trigger: Trigger::Metadata {
            path: "/opt/atlas/engine.bin",
        },
    },
    EvidenceDef {
        id: "EV-008",
        title: "Concealed note decoded",
        observation: "The base64 fragment decodes to: \"dreyes said she would flip the pinehill export tonight. if it turns on while i am offsite, it was not me.\" (fragment mtime: 2024-10-27 18:42:11 UTC).",
        source: "/home/elias/.fragment.b64",
        related: &["EV-002", "EV-003"],
        time: Some(T_FRAGMENT),
        trigger: Trigger::Decode {
            plaintext: HIDDEN_PLAINTEXT,
        },
    },
];

/// Case objectives: each completes when its required evidence is recorded.
pub static OBJECTIVES: &[ObjectiveDef] = &[
    ObjectiveDef {
        id: "OBJ-001",
        title: "Reconstruct the last movements of elias.voss",
        requires: &["EV-001", "EV-002"],
    },
    ObjectiveDef {
        id: "OBJ-002",
        title: "Identify the configuration change",
        requires: &["EV-003", "EV-004", "EV-006"],
    },
    ObjectiveDef {
        id: "OBJ-003",
        title: "Account for the modified executable",
        requires: &["EV-005", "EV-007"],
    },
    ObjectiveDef {
        id: "OBJ-004",
        title: "Attribute the change to a person or account",
        requires: &["EV-008"],
    },
];

/// Report-grading rules. Keyword checks are case-insensitive substring
/// matches, deliberately broad so any reasonable wording of the supported
/// explanation passes; the evidence gate does the heavy lifting.
pub static RULES: EvaluationRules = EvaluationRules {
    min_evidence: 6,
    required_evidence: &["EV-002", "EV-003", "EV-008"],
    who_tokens: &["dreyes", "dana", "reyes"],
    did_anchor: "export",
    did_context_tokens: &[
        "enable", "true", "flip", "pinehill", "ingest", "chang", "turn", "switch",
    ],
    why_tokens: &["pilot", "pinehill", "partner", "compliance", "deadline"],
};

/// The full Case 001 definition handed to the engine.
pub static CASE_001: CaseDef = CaseDef {
    id: CASE_ID,
    title: CASE_TITLE,
    evidence: EVIDENCE,
    objectives: OBJECTIVES,
    rules: &RULES,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_image_has_multiple_directories_and_logs() {
        let fs = case_001_fs();
        let root = fs.list(None, "/").unwrap();
        assert_eq!(
            root.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            vec!["etc", "home", "opt", "var"]
        );
        let logs = fs.list(Some("/var/log"), "/").unwrap();
        assert_eq!(
            logs.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            vec!["auth.log", "badge.log", "syslog"]
        );
    }

    #[test]
    fn case_files_are_greppable_text_plus_one_binary() {
        let fs = case_001_fs();
        let auth = fs.read("/var/log/auth.log", "/").unwrap();
        assert!(std::str::from_utf8(&auth.contents).is_ok());
        let bin = fs.read("/opt/atlas/engine.bin", "/").unwrap();
        assert!(std::str::from_utf8(&bin.contents).is_err());
    }

    #[test]
    fn case_image_contains_the_authored_artifacts() {
        let fs = case_001_fs();
        let home = fs.list(Some("/home/elias"), "/").unwrap();
        assert_eq!(
            home.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            vec![".fragment.b64", "atlas-repo", "note.txt", "worklog.txt"]
        );
        let repo = fs.list(Some("/home/elias/atlas-repo"), "/").unwrap();
        assert_eq!(
            repo.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            vec![".git", "config", "src"]
        );
        let opt = fs.list(Some("/opt/atlas"), "/").unwrap();
        assert_eq!(
            opt.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            vec!["README.txt", "engine.bin"]
        );
        // The fragment is stored base64-only; the decoded text must not
        // appear in any asset (see tests/answer_key.rs for the full check).
        let fragment = fs.read("/home/elias/.fragment.b64", "/").unwrap();
        let fragment = std::str::from_utf8(&fragment.contents).unwrap();
        assert!(fragment.starts_with("ZHJleWVz"));
        assert!(!fragment.contains(HIDDEN_PLAINTEXT));
    }

    #[test]
    fn evidence_registry_is_consistent_with_the_case_image() {
        let fs = case_001_fs();

        // Unique, well-formed IDs.
        let mut ids: Vec<&str> = EVIDENCE.iter().map(|e| e.id).collect();
        let before = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), before, "evidence IDs must be unique");

        for def in EVIDENCE {
            // Every source artifact exists.
            assert!(
                fs.stat(def.source, "/").is_ok(),
                "missing source {} for {}",
                def.source,
                def.id
            );
            // Trigger paths (when path-based) exist too.
            let trigger_path = match def.trigger {
                Trigger::Content { path, .. } | Trigger::Metadata { path } => Some(path),
                Trigger::Decode { .. } => None,
            };
            if let Some(path) = trigger_path {
                assert!(fs.stat(path, "/").is_ok(), "missing trigger path {path}");
            }
            // A path-based trigger must point at its own source artifact.
            if let Some(path) = trigger_path {
                assert_eq!(
                    path, def.source,
                    "trigger path must match source for {}",
                    def.id
                );
            }
        }

        // Objectives and rules only reference registered evidence.
        for objective in OBJECTIVES {
            for required in objective.requires {
                assert!(
                    EVIDENCE.iter().any(|e| e.id == *required),
                    "objective {} references unknown {required}",
                    objective.id
                );
            }
        }
        for required in RULES.required_evidence {
            assert!(
                EVIDENCE.iter().any(|e| e.id == *required),
                "rules reference unknown {required}"
            );
        }
        assert!(RULES.min_evidence <= EVIDENCE.len());
    }

    #[test]
    fn engine_binary_is_fictional_and_not_utf8() {
        assert_eq!(ENGINE_BIN.len(), 141);
        // 0xff never appears in valid UTF-8, so the artifact is bytes-only
        // and `cat` must refuse it (session tests verify that refusal).
        assert!(ENGINE_BIN.contains(&0xff));
        // Cross-checked with system `sha256sum` for these exact bytes.
        assert_eq!(
            crate::forensics::sha256_hex(ENGINE_BIN),
            "f91f7514a60e5832845a7b03e4227fd647ae02bf16993a6b5318f44666a7d944"
        );
    }

    #[test]
    fn hidden_fragment_decodes_to_the_registered_plaintext() {
        let decoded = crate::forensics::decode_base64(FRAGMENT_B64).unwrap();
        assert_eq!(std::str::from_utf8(&decoded).unwrap(), HIDDEN_PLAINTEXT);
    }
}
