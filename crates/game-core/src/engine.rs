//! The game engine: turns input strings into outputs and state changes.
//!
//! This is the shared core used by both the native CLI and (later) the WASM
//! adapter. It parses input into a typed [`Command`], executes it against the
//! virtual filesystem, and returns a [`CommandOutput`]. Evidence discovery and
//! objective tracking will attach to the same execute call in a later
//! milestone; `CommandOutput` is the extension point for those state changes.

use crate::command::{Command, ParseError};
use crate::vfs::{EntryKind, VfsError, VirtualFs};

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

Searching
  grep [-n] PATTERN PATH   search one text file; -n prints line numbers

Notes
  - Every path lives inside the simulated case filesystem. The host
    operating system is never read or executed.
  - grep searches a single file (directories are not searched yet).
  - Paths that climb above / with `..` are rejected as traversal attempts.
  - Quotes group arguments, e.g.: grep -n \"failed login\" /var/log/auth.log";

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
        for command in ["help", "pwd", "ls", "cd", "cat", "grep"] {
            assert!(help.contains(command), "help is missing {command}");
        }
        // Not implemented until Milestone 2.
        assert!(!help.contains("stat"));
    }
}
