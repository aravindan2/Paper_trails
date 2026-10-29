//! Virtual filesystem rooted at `/`.
//!
//! Files are stored in a sorted map keyed by normalized absolute paths so all
//! command output is deterministic. Directories are *derived* from file paths:
//! a directory exists if at least one file lives beneath it. Empty directories
//! cannot be represented in this first version (documented limitation).
//!
//! Nothing in this module ever touches the host filesystem.

use std::collections::BTreeMap;
use std::fmt;

/// Normalized path of the virtual root.
pub const ROOT: &str = "/";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Directory,
}

impl EntryKind {
    fn as_str(self) -> &'static str {
        match self {
            EntryKind::File => "regular file",
            EntryKind::Directory => "directory",
        }
    }
}

impl fmt::Display for EntryKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A file in the simulated filesystem. Contents are raw bytes so binary
/// artifacts are first-class citizens from the start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualFile {
    pub contents: Vec<u8>,
    pub permissions: u16,
    /// Unix timestamp (seconds, UTC).
    pub modified_at: i64,
}

impl VirtualFile {
    /// Convenience constructor for UTF-8 text files.
    pub fn text(contents: &str, permissions: u16, modified_at: i64) -> Self {
        VirtualFile {
            contents: contents.as_bytes().to_vec(),
            permissions,
            modified_at,
        }
    }
}

/// Metadata for a resolved path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryInfo {
    pub kind: EntryKind,
    pub size: usize,
    pub permissions: u16,
    pub modified_at: i64,
}

/// One entry produced by `ls`. `name` is the entry name (not a full path).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub kind: EntryKind,
    pub size: usize,
}

/// Errors surfaced by path resolution and filesystem lookups. Display strings
/// deliberately mimic terminal error conventions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VfsError {
    /// A `..` component tried to climb above `/`.
    EscapesRoot,
    /// The path was empty or whitespace only.
    EmptyPath,
    /// A file operation targeted a directory.
    NotAFile,
    /// A directory operation targeted a file (or a file sits where a
    /// directory component is required).
    NotADirectory,
    /// No file or directory exists at this path.
    NotFound,
    /// An insert would make a path both a file and a directory.
    Conflict,
}

impl fmt::Display for VfsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VfsError::EscapesRoot => write!(f, "path escapes the virtual root"),
            VfsError::EmptyPath => write!(f, "empty path"),
            VfsError::NotAFile => write!(f, "Is a directory"),
            VfsError::NotADirectory => write!(f, "Not a directory"),
            VfsError::NotFound => write!(f, "No such file or directory"),
            VfsError::Conflict => write!(f, "path conflicts with an existing entry"),
        }
    }
}

impl std::error::Error for VfsError {}

/// Normalize `input` against `cwd` into an absolute path with no empty, `.`,
/// or `..` components.
///
/// Climbing above the root with `..` is an error rather than being clamped,
/// so suspicious traversal attempts are visible to the player and testable.
pub fn normalize(input: &str, cwd: &str) -> Result<String, VfsError> {
    if input.trim().is_empty() {
        return Err(VfsError::EmptyPath);
    }
    let mut components: Vec<&str> = Vec::new();
    if !input.starts_with('/') {
        // Prepend the working directory for relative paths. `cwd` is always
        // already normalized, so its components are pushed as-is.
        components.extend(
            cwd.trim_start_matches('/')
                .split('/')
                .filter(|part| !part.is_empty()),
        );
    }
    for component in input.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                if components.pop().is_none() {
                    return Err(VfsError::EscapesRoot);
                }
            }
            other => components.push(other),
        }
    }
    Ok(format!("/{}", components.join("/")))
}

/// Parent paths of a normalized path, nearest first, ending at the root.
/// The root itself is included and is never a file.
fn ancestors(path: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = path;
    while let Some(idx) = current.rfind('/') {
        current = if idx == 0 { "/" } else { &current[..idx] };
        out.push(current.to_string());
        if current == "/" {
            break;
        }
    }
    out
}

/// Format a Unix timestamp as a deterministic UTC string. Implemented with
/// `std` only so the engine stays dependency-free and WASM-friendly.
pub fn format_timestamp(epoch_seconds: i64) -> String {
    let days = epoch_seconds.div_euclid(86_400);
    let secs = epoch_seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
        year,
        month,
        day,
        secs / 3_600,
        (secs % 3_600) / 60,
        secs % 60
    )
}

/// Convert days since the Unix epoch to a civil (year, month, day) date.
/// Howard Hinnant's `civil_from_days` algorithm.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if month <= 2 { y + 1 } else { y }, month, day)
}

/// The simulated filesystem. Case data is inserted at construction time;
/// commands only read from it.
#[derive(Debug, Default)]
pub struct VirtualFs {
    files: BTreeMap<String, VirtualFile>,
}

impl VirtualFs {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a file. The path is normalized relative to the root. Refuses
    /// inserts that would make a path both a file and a directory.
    pub fn insert(&mut self, path: &str, file: VirtualFile) -> Result<(), VfsError> {
        let path = normalize(path, ROOT)?;
        if path == ROOT {
            return Err(VfsError::Conflict);
        }
        if ancestors(&path).iter().any(|a| self.files.contains_key(a)) {
            return Err(VfsError::Conflict);
        }
        let prefix = format!("{path}/");
        if self.files.keys().any(|key| key.starts_with(&prefix)) {
            return Err(VfsError::Conflict);
        }
        self.files.insert(path, file);
        Ok(())
    }

    /// Resolve `path` (absolute or relative to `cwd`) to a normalized path.
    pub fn resolve(&self, path: &str, cwd: &str) -> Result<String, VfsError> {
        normalize(path, cwd)
    }

    /// Metadata for a file or directory.
    pub fn stat(&self, path: &str, cwd: &str) -> Result<EntryInfo, VfsError> {
        let path = self.resolve(path, cwd)?;
        self.stat_normalized(&path)
    }

    /// File contents. Errors if the path is a directory or missing.
    pub fn read(&self, path: &str, cwd: &str) -> Result<&VirtualFile, VfsError> {
        let path = self.resolve(path, cwd)?;
        match self.files.get(&path) {
            Some(file) => Ok(file),
            None => Err(self.lookup_miss(&path)),
        }
    }

    /// List the immediate children of a directory. `path` of `None` means the
    /// working directory. Listing a regular file yields just that entry.
    pub fn list(&self, path: Option<&str>, cwd: &str) -> Result<Vec<DirEntry>, VfsError> {
        let path = self.resolve(path.unwrap_or(cwd), cwd)?;
        if let Some(file) = self.files.get(&path) {
            return Ok(vec![DirEntry {
                name: basename(&path).to_string(),
                kind: EntryKind::File,
                size: file.contents.len(),
            }]);
        }
        if !self.is_dir(&path) {
            return Err(self.lookup_miss(&path));
        }
        // Keys always start with `/`, so the root needs no extra separator.
        let prefix = if path == ROOT {
            ROOT.to_string()
        } else {
            format!("{path}/")
        };
        let mut children: BTreeMap<String, DirEntry> = BTreeMap::new();
        for (key, file) in &self.files {
            let Some(rest) = key.strip_prefix(&prefix) else {
                continue;
            };
            match rest.split_once('/') {
                Some((name, _)) => {
                    children.entry(name.to_string()).or_insert(DirEntry {
                        name: name.to_string(),
                        kind: EntryKind::Directory,
                        size: 0,
                    });
                }
                None => {
                    children.insert(
                        rest.to_string(),
                        DirEntry {
                            name: rest.to_string(),
                            kind: EntryKind::File,
                            size: file.contents.len(),
                        },
                    );
                }
            }
        }
        Ok(children.into_values().collect())
    }

    fn stat_normalized(&self, path: &str) -> Result<EntryInfo, VfsError> {
        if let Some(file) = self.files.get(path) {
            return Ok(EntryInfo {
                kind: EntryKind::File,
                size: file.contents.len(),
                permissions: file.permissions,
                modified_at: file.modified_at,
            });
        }
        if self.is_dir(path) {
            return Ok(EntryInfo {
                kind: EntryKind::Directory,
                size: 0,
                permissions: 0o755,
                modified_at: 0,
            });
        }
        Err(self.lookup_miss(path))
    }

    /// Distinguish "not found" from "a file sits where a directory is needed".
    fn lookup_miss(&self, path: &str) -> VfsError {
        if self.is_dir(path) {
            VfsError::NotAFile
        } else if ancestors(path).iter().any(|a| self.files.contains_key(a)) {
            VfsError::NotADirectory
        } else {
            VfsError::NotFound
        }
    }

    fn is_dir(&self, path: &str) -> bool {
        if path == ROOT {
            return true;
        }
        let prefix = format!("{path}/");
        self.files.keys().any(|key| key.starts_with(&prefix))
    }
}

fn basename(path: &str) -> &str {
    match path.rsplit_once('/') {
        Some((_, last)) => last,
        None => path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Small fixture: two directories, two text files, one binary file.
    fn fixture() -> VirtualFs {
        let mut fs = VirtualFs::new();
        fs.insert(
            "/etc/hostname",
            VirtualFile::text("atlas\n", 0o644, 1_730_000_000),
        )
        .unwrap();
        fs.insert(
            "/home/elias/note.txt",
            VirtualFile::text("remember the backup window\n", 0o600, 1_730_000_100),
        )
        .unwrap();
        fs.insert(
            "/opt/atlas/tool.bin",
            VirtualFile {
                contents: vec![0x7f, b'E', b'L', b'F', 0x00, 0xff, 0xfe],
                permissions: 0o755,
                modified_at: 1_730_000_200,
            },
        )
        .unwrap();
        fs
    }

    #[test]
    fn normalize_absolute_path() {
        assert_eq!(
            normalize("/var/log/auth.log", "/"),
            Ok("/var/log/auth.log".to_string())
        );
    }

    #[test]
    fn normalize_collapses_slashes_and_dots() {
        assert_eq!(
            normalize("//var//log/./auth.log", "/home"),
            Ok("/var/log/auth.log".to_string())
        );
    }

    #[test]
    fn normalize_resolves_dotdot() {
        assert_eq!(normalize("/a/../b", "/"), Ok("/b".to_string()));
        assert_eq!(
            normalize("../log/auth.log", "/var/log"),
            Ok("/var/log/auth.log".to_string())
        );
        assert_eq!(normalize("../../x", "/var/log"), Ok("/x".to_string()));
    }

    #[test]
    fn normalize_relative_to_cwd() {
        assert_eq!(
            normalize("elias/note.txt", "/home"),
            Ok("/home/elias/note.txt".to_string())
        );
        assert_eq!(normalize(".", "/var/log"), Ok("/var/log".to_string()));
        assert_eq!(normalize("/", "/var/log"), Ok("/".to_string()));
    }

    #[test]
    fn normalize_rejects_escaping_above_root() {
        assert_eq!(
            normalize("../../../etc/passwd", "/home"),
            Err(VfsError::EscapesRoot)
        );
        assert_eq!(normalize("/../etc", "/"), Err(VfsError::EscapesRoot));
        assert_eq!(normalize("..", "/"), Err(VfsError::EscapesRoot));
    }

    #[test]
    fn normalize_rejects_empty_paths() {
        assert_eq!(normalize("", "/"), Err(VfsError::EmptyPath));
        assert_eq!(normalize("   ", "/home"), Err(VfsError::EmptyPath));
    }

    #[test]
    fn stat_distinguishes_file_dir_and_missing() {
        let fs = fixture();
        let file = fs.stat("/etc/hostname", "/").unwrap();
        assert_eq!(file.kind, EntryKind::File);
        assert_eq!(file.size, 6);
        let dir = fs.stat("/home/elias", "/").unwrap();
        assert_eq!(dir.kind, EntryKind::Directory);
        assert_eq!(dir.permissions, 0o755);
        assert_eq!(fs.stat("/no/such/place", "/"), Err(VfsError::NotFound));
        // A file where a directory component is required.
        assert_eq!(
            fs.stat("/etc/hostname/x", "/"),
            Err(VfsError::NotADirectory)
        );
    }

    #[test]
    fn stat_root_is_a_directory() {
        let fs = fixture();
        assert_eq!(fs.stat("/", "/").unwrap().kind, EntryKind::Directory);
    }

    #[test]
    fn read_returns_bytes_and_rejects_directories() {
        let fs = fixture();
        let file = fs.read("/opt/atlas/tool.bin", "/").unwrap();
        assert_eq!(
            file.contents,
            vec![0x7f, b'E', b'L', b'F', 0x00, 0xff, 0xfe]
        );
        assert_eq!(fs.read("/home", "/"), Err(VfsError::NotAFile));
        assert_eq!(fs.read("/missing.txt", "/"), Err(VfsError::NotFound));
    }

    #[test]
    fn list_is_sorted_and_marks_directories() {
        let fs = fixture();
        let root = fs.list(None, "/").unwrap();
        assert_eq!(
            root.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            vec!["etc", "home", "opt"]
        );
        assert!(root.iter().all(|e| e.kind == EntryKind::Directory));

        let elias = fs.list(Some("elias"), "/home").unwrap();
        assert_eq!(elias.len(), 1);
        assert_eq!(elias[0].name, "note.txt");
        assert_eq!(elias[0].kind, EntryKind::File);
    }

    #[test]
    fn list_file_yields_single_entry() {
        let fs = fixture();
        let entries = fs.list(Some("/etc/hostname"), "/").unwrap();
        assert_eq!(
            entries,
            vec![DirEntry {
                name: "hostname".to_string(),
                kind: EntryKind::File,
                size: 6,
            }]
        );
    }

    #[test]
    fn list_missing_path_errors() {
        let fs = fixture();
        assert_eq!(fs.list(Some("/nope"), "/"), Err(VfsError::NotFound));
    }

    #[test]
    fn insert_rejects_file_directory_conflicts() {
        let mut fs = VirtualFs::new();
        fs.insert("/a", VirtualFile::text("x", 0o644, 0)).unwrap();
        assert_eq!(
            fs.insert("/a/b", VirtualFile::text("y", 0o644, 0)),
            Err(VfsError::Conflict)
        );
        assert_eq!(
            fs.insert("/", VirtualFile::text("y", 0o644, 0)),
            Err(VfsError::Conflict)
        );

        let mut fs = VirtualFs::new();
        fs.insert("/a/b", VirtualFile::text("x", 0o644, 0)).unwrap();
        assert_eq!(
            fs.insert("/a", VirtualFile::text("y", 0o644, 0)),
            Err(VfsError::Conflict)
        );
    }

    #[test]
    fn timestamps_format_deterministically() {
        assert_eq!(format_timestamp(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(format_timestamp(-1), "1969-12-31 23:59:59 UTC");
        assert_eq!(format_timestamp(1_729_987_200), "2024-10-27 00:00:00 UTC");
        // Leap year boundary.
        assert_eq!(format_timestamp(1_709_164_800), "2024-02-29 00:00:00 UTC");
    }
}
