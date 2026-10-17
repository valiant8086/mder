//! `mder --register`: adds mder to Explorer's Open with list for markdown
//! files, for the current user only. It never changes which program opens
//! them by default. `mder --unregister` removes it again.

use std::io;

const EXTENSIONS: [&str; 3] = [".md", ".markdown", ".mdown"];
const PROG_ID: &str = "mder.markdown";

/// Registers the running exe, wherever it lives.
#[cfg(windows)]
pub fn register() -> io::Result<String> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let exe = std::env::current_exe()?;
    let command = format!("\"{}\" \"%1\"", exe.display());
    let classes = RegKey::predef(HKEY_CURRENT_USER).create_subkey(r"Software\Classes")?.0;

    let (app, _) = classes.create_subkey(r"Applications\mder.exe")?;
    app.set_value("FriendlyAppName", &"mder")?;
    app.create_subkey(r"shell\open\command")?.0.set_value("", &command)?;
    let (types, _) = app.create_subkey("SupportedTypes")?;

    let (prog, _) = classes.create_subkey(PROG_ID)?;
    prog.set_value("", &"Markdown document")?;
    prog.set_value("FriendlyTypeName", &"Markdown document")?;
    prog.create_subkey(r"shell\open\command")?.0.set_value("", &command)?;

    for ext in EXTENSIONS {
        types.set_value(ext, &"")?;
        classes.create_subkey(format!(r"{ext}\OpenWithProgids"))?.0.set_value(PROG_ID, &"")?;
    }
    notify_shell();
    Ok(format!(
        "mder is now in the Open with list for {} files.\n\nIt's registered at:\n{}\n\nIf you move mder, run mder --register again from its new place.",
        EXTENSIONS.join(", "),
        exe.display()
    ))
}

#[cfg(windows)]
pub fn unregister() -> io::Result<String> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_ALL_ACCESS};

    let classes = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(r"Software\Classes", KEY_ALL_ACCESS)?;
    let ignore_missing = |r: io::Result<()>| match r {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    };
    ignore_missing(classes.delete_subkey_all(r"Applications\mder.exe"))?;
    ignore_missing(classes.delete_subkey_all(PROG_ID))?;
    for ext in EXTENSIONS {
        if let Ok(key) = classes.open_subkey_with_flags(format!(r"{ext}\OpenWithProgids"), KEY_ALL_ACCESS) {
            ignore_missing(key.delete_value(PROG_ID))?;
        }
    }
    notify_shell();
    Ok("mder has been removed from the Open with list.".to_string())
}

/// Tells Explorer that file associations changed.
#[cfg(windows)]
fn notify_shell() {
    #[link(name = "shell32")]
    unsafe extern "system" {
        fn SHChangeNotify(event_id: i32, flags: u32, item1: *const std::ffi::c_void, item2: *const std::ffi::c_void);
    }
    const SHCNE_ASSOCCHANGED: i32 = 0x0800_0000;
    const SHCNF_IDLIST: u32 = 0;
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, std::ptr::null(), std::ptr::null()) };
}

#[cfg(not(windows))]
pub fn register() -> io::Result<String> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "--register is only available on Windows so far."))
}

#[cfg(not(windows))]
pub fn unregister() -> io::Result<String> {
    register()
}
