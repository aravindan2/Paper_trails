//! Case 001 ("The Last Commit") — scaffolding filesystem.
//!
//! Everything in this module is **placeholder content for Milestone 1**. It
//! exists so navigation and search commands have something realistic to work
//! against. No story facts are committed here: timestamps, log lines, and file
//! contents will be authored, cross-checked, and replaced during Milestone 3
//! (case authoring), when the answer key is kept out of public assets.

use crate::vfs::{VirtualFile, VirtualFs};

/// Stable identifier for the first case.
pub const CASE_ID: &str = "CASE-001";

/// Title shown to the player.
pub const CASE_TITLE: &str = "The Last Commit";

// Fixed timestamps keep output deterministic in tests. Roughly late October
// 2024, matching the placeholder log lines below.
const T_LOG_EVE: i64 = 1_730_066_400; // 2024-10-27 22:00:00 UTC
const T_LOG_NIGHT: i64 = 1_730_070_420; // 2024-10-27 23:07:00 UTC
const T_LOG_MORNING: i64 = 1_730_110_520; // 2024-10-28 10:15:20 UTC
const T_NOTE: i64 = 1_729_988_400; // 2024-10-27 00:20:00 UTC
const T_WORKLOG: i64 = 1_729_944_000; // 2024-10-26 12:00:00 UTC
const T_README: i64 = 1_727_740_800; // 2024-10-01 12:00:00 UTC

const AUTH_LOG_PLACEHOLDER: &str = "\
# PLACEHOLDER — Milestone 1 scaffolding, replaced during case authoring (Milestone 3).
Oct 27 21:02:11 atlas sshd[812]: Accepted publickey for elias from 10.0.4.12 port 51104 ssh2
Oct 27 21:04:39 atlas sudo: elias : TTY=pts/0 ; PWD=/home/elias ; USER=root ; COMMAND=/usr/bin/systemctl status atlas
Oct 27 22:41:03 atlas sshd[947]: Accepted password for buildsvc from 10.0.9.31 port 40722 ssh2
Oct 27 23:12:55 atlas systemd-logind[604]: New session 44 of user elias.
Oct 28 07:15:20 atlas sshd[1033]: Failed password for invalid user guest from 203.0.113.44 port 51988 ssh2
";

const SYSLOG_PLACEHOLDER: &str = "\
# PLACEHOLDER — Milestone 1 scaffolding, replaced during case authoring (Milestone 3).
Oct 27 21:03:02 atlas kernel: atlasd[1201]: service started (build 0.9.3)
Oct 27 22:41:47 atlas kernel: atlasd[1201]: configuration reload requested by buildsvc
Oct 28 06:58:11 atlas kernel: atlasd[1201]: nightly report generated
";

const NOTE_PLACEHOLDER: &str = "\
PLACEHOLDER NOTES — Milestone 1 scaffolding, not final case content.
- ask ops about the atlas configuration reload
- backup window is Sunday 02:00 UTC
";

const WORKLOG_PLACEHOLDER: &str = "\
PLACEHOLDER WORKLOG — Milestone 1 scaffolding, not final case content.
2024-10-26: routine maintenance, nothing unusual.
";

const README_PLACEHOLDER: &str = "\
ATLAS RESEARCH SYSTEM — PLACEHOLDER README (Milestone 1 scaffolding).
This image contains the Atlas research system, release 0.9.3.
";

/// Build the placeholder case filesystem.
///
/// Layout: `/home/elias`, `/var/log`, `/opt/atlas`, and `/etc` — enough to
/// exercise navigation, listing, metadata, and search commands.
pub fn case_001_fs() -> VirtualFs {
    let mut fs = VirtualFs::new();

    fs.insert(
        "/var/log/auth.log",
        VirtualFile::text(AUTH_LOG_PLACEHOLDER, 0o640, T_LOG_NIGHT),
    )
    .expect("auth.log path is valid and conflict-free");
    fs.insert(
        "/var/log/syslog",
        VirtualFile::text(SYSLOG_PLACEHOLDER, 0o640, T_LOG_MORNING),
    )
    .expect("syslog path is valid and conflict-free");
    fs.insert(
        "/home/elias/note.txt",
        VirtualFile::text(NOTE_PLACEHOLDER, 0o600, T_NOTE),
    )
    .expect("note.txt path is valid and conflict-free");
    fs.insert(
        "/home/elias/worklog.txt",
        VirtualFile::text(WORKLOG_PLACEHOLDER, 0o644, T_WORKLOG),
    )
    .expect("worklog.txt path is valid and conflict-free");
    fs.insert(
        "/opt/atlas/README.txt",
        VirtualFile::text(README_PLACEHOLDER, 0o644, T_README),
    )
    .expect("README.txt path is valid and conflict-free");
    fs.insert(
        "/etc/hostname",
        VirtualFile::text("atlas\n", 0o644, 1_727_740_800),
    )
    .expect("hostname path is valid and conflict-free");
    // A small non-UTF-8 artifact so `cat`/`grep` binary handling is exercised
    // by the real case image (ELF-style header bytes, fictional).
    fs.insert(
        "/opt/atlas/engine.bin",
        VirtualFile {
            contents: vec![0x7f, b'E', b'L', b'F', 0x02, 0x01, 0x01, 0x00, 0xff, 0xfe],
            permissions: 0o755,
            modified_at: T_LOG_EVE,
        },
    )
    .expect("engine.bin path is valid and conflict-free");

    fs
}

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
            vec!["auth.log", "syslog"]
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
}
