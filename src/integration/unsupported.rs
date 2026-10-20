//! Systems where mder can't register yet. Linux is planned: a desktop file
//! in ~/.local/share/applications listing the MIME types.

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
