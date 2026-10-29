//! End-to-end session test: drives the engine through the real Case 001
//! scaffolding image the way a player would, asserting exact outputs.

use game_core::GameEngine;

/// Run a command and return its output text.
fn run(engine: &mut GameEngine, input: &str) -> String {
    engine.execute(input).text
}

#[test]
fn player_can_navigate_and_search_the_case_image() {
    let mut engine = GameEngine::case_001();

    // Fresh start.
    assert_eq!(run(&mut engine, "pwd"), "/");
    assert_eq!(run(&mut engine, "ls"), "etc/\nhome/\nopt/\nvar/");

    // Relative navigation.
    assert_eq!(run(&mut engine, "cd var/log"), "");
    assert_eq!(run(&mut engine, "pwd"), "/var/log");
    assert_eq!(run(&mut engine, "ls"), "auth.log\nsyslog");

    // Search with line numbers (the bread-and-butter forensic move).
    assert_eq!(
        run(&mut engine, "grep -n \"Accepted password\" auth.log"),
        "auth.log:4:Oct 27 22:41:03 atlas sshd[947]: Accepted password for buildsvc from 10.0.9.31 port 40722 ssh2"
    );
    // Quoted pattern with spaces, absolute path, no -n.
    assert_eq!(
        run(&mut engine, r#"grep "New session" /var/log/auth.log"#),
        "/var/log/auth.log:Oct 27 23:12:55 atlas systemd-logind[604]: New session 44 of user elias."
    );
    // A miss is quiet, like real grep.
    assert_eq!(run(&mut engine, "grep -n zzzz auth.log"), "");

    // Back to the root, then absolute navigation into another directory.
    assert_eq!(run(&mut engine, "cd /"), "");
    assert_eq!(run(&mut engine, "ls /home/elias"), "note.txt\nworklog.txt");
    assert_eq!(
        run(&mut engine, "cat /home/elias/note.txt"),
        "PLACEHOLDER NOTES — Milestone 1 scaffolding, not final case content.\n- ask ops about the atlas configuration reload\n- backup window is Sunday 02:00 UTC"
    );
}

#[test]
fn player_cannot_escape_the_virtual_root() {
    let mut engine = GameEngine::case_001();
    assert_eq!(
        run(&mut engine, "cd ../../../../../.."),
        "cd: ../../../../../..: path escapes the virtual root"
    );
    assert_eq!(
        run(&mut engine, "cat /../../../etc/passwd"),
        "cat: /../../../etc/passwd: path escapes the virtual root"
    );
    assert_eq!(
        run(&mut engine, "cd /etc/hostname"),
        "cd: /etc/hostname: Not a directory"
    );
    // Still safely at the root.
    assert_eq!(run(&mut engine, "pwd"), "/");
}

#[test]
fn binary_artifact_is_inspected_safely_as_bytes() {
    let mut engine = GameEngine::case_001();
    // cat refuses politely...
    assert_eq!(
        run(&mut engine, "cat /opt/atlas/engine.bin"),
        "cat: /opt/atlas/engine.bin: cannot display: not valid UTF-8 text"
    );
    // ...and grep reports binary matches without dumping bytes.
    assert_eq!(
        run(&mut engine, "grep ELF /opt/atlas/engine.bin"),
        "grep: /opt/atlas/engine.bin: binary file matches"
    );
    assert_eq!(run(&mut engine, "grep XYZ /opt/atlas/engine.bin"), "");
}

#[test]
fn forensic_toolkit_examines_the_binary_artifact() {
    let mut engine = GameEngine::case_001();

    // Metadata, byte dump, printable runs, and a real SHA-256 digest.
    assert_eq!(
        run(&mut engine, "stat /opt/atlas/engine.bin"),
        "  Path: /opt/atlas/engine.bin\n  Type: regular file\n  Size: 10 bytes\n  Permissions: 0755\n  Modified: 2024-10-27 22:00:00 UTC"
    );
    assert_eq!(
        run(&mut engine, "hexdump /opt/atlas/engine.bin"),
        "00000000  7f 45 4c 46 02 01 01 00  ff fe                    |.ELF......|"
    );
    // Default minimum (4) misses the 3-character run; -n 3 finds it.
    assert_eq!(run(&mut engine, "strings /opt/atlas/engine.bin"), "");
    assert_eq!(
        run(&mut engine, "strings -n 3 /opt/atlas/engine.bin"),
        "ELF"
    );
    // Digest cross-checked against system `sha256sum` for these bytes.
    assert_eq!(
        run(&mut engine, "sha256sum /opt/atlas/engine.bin"),
        "f6a87668bcc94b9708ec03e72c7c12907dfc45edd944ddcaa8ac310a36485d18  /opt/atlas/engine.bin"
    );

    // Directories and text files work with stat too.
    assert_eq!(
        run(&mut engine, "stat /var/log"),
        "  Path: /var/log\n  Type: directory\n  Size: -\n  Permissions: 0755\n  Modified: -"
    );

    // Decoding works end to end without touching the filesystem.
    assert_eq!(run(&mut engine, "decode hex 434153452d303031"), "CASE-001");
    assert_eq!(run(&mut engine, "decode base64 UkVDT1JERUQ="), "RECORDED");
}

#[test]
fn malformed_input_is_reported_not_executed() {
    let mut engine = GameEngine::case_001();
    assert_eq!(
        run(&mut engine, "sudo rm -rf /"),
        "unknown command: sudo\nType 'help' to list supported commands."
    );
    assert_eq!(
        run(&mut engine, "cat \"unterminated"),
        "syntax error: unterminated quote"
    );
    assert_eq!(run(&mut engine, ""), "");
    // The session state never changes on bad input.
    assert_eq!(run(&mut engine, "pwd"), "/");
}

#[test]
fn help_describes_the_supported_command_set() {
    let mut engine = GameEngine::case_001();
    let help = run(&mut engine, "help");
    for expected in [
        "help",
        "pwd",
        "ls",
        "cd",
        "cat",
        "grep",
        "-n",
        "simulated case filesystem",
    ] {
        assert!(help.contains(expected), "help missing: {expected}");
    }
}
