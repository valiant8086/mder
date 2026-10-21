//! Systems where mder can't register: anything that isn't Windows, macOS,
//! or a Unix desktop (see linux.rs).

use std::io;
use std::path::{Path, PathBuf};

use super::{Change, Confirmation, FileKind, Item, Status};

pub fn supported() -> bool {
    false
}

pub fn items(kind: &FileKind) -> Vec<Item> {
    super::per_extension(kind)
}

pub fn status(_kind: &FileKind, _item: &Item) -> Status {
    Status::NotRegistered
}

pub fn needs_choice(_changes: &[Change]) -> bool {
    false
}

pub fn choices(_changes: &[Change]) -> Vec<(String, PathBuf)> {
    Vec::new()
}

pub fn confirmation() -> Confirmation {
    Confirmation::Done
}

pub fn apply(_changes: &[Change], _instead: Option<&Path>) -> io::Result<()> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "Adding mder to Open with isn't available on this system yet."))
}
