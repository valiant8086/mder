//! Adding mder to the system's Open with menu, per file type.
//!
//! One table of file kinds drives the Options dialog and `--register`. A kind
//! only appears once mder can open it. Registering never changes which
//! program opens a type by default; it only offers mder as a choice.

use std::io;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as platform;

#[cfg(not(windows))]
mod unsupported;
#[cfg(not(windows))]
use unsupported as platform;

pub struct FileKind {
    /// Shown in the Options dialog.
    pub name: &'static str,
    /// Windows ProgID shared by every extension of this kind.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub prog_id: &'static str,
    /// What Explorer calls a file of this kind.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub type_name: &'static str,
    pub extensions: &'static [&'static str],
    /// For Linux desktop files.
    #[allow(dead_code)]
    pub mime: &'static [&'static str],
}

pub const KINDS: &[FileKind] = &[FileKind {
    name: "Markdown files",
    prog_id: "mder.markdown",
    type_name: "Markdown document",
    extensions: &[".md", ".markdown", ".mdown", ".mkd", ".mkdn", ".mdwn"],
    mime: &["text/markdown", "text/x-markdown"],
}];

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    NotRegistered,
    /// Registered, and pointing at this copy of mder.
    Registered,
    /// Registered, but pointing at another copy, for example one that moved.
    #[cfg_attr(not(windows), allow(dead_code))]
    RegisteredElsewhere(String),
}

/// Whether this system supports registering at all.
pub fn supported() -> bool {
    platform::SUPPORTED
}

pub fn status(kind: &FileKind, ext: &str) -> Status {
    platform::status(kind, ext)
}

/// One change: register (`true`) or unregister (`false`) an extension.
/// Only read where registering is supported.
#[cfg_attr(not(windows), allow(dead_code))]
pub struct Change<'a> {
    pub kind: &'a FileKind,
    pub ext: &'a str,
    pub register: bool,
}

pub fn apply(changes: &[Change]) -> io::Result<()> {
    platform::apply(changes)
}

/// `mder --register`: everything mder supports, as if every box were checked.
pub fn register_all() -> io::Result<String> {
    let changes: Vec<Change> = all(true);
    apply(&changes)?;
    let exe = std::env::current_exe()?;
    Ok(format!(
        "mder is now in the Open with list for {} files.\n\nIt's registered at:\n{}\n\nIf you move mder, run mder --register again from its new place, or use Tools, Options.",
        KINDS.iter().flat_map(|k| k.extensions).copied().collect::<Vec<_>>().join(", "),
        exe.display()
    ))
}

/// `mder --unregister`: removes every registration.
pub fn unregister_all() -> io::Result<String> {
    apply(&all(false))?;
    Ok("mder has been removed from the Open with list.".to_string())
}

fn all(register: bool) -> Vec<Change<'static>> {
    KINDS
        .iter()
        .flat_map(|kind| kind.extensions.iter().map(move |ext| Change { kind, ext, register }))
        .collect()
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// Writes to your registry, so it only runs when asked:
    /// `cargo test -- --ignored`. It removes everything it added, which
    /// includes any registration of mder that was already there.
    #[test]
    #[ignore]
    fn register_and_unregister() {
        let md = &KINDS[0];
        register_all().unwrap();
        for ext in md.extensions {
            assert_eq!(status(md, ext), Status::Registered, "{ext}");
        }

        apply(&[Change { kind: md, ext: ".mkd", register: false }]).unwrap();
        assert_eq!(status(md, ".mkd"), Status::NotRegistered);
        assert_eq!(status(md, ".md"), Status::Registered);

        unregister_all().unwrap();
        for ext in md.extensions {
            assert_eq!(status(md, ext), Status::NotRegistered, "{ext}");
        }
    }
}
