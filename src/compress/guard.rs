//! The trust boundary. Every path from a tool call is validated here before a
//! single byte is read. Rules and rationale: `docs/SECURITY.md`.

use std::fmt;
use std::path::{Path, PathBuf};

/// Hard ceiling on a file we will read into memory. The whole point of this
/// project is to coexist with a local model on an 8GB machine.
pub const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;

/// File names and extensions refused even inside the allowed root.
const DENIED_NAMES: &[&str] =
    &[".env", ".netrc", ".npmrc", "id_rsa", "id_ed25519", "credentials", ".htpasswd"];
const DENIED_EXTENSIONS: &[&str] = &["pem", "key", "p12", "pfx", "keystore"];

/// Why a path was refused.
///
/// Variants are categories on purpose: a raw `io::Error` message leaks
/// directory structure back to the caller.
#[derive(Debug, PartialEq, Eq)]
pub enum GuardError {
    EmptyPath,
    NotFound,
    NotARegularFile,
    OutsideRoot,
    Denied,
    TooLarge { bytes: u64 },
    NotUtf8,
    Unreadable,
}

impl fmt::Display for GuardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPath => write!(f, "filePath must not be empty"),
            Self::NotFound => write!(f, "file not found"),
            Self::NotARegularFile => write!(f, "not a regular file"),
            Self::OutsideRoot => write!(f, "path resolves outside the allowed root"),
            Self::Denied => write!(f, "file name is on the sensitive-file deny list"),
            Self::TooLarge { bytes } => {
                write!(f, "file is {bytes} bytes, limit is {MAX_FILE_BYTES}")
            }
            Self::NotUtf8 => write!(f, "file is not valid UTF-8 text"),
            Self::Unreadable => write!(f, "file could not be read"),
        }
    }
}

impl std::error::Error for GuardError {}

fn is_denied(path: &Path) -> bool {
    let name_denied = path
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|name| DENIED_NAMES.iter().any(|d| name.eq_ignore_ascii_case(d)));

    let ext_denied = path
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|ext| DENIED_EXTENSIONS.iter().any(|d| ext.eq_ignore_ascii_case(d)));

    // `.git/config` holds credentials in some setups; the whole dir is noise anyway.
    let in_git_dir = path.components().any(|c| c.as_os_str() == ".git");

    name_denied || ext_denied || in_git_dir
}

/// Validate `requested` against `root` and return the canonical path.
///
/// Canonicalization happens *first*, so `..` and symlinks are resolved before
/// the containment check. Checking the raw string for `".."` is not enough —
/// a symlink defeats it.
///
/// # Errors
/// See [`GuardError`].
pub fn resolve(requested: &str, root: &Path) -> Result<PathBuf, GuardError> {
    if requested.trim().is_empty() {
        return Err(GuardError::EmptyPath);
    }

    let root = root.canonicalize().map_err(|_| GuardError::Unreadable)?;
    let candidate =
        if Path::new(requested).is_absolute() { PathBuf::from(requested) } else { root.join(requested) };

    let resolved = candidate.canonicalize().map_err(|_| GuardError::NotFound)?;

    if !resolved.starts_with(&root) {
        return Err(GuardError::OutsideRoot);
    }
    if is_denied(&resolved) {
        return Err(GuardError::Denied);
    }

    let metadata = resolved.metadata().map_err(|_| GuardError::Unreadable)?;
    if !metadata.is_file() {
        // Also catches FIFOs and device files, which would block forever.
        return Err(GuardError::NotARegularFile);
    }
    if metadata.len() > MAX_FILE_BYTES {
        return Err(GuardError::TooLarge { bytes: metadata.len() });
    }

    Ok(resolved)
}

/// Read a file that has already passed [`resolve`] as UTF-8 text.
///
/// Takes a validated `&Path`, not a caller-supplied string, so the boundary
/// cannot be crossed twice or skipped by accident.
///
/// # Errors
/// See [`GuardError`].
pub fn read_text(path: &Path) -> Result<String, GuardError> {
    let bytes = std::fs::read(path).map_err(|_| GuardError::Unreadable)?;
    String::from_utf8(bytes).map_err(|_| GuardError::NotUtf8)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// Minimal scratch dir. ponytail: no tempfile dep for a handful of tests.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir()
                .join(format!("cce-guard-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&dir).expect("scratch dir");
            Self(dir)
        }

        fn write(&self, name: &str, body: &str) -> PathBuf {
            let path = self.0.join(name);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("parent dir");
            }
            std::fs::write(&path, body).expect("write file");
            path
        }

        fn root(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn reads_a_normal_file_inside_the_root() {
        let scratch = Scratch::new();
        scratch.write("ok.rs", "fn main() {}\n");
        let path = resolve("ok.rs", scratch.root()).expect("valid path");
        assert_eq!(read_text(&path).expect("readable"), "fn main() {}\n");
    }

    #[test]
    fn rejects_traversal_out_of_the_root() {
        let scratch = Scratch::new();
        scratch.write("nested/inner.rs", "x");
        assert_eq!(
            resolve("nested/../../../etc/hosts", scratch.root()),
            Err(GuardError::OutsideRoot)
        );
    }

    #[test]
    fn rejects_an_absolute_path_outside_the_root() {
        let scratch = Scratch::new();
        assert_eq!(resolve("/etc/hosts", scratch.root()), Err(GuardError::OutsideRoot));
    }

    #[test]
    fn rejects_an_empty_path() {
        let scratch = Scratch::new();
        assert_eq!(resolve("   ", scratch.root()), Err(GuardError::EmptyPath));
    }

    #[test]
    fn rejects_a_missing_file() {
        let scratch = Scratch::new();
        assert_eq!(resolve("nope.rs", scratch.root()), Err(GuardError::NotFound));
    }

    #[test]
    fn rejects_a_directory() {
        let scratch = Scratch::new();
        scratch.write("sub/f.rs", "x");
        assert_eq!(resolve("sub", scratch.root()), Err(GuardError::NotARegularFile));
    }

    #[test]
    fn rejects_deny_listed_names_and_extensions() {
        let scratch = Scratch::new();
        scratch.write(".env", "SECRET=1");
        scratch.write("server.pem", "-----BEGIN-----");
        assert_eq!(resolve(".env", scratch.root()), Err(GuardError::Denied));
        assert_eq!(resolve("server.pem", scratch.root()), Err(GuardError::Denied));
    }

    #[test]
    fn rejects_anything_inside_a_git_directory() {
        let scratch = Scratch::new();
        scratch.write(".git/config", "[user]");
        assert_eq!(resolve(".git/config", scratch.root()), Err(GuardError::Denied));
    }

    #[test]
    fn rejects_non_utf8_content() {
        let scratch = Scratch::new();
        let path = scratch.0.join("bin.rs");
        std::fs::write(&path, [0xff_u8, 0xfe, 0x00]).expect("write");
        let resolved = resolve("bin.rs", scratch.root()).expect("valid path");
        assert_eq!(read_text(&resolved), Err(GuardError::NotUtf8));
    }

    #[test]
    fn error_messages_never_contain_a_path() {
        // SECURITY.md: errors carry a category, not directory structure.
        let scratch = Scratch::new();
        let message = resolve("/etc/hosts", scratch.root()).unwrap_err().to_string();
        assert!(!message.contains('/'), "{message}");
    }
}
