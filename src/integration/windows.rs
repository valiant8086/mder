//! Per-user registration under HKCU\Software\Classes. No admin rights needed.
//!
//! - `<ProgID>` (for example `mder.markdown`): the type name, and
//!   `shell\open\command` = `"<exe>" "%1"`. Shared by a kind's extensions.
//! - `.<ext>\OpenWithProgids\<ProgID>`: puts mder in that extension's Open with list.
//! - `Applications\mder.exe`: mder's own entry, with `SupportedTypes`.

use std::io;
use std::path::{Path, PathBuf};

use winreg::RegKey;
use winreg::enums::{HKEY_CURRENT_USER, KEY_ALL_ACCESS, KEY_READ};

use super::{Change, Confirmation, FileKind, Item, KINDS, Status};

const APP_KEY: &str = r"Applications\mder.exe";

pub fn supported() -> bool {
    true
}

/// One box per extension; the item's key is the extension.
pub fn items(kind: &FileKind) -> Vec<Item> {
    super::per_extension(kind)
}

/// Windows never needs an app picked: unregistering only removes mder.
pub fn needs_choice(_changes: &[Change]) -> bool {
    false
}

pub fn choices(_changes: &[Change]) -> Vec<(String, PathBuf)> {
    Vec::new()
}

/// The registry changes as soon as `apply` writes it.
pub fn confirmation() -> Confirmation {
    Confirmation::Done
}

fn classes(access: u32) -> io::Result<RegKey> {
    RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(r"Software\Classes", access)
}

/// The exe a ProgID's open command runs, if it has one.
fn command_exe(classes: &RegKey, prog_id: &str) -> Option<String> {
    let command: String = classes.open_subkey(format!(r"{prog_id}\shell\open\command")).ok()?.get_value("").ok()?;
    let command = command.trim();
    let exe = match command.strip_prefix('"') {
        Some(rest) => rest.split('"').next()?,
        None => command.split_whitespace().next()?,
    };
    Some(exe.to_string())
}

fn same_file(a: &Path, b: &Path) -> bool {
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    canon(a).to_string_lossy().to_lowercase() == canon(b).to_string_lossy().to_lowercase()
}

fn has_open_with(classes: &RegKey, kind: &FileKind, ext: &str) -> bool {
    classes
        .open_subkey(format!(r"{ext}\OpenWithProgids"))
        .is_ok_and(|key| key.enum_values().flatten().any(|(name, _)| name.eq_ignore_ascii_case(kind.prog_id)))
}

pub fn status(kind: &FileKind, item: &Item) -> Status {
    let ext = item.key.as_str();
    let Ok(classes) = classes(KEY_READ) else { return Status::NotRegistered };
    if !has_open_with(&classes, kind, ext) {
        return Status::NotRegistered;
    }
    let Some(exe) = command_exe(&classes, kind.prog_id) else { return Status::NotRegistered };
    match std::env::current_exe() {
        Ok(me) if same_file(&me, Path::new(&exe)) => Status::Registered,
        _ => Status::RegisteredElsewhere(exe),
    }
}

pub fn apply(changes: &[Change], _instead: Option<&Path>) -> io::Result<()> {
    let exe = std::env::current_exe()?;
    let command = format!("\"{}\" \"%1\"", exe.display());
    let classes = RegKey::predef(HKEY_CURRENT_USER).create_subkey(r"Software\Classes")?.0;

    for change in changes {
        let ext = change.item.key.as_str();
        if change.register {
            register(&classes, change.kind, ext, &command)?;
        } else {
            unregister(&classes, change.kind, ext)?;
        }
    }
    remove_unused(&classes)?;
    notify_shell();
    Ok(())
}

fn register(classes: &RegKey, kind: &FileKind, ext: &str, command: &str) -> io::Result<()> {
    // Registering any extension points the whole kind at this copy.
    let (prog, _) = classes.create_subkey(kind.prog_id)?;
    prog.set_value("", &kind.type_name)?;
    prog.set_value("FriendlyTypeName", &kind.type_name)?;
    prog.create_subkey(r"shell\open\command")?.0.set_value("", &command)?;

    let (app, _) = classes.create_subkey(APP_KEY)?;
    app.set_value("FriendlyAppName", &"mder")?;
    app.create_subkey(r"shell\open\command")?.0.set_value("", &command)?;
    app.create_subkey("SupportedTypes")?.0.set_value(ext, &"")?;

    classes.create_subkey(format!(r"{ext}\OpenWithProgids"))?.0.set_value(kind.prog_id, &"")?;
    Ok(())
}

fn unregister(classes: &RegKey, kind: &FileKind, ext: &str) -> io::Result<()> {
    if let Ok(key) = classes.open_subkey_with_flags(format!(r"{ext}\OpenWithProgids"), KEY_ALL_ACCESS) {
        ignore_missing(key.delete_value(kind.prog_id))?;
    }
    if let Ok(key) = classes.open_subkey_with_flags(format!(r"{APP_KEY}\SupportedTypes"), KEY_ALL_ACCESS) {
        ignore_missing(key.delete_value(ext))?;
    }
    Ok(())
}

/// Removes a kind's ProgID once none of its extensions use it, and mder's
/// own entry once nothing is registered, so nothing is left behind.
fn remove_unused(classes: &RegKey) -> io::Result<()> {
    let mut any = false;
    for kind in KINDS {
        if kind.extensions.iter().any(|ext| has_open_with(classes, kind, ext)) {
            any = true;
        } else {
            ignore_missing(classes.delete_subkey_all(kind.prog_id))?;
        }
    }
    if !any {
        ignore_missing(classes.delete_subkey_all(APP_KEY))?;
    }
    Ok(())
}

fn ignore_missing(result: io::Result<()>) -> io::Result<()> {
    match result {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// Tells Explorer that file associations changed.
fn notify_shell() {
    #[link(name = "shell32")]
    unsafe extern "system" {
        fn SHChangeNotify(event_id: i32, flags: u32, item1: *const std::ffi::c_void, item2: *const std::ffi::c_void);
    }
    const SHCNE_ASSOCCHANGED: i32 = 0x0800_0000;
    const SHCNF_IDLIST: u32 = 0;
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, std::ptr::null(), std::ptr::null()) };
}
