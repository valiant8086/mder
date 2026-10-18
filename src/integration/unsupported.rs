//! Systems where mder can't register yet. Linux is planned: a desktop file
//! in ~/.local/share/applications listing the MIME types.

use std::io;

use super::{Change, FileKind, Status};

pub const SUPPORTED: bool = false;

pub fn status(_kind: &FileKind, _ext: &str) -> Status {
    Status::NotRegistered
}

pub fn apply(_changes: &[Change]) -> io::Result<()> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "Adding mder to Open with isn't available on this system yet."))
}
