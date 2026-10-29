//! The game engine: turns input strings into outputs and state changes.
//!
//! This is the shared core used by both the native CLI and (later) the WASM
//! adapter. It parses input into a typed [`Command`], executes it against the
//! virtual filesystem, and returns a [`CommandOutput`]. Evidence discovery and
//! objective tracking will attach to the same execute call in a later
//! milestone; `CommandOutput` is the extension point for those state changes.

use crate::command::{Command, Encoding, ParseError};
use crate::forensics;
use crate::vfs::{EntryKind, VfsError, VirtualFs, format_timestamp};

/// Mutable player-facing state that persists across commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameState {
    /// Current working directory (always normalized, starts at `/`).
    pub cwd: String,
}

impl Default for GameState {
    fn default() -> Self {
        GameState {
            cwd: "/".to_string(),
        }
    }
}

/// Result of executing one command. `text` is what the interface prints;
/// future fields (new evidence, objective updates) are added here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub text: String,
}

impl CommandOutput {
    fn empty() -> Self {
        CommandOutput {
            text: String::new(),
        }
    }
}

const HELP: &str = "\
paper-trails — supported commands

Navigation and inspection
  help                     show this message
  pwd                      print the current directory
  ls [PATH]                list a directory (default: current directory)
  cd [PATH]                change directory (bare `cd` returns to /)
  cat PATH...              print text file contents
  stat PATH                show size, permissions, and modification time

Searching
  grep [-n] PATTERN PATH   search one text file; -n prints line numbers

Forensic tools
  strings [-n LEN] PATH    printable ASCII runs (default minimum length: 4)
  hexdump PATH             hex + ASCII dump, 16 bytes per row
  sha256sum PATH           print the SHA-256 digest of a file
  decode hex|base64 VALUE  decode hex or standard base64 text

Notes
  - Every path lives inside the simulated case filesystem. The host
    operating system is never read or executed.
  - grep searches a single file (directories are not searched yet).
  - Paths that climb above / with `..` are rejected as traversal attempts.
  - Quotes group arguments, e.g.: grep -n \"failed login\" /var/log/auth.log
  - decode prints valid UTF-8 as text; other bytes are shown explicitly,
    e.g.: (bytes) 7f 45";

/// The game engine for one case.
#[derive(Debug)]
pub struct GameEngine {
    vfs: VirtualFs,
    state: GameState,
}

impl GameEngine {
    /// Build an engine over a prepared virtual filesystem.
    pub fn new(vfs: VirtualFs) -> Self {
        GameEngine {
            vfs,
            state: GameState::default(),
        }
    }

    /// Build the engine loaded with the current Case 001 scaffolding image.
    pub fn case_001() -> Self {
        Self::new(crate::case::case_001_fs())
    }

    pub fn state(&self) -> &GameState {
        &self.state
    }

    pub fn cwd(&self) -> &str {
        &self.state.cwd
    }

    /// Execute one line of player input. Never touches the host system.
    pub fn execute(&mut self, input: &str) -> CommandOutput {
        match Command::parse(input) {
            Ok(command) => self.run(command),
            Err(ParseError::EmptyInput) => CommandOutput::empty(),
            Err(err) => CommandOutput {
                text: err.to_string(),
            },
        }
    }

    fn run(&mut self, command: Command) -> CommandOutput {
        let text = match command {
            Command::Help => HELP.to_string(),
            Command::Pwd => self.state.cwd.clone(),
            Command::Ls { path } => self.cmd_ls(path.as_deref()),
            Command::Cd { path } => self.cmd_cd(&path),
            Command::Cat { paths } => self.cmd_cat(&paths),
            Command::Grep {
                pattern,
                path,
                show_line_numbers,
            } => self.cmd_grep(&pattern, &path, show_line_numbers),
            Command::Stat { path } => self.cmd_stat(&path),
            Command::Strings { path, min_length } => self.cmd_strings(&path, min_length),
            Command::Hexdump { path } => self.cmd_hexdump(&path),
            Command::Sha256Sum { path } => self.cmd_sha256sum(&path),
            Command::Decode { encoding, value } => self.cmd_decode(encoding, &value),
        };
        CommandOutput { text }
    }

    fn cmd_ls(&self, path: Option<&str>) -> String {
        match self.vfs.list(path, &self.state.cwd) {
            Ok(entries) => entries
                .iter()
                .map(|entry| match entry.kind {
                    EntryKind::Directory => format!("{}/", entry.name),
                    EntryKind::File => entry.name.clone(),
                })
                .collect::<Vec<_>>()
                .join("\n"),
            Err(err) => format!("ls: {}: {err}", path.unwrap_or(&self.state.cwd)),
        }
    }

    fn cmd_cd(&mut self, path: &str) -> String {
        match self.vfs.stat(path, &self.state.cwd) {
            Ok(entry) if entry.kind == EntryKind::Directory => {
                match self.vfs.resolve(path, &self.state.cwd) {
                    Ok(normalized) => {
                        self.state.cwd = normalized;
                        String::new()
                    }
                    Err(err) => format!("cd: {path}: {err}"),
                }
            }
            Ok(_) => format!("cd: {path}: {}", VfsError::NotADirectory),
            Err(err) => format!("cd: {path}: {err}"),
        }
    }

    fn cmd_cat(&self, paths: &[String]) -> String {
        let mut out = String::new();
        for path in paths {
            match self.vfs.read(path, &self.state.cwd) {
                Ok(file) => match std::str::from_utf8(&file.contents) {
                    Ok(text) => out.push_str(text),
                    Err(_) => {
                        out.push_str(&format!(
                            "cat: {path}: cannot display: not valid UTF-8 text\n"
                        ));
                    }
                },
                Err(err) => out.push_str(&format!("cat: {path}: {err}\n")),
            }
        }
        out.trim_end_matches('\n').to_string()
    }

    fn cmd_grep(&self, pattern: &str, path: &str, show_line_numbers: bool) -> String {
        let file = match self.vfs.read(path, &self.state.cwd) {
            Ok(file) => file,
            Err(err) => return format!("grep: {path}: {err}"),
        };
        let needle = pattern.as_bytes();
        match std::str::from_utf8(&file.contents) {
            Ok(text) => text
                .lines()
                .enumerate()
                .filter(|(_, line)| line.contains(pattern))
                .map(|(index, line)| {
                    let line = line.strip_suffix('\r').unwrap_or(line);
                    if show_line_numbers {
                        format!("{path}:{}:{line}", index + 1)
                    } else {
                        format!("{path}:{line}")
                    }
                })
                .collect::<Vec<_>>()
                .join("\n"),
            Err(_) => {
                // Non-UTF-8 content: only report whether the pattern occurs in
                // the raw bytes, mirroring grep's "binary file matches".
                let found = needle.is_empty()
                    || file
                        .contents
                        .windows(needle.len())
                        .any(|window| window == needle);
                if found {
                    format!("grep: {path}: binary file matches")
                } else {
                    String::new()
                }
            }
        }
    }

    fn cmd_stat(&self, path: &str) -> String {
        match self.vfs.stat(path, &self.state.cwd) {
            Ok(entry) => {
                // Directories have derived metadata only: the filesystem does
                // not track their size or modification time, so those fields
                // are reported honestly as "-".
                let (size, modified) = if entry.kind == EntryKind::Directory {
                    ("-".to_string(), "-".to_string())
                } else {
                    (
                        format!("{} bytes", entry.size),
                        format_timestamp(entry.modified_at),
                    )
                };
                format!(
                    "  Path: {path}\n  Type: {}\n  Size: {size}\n  Permissions: {:04o}\n  Modified: {modified}",
                    entry.kind, entry.permissions
                )
            }
            Err(err) => format!("stat: {path}: {err}"),
        }
    }

    fn cmd_strings(&self, path: &str, min_length: usize) -> String {
        let file = match self.vfs.read(path, &self.state.cwd) {
            Ok(file) => file,
            Err(err) => return format!("strings: {path}: {err}"),
        };
        forensics::extract_strings(&file.contents, min_length).join("\n")
    }

    fn cmd_hexdump(&self, path: &str) -> String {
        let file = match self.vfs.read(path, &self.state.cwd) {
            Ok(file) => file,
            Err(err) => return format!("hexdump: {path}: {err}"),
        };
        forensics::hexdump(&file.contents)
    }

    fn cmd_sha256sum(&self, path: &str) -> String {
        let file = match self.vfs.read(path, &self.state.cwd) {
            Ok(file) => file,
            Err(err) => return format!("sha256sum: {path}: {err}"),
        };
        // Same shape as the Unix tool: `<digest>  <path as typed>`.
        format!("{}  {path}", forensics::sha256_hex(&file.contents))
    }

    fn cmd_decode(&self, encoding: Encoding, value: &str) -> String {
        let decoded = match encoding {
            Encoding::Hex => forensics::decode_hex(value),
            Encoding::Base64 => forensics::decode_base64(value),
        };
        match decoded {
            Ok(bytes) => forensics::format_decoded(&bytes),
            Err(err) => format!("decode: {err}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vfs::VirtualFile;

    fn engine() -> GameEngine {
        let mut vfs = VirtualFs::new();
        vfs.insert(
            "/etc/hostname",
            VirtualFile::text("atlas\n", 0o644, 1_729_987_200),
        )
        .unwrap();
        vfs.insert(
            "/var/log/auth.log",
            VirtualFile::text(
                "Oct 27 21:02:11 atlas sshd: Accepted publickey for elias\n\
                 Oct 27 23:47:05 atlas sshd: Accepted password for buildsvc\n\
                 Oct 28 07:15:20 atlas sshd: Failed password for guest\n",
                0o640,
                1_730_066_400,
            ),
        )
        .unwrap();
        vfs.insert(
            "/home/elias/note.txt",
            VirtualFile::text("remember the backup window\n", 0o600, 1_730_000_100),
        )
        .unwrap();
        vfs.insert(
            "/opt/atlas/tool.bin",
            VirtualFile {
                contents: vec![0x7f, b'E', b'L', b'F', b'x', b'y', 0xff],
                permissions: 0o755,
                modified_at: 1_730_000_200,
            },
        )
        .unwrap();
        vfs.insert(
            "/opt/empty.dat",
            VirtualFile {
                contents: Vec::new(),
                permissions: 0o644,
                modified_at: 1_730_000_300,
            },
        )
        .unwrap();
        GameEngine::new(vfs)
    }

    #[test]
    fn starts_at_root_and_pwd_reports_it() {
        let mut engine = engine();
        assert_eq!(engine.execute("pwd").text, "/");
        assert_eq!(
            engine.execute("pwd extra").text,
            "pwd: too many arguments\nusage: pwd"
        );
    }

    #[test]
    fn cd_relative_then_pwd_reflects_new_directory() {
        let mut engine = engine();
        assert_eq!(engine.execute("cd var/log").text, "");
        assert_eq!(engine.execute("pwd").text, "/var/log");
        assert_eq!(
            engine.execute("cd auth.log").text,
            "cd: auth.log: Not a directory"
        );
        assert_eq!(engine.execute("cd").text, "");
        assert_eq!(engine.execute("pwd").text, "/");
    }

    #[test]
    fn cd_reports_missing_paths_and_traversal() {
        let mut engine = engine();
        assert_eq!(
            engine.execute("cd /nope").text,
            "cd: /nope: No such file or directory"
        );
        assert_eq!(
            engine.execute("cd ../../../../etc").text,
            "cd: ../../../../etc: path escapes the virtual root"
        );
    }

    #[test]
    fn ls_lists_directories_with_sorted_entries() {
        let mut engine = engine();
        assert_eq!(engine.execute("ls").text, "etc/\nhome/\nopt/\nvar/");
        assert_eq!(engine.execute("ls /home/elias").text, "note.txt");
        assert_eq!(engine.execute("ls /etc/hostname").text, "hostname");
        assert_eq!(
            engine.execute("ls /nope").text,
            "ls: /nope: No such file or directory"
        );
    }

    #[test]
    fn cat_prints_text_and_errors_for_each_operand() {
        let mut engine = engine();
        assert_eq!(
            engine.execute("cat /home/elias/note.txt").text,
            "remember the backup window"
        );
        // Multiple operands concatenate exactly like real cat: each file
        // contributes its own bytes (note.txt ends with a newline).
        assert_eq!(
            engine
                .execute("cat /home/elias/note.txt /home/elias/note.txt")
                .text,
            "remember the backup window\nremember the backup window"
        );
        assert_eq!(
            engine.execute("cat /home").text,
            "cat: /home: Is a directory"
        );
        assert_eq!(
            engine.execute("cat /missing.txt").text,
            "cat: /missing.txt: No such file or directory"
        );
        assert_eq!(
            engine.execute("cat /opt/atlas/tool.bin").text,
            "cat: /opt/atlas/tool.bin: cannot display: not valid UTF-8 text"
        );
        // A failed operand does not stop later operands.
        assert_eq!(
            engine.execute("cat /missing.txt /home/elias/note.txt").text,
            "cat: /missing.txt: No such file or directory\nremember the backup window"
        );
    }

    #[test]
    fn grep_with_line_numbers_is_deterministic() {
        let mut engine = engine();
        assert_eq!(
            engine
                .execute(r#"grep -n "Accepted" /var/log/auth.log"#)
                .text,
            "/var/log/auth.log:1:Oct 27 21:02:11 atlas sshd: Accepted publickey for elias\n\
             /var/log/auth.log:2:Oct 27 23:47:05 atlas sshd: Accepted password for buildsvc"
        );
        assert_eq!(
            engine.execute("grep -n guest /var/log/auth.log").text,
            "/var/log/auth.log:3:Oct 28 07:15:20 atlas sshd: Failed password for guest"
        );
    }

    #[test]
    fn grep_reports_missing_files_directories_and_quiet_misses() {
        let mut engine = engine();
        assert_eq!(
            engine.execute("grep x /missing.log").text,
            "grep: /missing.log: No such file or directory"
        );
        assert_eq!(
            engine.execute("grep x /var").text,
            "grep: /var: Is a directory"
        );
        assert_eq!(
            engine.execute("grep zzz auth.log").text,
            "grep: auth.log: No such file or directory"
        );
        assert_eq!(engine.execute("grep zzz /var/log/auth.log").text, "");
    }

    #[test]
    fn shell_operators_are_not_supported() {
        let mut engine = engine();
        // Chaining syntax is never executed: input is parsed in-process, so a
        // pipe simply starts an unrecognized command.
        assert_eq!(
            engine.execute("echo hello | grep x").text,
            "unknown command: echo\nType 'help' to list supported commands."
        );
        // And malformed chaining surfaces the first command's arity error
        // rather than running anything.
        assert_eq!(
            engine
                .execute("cd /var/log && grep -n Failed auth.log")
                .text,
            "cd: too many arguments\nusage: cd [PATH]"
        );
    }

    #[test]
    fn grep_matches_binary_bytes_as_binary() {
        let mut engine = engine();
        assert_eq!(
            engine.execute("grep ELF /opt/atlas/tool.bin").text,
            "grep: /opt/atlas/tool.bin: binary file matches"
        );
        assert_eq!(engine.execute("grep ABC /opt/atlas/tool.bin").text, "");
    }

    #[test]
    fn relative_grep_works_after_cd() {
        let mut engine = engine();
        engine.execute("cd /var/log");
        assert_eq!(
            engine.execute("grep -n Accepted auth.log").text,
            "auth.log:1:Oct 27 21:02:11 atlas sshd: Accepted publickey for elias\n\
             auth.log:2:Oct 27 23:47:05 atlas sshd: Accepted password for buildsvc"
        );
    }

    #[test]
    fn unknown_and_malformed_input_never_reaches_a_shell() {
        let mut engine = engine();
        assert_eq!(
            engine.execute("rm -rf /").text,
            "unknown command: rm\nType 'help' to list supported commands."
        );
        assert_eq!(
            engine.execute("cat \"oops").text,
            "syntax error: unterminated quote"
        );
        assert_eq!(engine.execute("").text, "");
        assert_eq!(engine.execute("   ").text, "");
    }

    #[test]
    fn help_lists_every_supported_command() {
        let mut engine = engine();
        let help = engine.execute("help").text;
        for command in [
            "help",
            "pwd",
            "ls",
            "cd",
            "cat",
            "grep",
            "stat",
            "strings",
            "hexdump",
            "sha256sum",
            "decode",
        ] {
            assert!(help.contains(command), "help is missing {command}");
        }
    }

    #[test]
    fn stat_shows_file_and_directory_metadata() {
        let mut engine = engine();
        assert_eq!(
            engine.execute("stat /home/elias/note.txt").text,
            "  Path: /home/elias/note.txt\n  Type: regular file\n  Size: 27 bytes\n  Permissions: 0600\n  Modified: 2024-10-27 03:35:00 UTC"
        );
        // Directories report derived permissions; size/mtime are not tracked.
        assert_eq!(
            engine.execute("stat /home/elias").text,
            "  Path: /home/elias\n  Type: directory\n  Size: -\n  Permissions: 0755\n  Modified: -"
        );
        assert_eq!(
            engine.execute("stat /opt/empty.dat").text,
            "  Path: /opt/empty.dat\n  Type: regular file\n  Size: 0 bytes\n  Permissions: 0644\n  Modified: 2024-10-27 03:38:20 UTC"
        );
    }

    #[test]
    fn stat_reports_errors_consistently() {
        let mut engine = engine();
        assert_eq!(
            engine.execute("stat /nope").text,
            "stat: /nope: No such file or directory"
        );
        assert_eq!(
            engine.execute("stat /etc/hostname/x").text,
            "stat: /etc/hostname/x: Not a directory"
        );
        assert_eq!(
            engine.execute("stat /../etc").text,
            "stat: /../etc: path escapes the virtual root"
        );
        assert_eq!(
            engine.execute("stat").text,
            "stat: missing operand\nusage: stat PATH"
        );
        assert_eq!(
            engine.execute("stat -l /opt").text,
            "stat: unsupported option: -l\nusage: stat PATH"
        );
    }

    #[test]
    fn strings_extracts_printable_runs_from_bytes() {
        let mut engine = engine();
        // tool.bin bytes: 7f 'E' 'L' 'F' 'x' 'y' ff — one run, "ELFxy".
        assert_eq!(engine.execute("strings /opt/atlas/tool.bin").text, "ELFxy");
        // A higher minimum filters the same run out.
        assert_eq!(engine.execute("strings -n 6 /opt/atlas/tool.bin").text, "");
        // Empty files produce no output.
        assert_eq!(engine.execute("strings /opt/empty.dat").text, "");
        // The option may follow the path operand.
        assert_eq!(engine.execute("strings /opt/atlas/tool.bin -n 6").text, "");
        assert_eq!(
            engine.execute("strings -n 3 /opt/atlas/tool.bin").text,
            "ELFxy"
        );
    }

    #[test]
    fn strings_rejects_bad_options_and_paths() {
        let mut engine = engine();
        assert_eq!(
            engine.execute("strings").text,
            "strings: missing operand\nusage: strings [-n LENGTH] PATH"
        );
        assert_eq!(
            engine.execute("strings -n 0 /opt/empty.dat").text,
            "strings: -n expects a positive integer, got '0'\nusage: strings [-n LENGTH] PATH"
        );
        assert_eq!(
            engine.execute("strings -n abc /opt/empty.dat").text,
            "strings: -n expects a positive integer, got 'abc'\nusage: strings [-n LENGTH] PATH"
        );
        assert_eq!(
            engine.execute("strings -n").text,
            "strings: missing operand\nusage: strings [-n LENGTH] PATH"
        );
        assert_eq!(
            engine.execute("strings -t x /opt/empty.dat").text,
            "strings: unsupported option: -t\nusage: strings [-n LENGTH] PATH"
        );
        assert_eq!(
            engine.execute("strings /nope").text,
            "strings: /nope: No such file or directory"
        );
        assert_eq!(
            engine.execute("strings /home").text,
            "strings: /home: Is a directory"
        );
    }

    #[test]
    fn hexdump_formats_rows_exactly() {
        let mut engine = engine();
        // Ground truth: `hexdump -C` of these seven bytes.
        assert_eq!(
            engine.execute("hexdump /opt/atlas/tool.bin").text,
            "00000000  7f 45 4c 46 78 79 ff                              |.ELFxy.|"
        );
        assert_eq!(engine.execute("hexdump /opt/empty.dat").text, "");
        assert_eq!(
            engine.execute("hexdump /nope").text,
            "hexdump: /nope: No such file or directory"
        );
        assert_eq!(
            engine.execute("hexdump /var").text,
            "hexdump: /var: Is a directory"
        );
        assert_eq!(
            engine.execute("hexdump").text,
            "hexdump: missing operand\nusage: hexdump PATH"
        );
        assert_eq!(
            engine.execute("hexdump -C /opt/empty.dat").text,
            "hexdump: unsupported option: -C\nusage: hexdump PATH"
        );
    }

    #[test]
    fn sha256sum_prints_real_digests() {
        let mut engine = engine();
        // Cross-checked with system `sha256sum` and Python hashlib.
        assert_eq!(
            engine.execute("sha256sum /opt/empty.dat").text,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  /opt/empty.dat"
        );
        assert_eq!(
            engine.execute("sha256sum /opt/atlas/tool.bin").text,
            "15fcfc4076f04d60713f52ab40ee96b883ef14fbb2c27fdee596438257f8926a  /opt/atlas/tool.bin"
        );
        assert_eq!(
            engine.execute("sha256sum /home").text,
            "sha256sum: /home: Is a directory"
        );
        assert_eq!(
            engine.execute("sha256sum").text,
            "sha256sum: missing operand\nusage: sha256sum PATH"
        );
    }

    #[test]
    fn decode_decodes_hex_and_base64_values() {
        let mut engine = engine();
        assert_eq!(engine.execute("decode hex 68656c6c6f").text, "hello");
        assert_eq!(engine.execute("decode HEX 48656C6C6F").text, "Hello");
        assert_eq!(engine.execute("decode base64 aGVsbG8=").text, "hello");
        assert_eq!(
            engine.execute("decode base64 SGVsbG8sIFdvcmxkIQ==").text,
            "Hello, World!"
        );
        // Non-UTF-8 results use the documented representation.
        assert_eq!(engine.execute("decode hex fffe").text, "(bytes) ff fe");
        assert_eq!(engine.execute("decode base64 +/8=").text, "(bytes) fb ff");
    }

    #[test]
    fn decode_reports_malformed_values_and_types() {
        let mut engine = engine();
        assert_eq!(
            engine.execute("decode hex 68656").text,
            "decode: invalid hex: odd number of digits (5)"
        );
        assert_eq!(
            engine.execute("decode hex 6g").text,
            "decode: invalid hex: invalid digit 'g' at index 1"
        );
        assert_eq!(engine.execute("decode hex ''").text, "decode: empty value");
        assert_eq!(
            engine.execute("decode base64 ab!d").text,
            "decode: invalid base64: invalid character '!' at index 2"
        );
        assert_eq!(
            engine.execute("decode base64 aGVsbG8").text,
            "decode: invalid base64: length 7 is not a multiple of 4"
        );
        assert_eq!(
            engine.execute("decode rot13 abc").text,
            "decode: unknown encoding: rot13\nusage: decode hex|base64 VALUE"
        );
        assert_eq!(
            engine.execute("decode hex").text,
            "decode: missing operand\nusage: decode hex|base64 VALUE"
        );
        assert_eq!(
            engine.execute("decode").text,
            "decode: missing operand\nusage: decode hex|base64 VALUE"
        );
        assert_eq!(
            engine.execute("decode hex a b").text,
            "decode: too many arguments\nusage: decode hex|base64 VALUE"
        );
    }
}
