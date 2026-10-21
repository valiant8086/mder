//! Linux and other XDG desktops: a desktop entry in your data folder
//! (`~/.local/share/applications/mder.desktop`) that lists the file types
//! mder opens. File managers read it for their Open with lists. It never
//! changes which program opens a type by default.
//!
//! Desktops go by file type (MIME type), not extension, so each kind is one
//! box. Some markdown extensions aren't known as markdown everywhere: Debian
//! 13 calls .mdown, .mkdn and .mdwn plain text. So registering a kind also
//! teaches the desktop its extensions, with a small MIME package
//! (`~/.local/share/mime/packages/mder.xml`). Everything stays in your home
//! folder, so no administrator rights are needed.

use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{Change, Confirmation, FileKind, Item, KINDS, Status};

const DESKTOP_FILE: &str = "mder.desktop";
const MIME_PACKAGE: &str = "mder.xml";

/// `$XDG_DATA_HOME`, or `~/.local/share`.
fn data_home() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share")))
}

fn applications_dir() -> Option<PathBuf> {
    data_home().map(|d| d.join("applications"))
}

fn mime_dir() -> Option<PathBuf> {
    data_home().map(|d| d.join("mime"))
}

pub fn supported() -> bool {
    data_home().is_some()
}

/// One box per kind: its types cover all its extensions.
pub fn items(kind: &FileKind) -> Vec<Item> {
    vec![Item {
        label: super::join_extensions(kind.extensions),
        key: kind.mime.join(";"),
        extensions: kind.extensions.to_vec(),
    }]
}

pub fn status(kind: &FileKind, _item: &Item) -> Status {
    let Some(entry) = read_entry() else { return Status::NotRegistered };
    if !kind.mime.iter().any(|m| entry.mime.contains(*m)) {
        return Status::NotRegistered;
    }
    match std::env::current_exe() {
        Ok(me) if same_file(&me, Path::new(&entry.exe)) => Status::Registered,
        _ => Status::RegisteredElsewhere(entry.exe),
    }
}

/// Unregistering only removes mder, so there's never an app to pick.
pub fn needs_choice(_changes: &[Change]) -> bool {
    false
}

pub fn choices(_changes: &[Change]) -> Vec<(String, PathBuf)> {
    Vec::new()
}

/// The files are written as soon as `apply` runs.
pub fn confirmation() -> Confirmation {
    Confirmation::Done
}

pub fn apply(changes: &[Change], _instead: Option<&Path>) -> io::Result<()> {
    let (Some(apps), Some(mime)) = (applications_dir(), mime_dir()) else {
        return Err(io::Error::other("couldn't find your data folder (HOME isn't set)"));
    };
    let mut types: BTreeSet<String> = read_entry().map(|e| e.mime).unwrap_or_default();
    for change in changes {
        for m in change.kind.mime {
            if change.register {
                types.insert(m.to_string());
            } else {
                types.remove(*m);
            }
        }
    }

    let entry_path = apps.join(DESKTOP_FILE);
    let package_path = mime.join("packages").join(MIME_PACKAGE);
    if types.is_empty() {
        remove_if_there(&entry_path)?;
        remove_if_there(&package_path)?;
    } else {
        let exe = std::env::current_exe()?;
        std::fs::create_dir_all(&apps)?;
        std::fs::write(&entry_path, desktop_entry(&exe, &types))?;
        std::fs::create_dir_all(package_path.parent().unwrap_or(&mime))?;
        std::fs::write(&package_path, mime_package(&types))?;
    }
    refresh(&apps, &mime);
    Ok(())
}

/// What mder's desktop entry says, as far as status needs.
struct Entry {
    exe: String,
    mime: BTreeSet<String>,
}

fn read_entry() -> Option<Entry> {
    let text = std::fs::read_to_string(applications_dir()?.join(DESKTOP_FILE)).ok()?;
    let mut exe = String::new();
    let mut mime = BTreeSet::new();
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("Exec=") {
            exe = exec_program(value);
        } else if let Some(value) = line.strip_prefix("MimeType=") {
            mime = value.split(';').filter(|m| !m.is_empty()).map(str::to_string).collect();
        }
    }
    Some(Entry { exe, mime })
}

fn desktop_entry(exe: &Path, types: &BTreeSet<String>) -> String {
    let mime: String = types.iter().map(|m| format!("{m};")).collect();
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=mder\n\
         GenericName=Markdown editor\n\
         Comment=Edit and read markdown, made for screen readers\n\
         Exec={} %f\n\
         Terminal=false\n\
         Categories=Utility;TextEditor;\n\
         MimeType={mime}\n",
        quote_exec(&exe.to_string_lossy())
    )
}

/// Teaches the desktop which extensions belong to each registered type.
fn mime_package(types: &BTreeSet<String>) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!-- Written by mder, for the file types it opens. Unchecking them in mder's Options removes it. -->\n\
         <mime-info xmlns=\"http://www.freedesktop.org/standards/shared-mime-info\">\n",
    );
    for kind in KINDS {
        let Some(main) = kind.mime.first() else { continue };
        if !types.contains(*main) {
            continue;
        }
        xml.push_str(&format!("  <mime-type type=\"{main}\">\n"));
        for ext in kind.extensions {
            xml.push_str(&format!("    <glob pattern=\"*{ext}\"/>\n"));
        }
        xml.push_str("  </mime-type>\n");
    }
    xml.push_str("</mime-info>\n");
    xml
}

/// The desktop entry spec's quoting for a program path in Exec.
fn quote_exec(path: &str) -> String {
    let mut out = String::from("\"");
    for c in path.chars() {
        match c {
            '"' | '`' | '$' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            '%' => out.push_str("%%"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The program an Exec line runs, undoing `quote_exec`.
fn exec_program(exec: &str) -> String {
    let exec = exec.trim();
    let Some(rest) = exec.strip_prefix('"') else {
        return exec.split_whitespace().next().unwrap_or("").replace("%%", "%");
    };
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.extend(chars.next()),
            '"' => break,
            c => out.push(c),
        }
    }
    out.replace("%%", "%")
}

fn same_file(a: &Path, b: &Path) -> bool {
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    canon(a) == canon(b)
}

fn remove_if_there(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// Tells the desktop the files changed. The tools may be missing on some
/// systems; then the desktop catches up on its own, later.
fn refresh(apps: &Path, mime: &Path) {
    let _ = Command::new("update-desktop-database").arg(apps).status();
    if mime.join("packages").exists() {
        let _ = Command::new("update-mime-database").arg(mime).status();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_quoting_round_trips() {
        for path in ["/usr/bin/mder", "/home/a b/mder", "/odd/$x\"y`z\\w%v/mder"] {
            assert_eq!(exec_program(&format!("{} %f", quote_exec(path))), path);
        }
        assert_eq!(exec_program("/usr/bin/mder %f"), "/usr/bin/mder");
    }

    /// Registers and unregisters in a scratch data folder, never your own.
    #[test]
    fn register_and_unregister_in_a_scratch_folder() {
        let scratch = std::env::temp_dir().join(format!("mder-xdg-test-{}", std::process::id()));
        // Safety: this is the only test that touches the environment.
        unsafe { std::env::set_var("XDG_DATA_HOME", &scratch) };
        let md = &KINDS[0];
        let item = &items(md)[0];
        assert_eq!(status(md, item), Status::NotRegistered);

        apply(&[Change { kind: md, item, register: true }], None).unwrap();
        assert_eq!(status(md, item), Status::Registered);
        let entry = std::fs::read_to_string(scratch.join("applications").join(DESKTOP_FILE)).unwrap();
        assert!(entry.contains("MimeType=text/markdown;text/x-markdown;"), "{entry}");
        let package = std::fs::read_to_string(scratch.join("mime/packages").join(MIME_PACKAGE)).unwrap();
        assert!(package.contains("<glob pattern=\"*.mdwn\"/>"), "{package}");

        apply(&[Change { kind: md, item, register: false }], None).unwrap();
        assert_eq!(status(md, item), Status::NotRegistered);
        assert!(!scratch.join("applications").join(DESKTOP_FILE).exists());
        assert!(!scratch.join("mime/packages").join(MIME_PACKAGE).exists());
        let _ = std::fs::remove_dir_all(&scratch);
    }
}
