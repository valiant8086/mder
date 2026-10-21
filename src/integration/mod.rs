//! mder and the system's file types, per kind of file.
//!
//! One table of file kinds drives the Options dialog and `--register`. A kind
//! only appears once mder can open it.
//!
//! - Windows: a box offers mder in a type's Open with menu. It never changes
//!   which program opens the type by default; Windows keeps that yours.
//! - macOS: mder.app always offers itself in Open With (its Info.plist says
//!   which types it opens), so a box instead makes mder the app that opens
//!   the type on double-click, and unchecking gives the type back.
//! - Linux: a box offers mder in Open with for a whole kind, through a
//!   desktop entry, since desktops go by file type rather than extension.

use std::io;
use std::path::{Path, PathBuf};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as platform;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as platform;

#[cfg(all(unix, not(target_os = "macos")))]
mod linux;
#[cfg(all(unix, not(target_os = "macos")))]
use linux as platform;

#[cfg(not(any(windows, unix)))]
mod unsupported;
#[cfg(not(any(windows, unix)))]
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

/// One checkbox. On Windows, one extension. On macOS, one file type as the
/// system sees it right now, which may cover several extensions: .md and
/// .markdown are both "Markdown document" there.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// What the box says, such as ".md and .markdown".
    pub label: String,
    /// The extension on Windows, the type's identifier on macOS.
    pub key: String,
    pub extensions: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    NotRegistered,
    /// Registered, and pointing at this copy of mder.
    Registered,
    /// Registered, but pointing at another copy, for example one that moved.
    #[cfg_attr(not(any(windows, unix)), allow(dead_code))]
    RegisteredElsewhere(String),
    /// macOS: another app opens this type, named here.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    OpensIn(String),
}

/// Whether this system supports registering at all. On macOS, only when
/// mder runs as mder.app.
pub fn supported() -> bool {
    platform::supported()
}

/// The checkboxes for a kind, in order.
pub fn items(kind: &FileKind) -> Vec<Item> {
    platform::items(kind)
}

pub fn status(kind: &FileKind, item: &Item) -> Status {
    platform::status(kind, item)
}

/// One change: register (`true`) or unregister (`false`) one checkbox.
/// Only read where registering is supported.
#[cfg_attr(not(any(windows, unix)), allow(dead_code))]
pub struct Change<'a> {
    /// macOS goes by the item's file type alone.
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    pub kind: &'a FileKind,
    /// Linux goes by the kind alone.
    #[cfg_attr(all(unix, not(target_os = "macos")), allow(dead_code))]
    pub item: &'a Item,
    pub register: bool,
}

/// macOS: whether unregistering needs you to pick the app that opens these
/// types instead, because mder doesn't know which one did before.
pub fn needs_choice(changes: &[Change]) -> bool {
    platform::needs_choice(changes)
}

/// The apps to pick from when `needs_choice` says so: name and place.
pub fn choices(changes: &[Change]) -> Vec<(String, PathBuf)> {
    platform::choices(changes)
}

/// Applies the changes. `instead` is the app picked for types that need one.
/// On macOS they may not be finished yet: see `confirmation`.
pub fn apply(changes: &[Change], instead: Option<&Path>) -> io::Result<()> {
    platform::apply(changes, instead)
}

/// Where the last changes stand.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Confirmation {
    Done,
    /// macOS hasn't answered yet: it may be asking you to confirm, in a
    /// dialog of its own.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Waiting,
    /// macOS refused some of them, for example because you chose to keep
    /// the app you had. Reported once.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Refused,
}

pub fn confirmation() -> Confirmation {
    platform::confirmation()
}

/// `mder --register`: everything mder supports, as if every box were checked.
pub fn register_all() -> io::Result<String> {
    let items = all_items();
    apply(&changes(&items, true), None)?;
    let extensions = KINDS.iter().flat_map(|k| k.extensions).copied().collect::<Vec<_>>().join(", ");
    if cfg!(target_os = "macos") {
        return Ok(format!(
            "Double-clicking {extensions} files now opens them in mder.\n\nTo undo it, run mder --unregister, or use Settings."
        ));
    }
    let exe = std::env::current_exe()?;
    Ok(format!(
        "mder is now in the Open with list for {extensions} files.\n\nIt's registered at:\n{}\n\nIf you move mder, run mder --register again from its new place, or use Tools, Options.",
        exe.display()
    ))
}

/// `mder --unregister`: removes every registration.
pub fn unregister_all() -> io::Result<String> {
    let items = all_items();
    apply(&changes(&items, false), None)?;
    if cfg!(target_os = "macos") {
        return Ok("Double-clicking these files no longer opens them in mder.".to_string());
    }
    Ok("mder has been removed from the Open with list.".to_string())
}

fn all_items() -> Vec<(&'static FileKind, Vec<Item>)> {
    KINDS.iter().map(|kind| (kind, items(kind))).collect()
}

fn changes<'a>(items: &'a [(&'static FileKind, Vec<Item>)], register: bool) -> Vec<Change<'a>> {
    items
        .iter()
        .flat_map(|(kind, items)| items.iter().map(move |item| Change { kind, item, register }))
        .collect()
}

/// One checkbox per extension, keyed by the extension.
#[cfg_attr(unix, allow(dead_code))]
fn per_extension(kind: &FileKind) -> Vec<Item> {
    kind.extensions
        .iter()
        .map(|&ext| Item { label: ext.to_string(), key: ext.to_string(), extensions: vec![ext] })
        .collect()
}

/// A label for a set of extensions: ".md", ".md and .markdown", or
/// ".a, .b and .c".
#[cfg_attr(windows, allow(dead_code))]
fn join_extensions(extensions: &[&str]) -> String {
    match extensions {
        [] => String::new(),
        [one] => one.to_string(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod label_tests {
    use super::join_extensions;

    #[test]
    fn joins_extensions() {
        assert_eq!(join_extensions(&[".md"]), ".md");
        assert_eq!(join_extensions(&[".md", ".markdown"]), ".md and .markdown");
        assert_eq!(join_extensions(&[".a", ".b", ".c"]), ".a, .b and .c");
    }
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
        let items = items(md);
        let item = |ext: &str| items.iter().find(|i| i.key == ext).unwrap();
        register_all().unwrap();
        for i in &items {
            assert_eq!(status(md, i), Status::Registered, "{}", i.key);
        }

        apply(&[Change { kind: md, item: item(".mkd"), register: false }], None).unwrap();
        assert_eq!(status(md, item(".mkd")), Status::NotRegistered);
        assert_eq!(status(md, item(".md")), Status::Registered);

        unregister_all().unwrap();
        for i in &items {
            assert_eq!(status(md, i), Status::NotRegistered, "{}", i.key);
        }
    }
}
