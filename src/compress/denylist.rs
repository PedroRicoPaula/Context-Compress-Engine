//! Files refused even inside the allowed root.
//!
//! A deny list is a weaker guarantee than an allow list, and this one is
//! mitigation rather than a promise — `docs/SECURITY.md` says so plainly.
//! What it must not be is *confidently wrong*, which is what exact-name
//! matching made it.
//!
//! Found by audit, 2026-08-24: the list held `.env` and nothing else from that
//! family. In Next.js, Vite and CRA the committed `.env` is usually a
//! placeholder while `.env.local` holds live credentials — so the rule blocked
//! the harmless file and passed the dangerous one. Same shape of gap left
//! `id_ecdsa` and `id_dsa` readable while `id_rsa` was blocked.
//!
//! Hence prefixes, not exact names, for families that have variants.

use std::path::Path;

/// Exact names, for files with no naming family.
const DENIED_NAMES: &[&str] = &[
    ".netrc",
    ".npmrc",
    ".pgpass",
    "credentials",
    ".htpasswd",
    "secrets.yaml",
    "secrets.yml",
];

/// Name prefixes. Covers `.env.local`, `.env.production`, `id_rsa.pub`, ...
const DENIED_PREFIXES: &[&str] = &[".env", "id_rsa", "id_dsa", "id_ecdsa", "id_ed25519"];

/// Extensions refused regardless of name.
const DENIED_EXTENSIONS: &[&str] = &[
    "pem", "key", "p12", "pfx", "keystore", "jks", "kdbx", "ppk", "asc",
];

/// Whether this resolved path is refused on name grounds.
///
/// Case-insensitive: a case-sensitive rule is bypassed by `.ENV.local` on a
/// case-insensitive filesystem, which is the default on macOS and Windows.
#[must_use]
pub fn is_denied(path: &Path) -> bool {
    let name_denied = path
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|name| {
            let lower = name.to_ascii_lowercase();
            DENIED_NAMES.iter().any(|d| lower == *d)
                || DENIED_PREFIXES.iter().any(|d| lower.starts_with(d))
        });

    let ext_denied = path
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|ext| {
            DENIED_EXTENSIONS
                .iter()
                .any(|d| ext.eq_ignore_ascii_case(d))
        });

    // `.git/config` can hold credentials; the rest of the directory is noise.
    let in_git_dir = path.components().any(|c| c.as_os_str() == ".git");

    name_denied || ext_denied || in_git_dir
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::panic,
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::indexing_slicing
    )]
    use super::*;
    use std::path::PathBuf;

    fn denied(name: &str) -> bool {
        is_denied(&PathBuf::from("/project").join(name))
    }

    #[test]
    fn rejects_the_whole_dotenv_family_not_just_the_bare_file() {
        for name in [
            ".env",
            ".env.local",
            ".env.production",
            ".env.development.local",
        ] {
            assert!(denied(name), "{name} should be denied");
        }
    }

    #[test]
    fn matching_is_case_insensitive() {
        // macOS and Windows default to case-insensitive filesystems, so a
        // case-sensitive rule is bypassed by simply changing the case.
        for name in [".ENV.Local", "ID_RSA", "server.PEM"] {
            assert!(denied(name), "{name} should be denied");
        }
    }

    #[test]
    fn rejects_every_ssh_key_name_not_only_rsa_and_ed25519() {
        for name in ["id_rsa", "id_rsa.pub", "id_dsa", "id_ecdsa", "id_ed25519"] {
            assert!(denied(name), "{name} should be denied");
        }
    }

    #[test]
    fn rejects_key_and_certificate_extensions() {
        for name in [
            "server.pem",
            "private.key",
            "bundle.p12",
            "store.jks",
            "vault.kdbx",
        ] {
            assert!(denied(name), "{name} should be denied");
        }
    }

    #[test]
    fn rejects_anything_under_a_git_directory() {
        assert!(is_denied(Path::new("/project/.git/config")));
        assert!(is_denied(Path::new("/project/sub/.git/hooks/pre-commit")));
    }

    #[test]
    fn a_prefix_rule_does_not_swallow_unrelated_files() {
        // An over-broad deny list is its own kind of bug: these are ordinary
        // source files and must stay readable.
        for name in [
            "environment.rs",
            "identity.rs",
            "envelope.py",
            "keyboard.ts",
            "idea.md",
        ] {
            assert!(!denied(name), "{name} wrongly denied");
        }
    }
}
