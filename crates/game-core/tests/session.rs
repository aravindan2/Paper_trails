//! End-to-end session test: drives the engine through the real Case 001
//! image the way a player would, asserting exact outputs (navigation,
//! search, forensics — and the discovery notices the journal appends).

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
    assert_eq!(run(&mut engine, "ls"), "auth.log\nbadge.log\nsyslog");

    // Search with line numbers (the bread-and-butter forensic move). The
    // second hit surfaces a registered observation, so the journal notice
    // follows the search results.
    assert_eq!(
        run(&mut engine, "grep -n \"Accepted password\" auth.log"),
        "auth.log:6:Oct 27 22:41:03 atlas sshd[947]: Accepted password for buildsvc from 10.0.9.31 port 40722 ssh2\n\
         auth.log:7:Oct 27 23:47:12 atlas sshd[991]: Accepted password for elias from 203.0.113.77 port 44118 ssh2\n\
         evidence recorded: EV-002 — Password login for elias from an external address"
    );
    // Quoted pattern with spaces, absolute path, no -n. These lines match
    // but carry no registered observation, so the output is pure grep.
    assert_eq!(
        run(
            &mut engine,
            r#"grep "Disconnected from user elias" /var/log/auth.log"#
        ),
        "/var/log/auth.log:Oct 27 21:31:02 atlas sshd[849]: Disconnected from user elias 10.0.4.12 port 50912\n\
         /var/log/auth.log:Oct 27 23:56:40 atlas sshd[991]: Disconnected from user elias 203.0.113.77 port 44118"
    );
    // A miss is quiet, like real grep.
    assert_eq!(run(&mut engine, "grep -n zzzz auth.log"), "");

    // Back to the root, then absolute navigation into another directory.
    assert_eq!(run(&mut engine, "cd /"), "");
    assert_eq!(
        run(&mut engine, "ls /home/elias"),
        ".fragment.b64\natlas-repo/\nnote.txt\nworklog.txt"
    );
    assert_eq!(
        run(&mut engine, "cat /home/elias/note.txt"),
        "- backup window: Sunday 02:00 UTC\n\
         - restart atlasd after any export.conf change\n\
         - compliance sign-off sheet is in the OPS-482 ticket"
    );

    // The journal reflects exactly what the searches surfaced above: one
    // observation, no objectives yet (its partner evidence is unfound).
    assert_eq!(
        run(&mut engine, "evidence"),
        "CASE-001 evidence: 1 of 8\n\
         \n\
         EV-002  Password login for elias from an external address\n\
         \x20       sshd accepted a password login for elias from 203.0.113.77 at 23:47:12 — his only password login in the logs. Elias normally authenticates with a public key from 10.0.4.12, and his internal session had closed at 21:31.\n\
         \x20       source: /var/log/auth.log\n\
         \x20       related: EV-001, EV-003\n\
         \n\
         Objectives:\n\
         \x20 [ ] Reconstruct the last movements of elias.voss\n\
         \x20 [ ] Identify the configuration change\n\
         \x20 [ ] Account for the modified executable\n\
         \x20 [ ] Attribute the change to a person or account"
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

    // Metadata records the modified-timestamp observation...
    assert_eq!(
        run(&mut engine, "stat /opt/atlas/engine.bin"),
        "  Path: /opt/atlas/engine.bin\n  \
         Type: regular file\n  \
         Size: 141 bytes\n  \
         Permissions: 0755\n  \
         Modified: 2024-10-27 23:53:31 UTC\n\
         evidence recorded: EV-007 — Executable modified after the documented release"
    );
    // ...the hex+ASCII dump surfaces the printable runs (records their
    // observation)...
    assert_eq!(
        run(&mut engine, "hexdump /opt/atlas/engine.bin"),
        "00000000  7f 45 4c 46 02 01 01 00  ff fe 00 00 61 74 6c 61  |.ELF........atla|\n\
         00000010  73 2d 65 6e 67 69 6e 65  20 30 2e 39 2e 33 2d 65  |s-engine 0.9.3-e|\n\
         00000020  78 70 6f 72 74 00 01 ff  65 78 70 6f 72 74 5f 65  |xport...export_e|\n\
         00000030  6e 61 62 6c 65 64 3d 31  00 7f 65 6e 64 70 6f 69  |nabled=1..endpoi|\n\
         00000040  6e 74 3d 68 74 74 70 73  3a 2f 2f 69 6e 67 65 73  |nt=https://inges|\n\
         00000050  74 2e 70 69 6e 65 68 69  6c 6c 2d 70 61 72 74 6e  |t.pinehill-partn|\n\
         00000060  65 72 73 2e 65 78 61 6d  70 6c 65 2f 76 31 00 ff  |ers.example/v1..|\n\
         00000070  62 75 69 6c 74 3d 32 30  32 34 2d 31 30 2d 32 37  |built=2024-10-27|\n\
         00000080  54 32 33 3a 35 33 3a 30  32 5a 00 ff fe           |T23:53:02Z...|\n\
         evidence recorded: EV-005 — Executable contains enabled-export strings\n\
         objective complete: OBJ-003 — Account for the modified executable"
    );
    // ...and repeating the view with `strings` records nothing new (the
    // journal entry already exists — discoveries are recorded once).
    assert_eq!(
        run(&mut engine, "strings /opt/atlas/engine.bin"),
        "atlas-engine 0.9.3-export\n\
         export_enabled=1\n\
         endpoint=https://ingest.pinehill-partners.example/v1\n\
         built=2024-10-27T23:53:02Z"
    );
    // -n filters runs by length (only the endpoint run reaches 30).
    assert_eq!(
        run(&mut engine, "strings -n 30 /opt/atlas/engine.bin"),
        "endpoint=https://ingest.pinehill-partners.example/v1"
    );
    // Digest cross-checked against system `sha256sum` for these bytes.
    assert_eq!(
        run(&mut engine, "sha256sum /opt/atlas/engine.bin"),
        "f91f7514a60e5832845a7b03e4227fd647ae02bf16993a6b5318f44666a7d944  /opt/atlas/engine.bin"
    );

    // Directories and text files work with stat too (and a directory view
    // never records file evidence).
    assert_eq!(
        run(&mut engine, "stat /var/log"),
        "  Path: /var/log\n  Type: directory\n  Size: -\n  Permissions: 0755\n  Modified: -"
    );

    // Decoding works end to end without touching the filesystem. Neither
    // value matches the registered concealed message, so nothing is recorded.
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
        "evidence",
        "timeline",
        "submit",
        "simulated case filesystem",
    ] {
        assert!(help.contains(expected), "help missing: {expected}");
    }
}
