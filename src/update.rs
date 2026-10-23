//! Updates: reading the newest release's changelog, downloading this
//! system's file for it, and putting the new version where this one is.
//! The dialogs are in update_ui.rs.
//!
//! Releases come from GitHub, where `releases/latest` only ever means the
//! newest published release, one that passed its checks and has its files.
//! The `update_source` setting points mder at a folder of the same files
//! instead (changelog.json and the downloads, side by side), for trying an
//! update before anything's published.
//!
//! Installing renames the running program out of the way, which every
//! system allows, then unpacks the new one where it was. The renamed copy
//! is deleted the next time mder starts (`clean_up`).

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

/// This build's version: Cargo.toml's, or MDER_VERSION's (see build.rs).
pub const VERSION: &str = env!("MDER_VERSION");

const RELEASES: &str = "https://github.com/valiant8086/mder/releases";

/// Where to send people when mder can't update itself.
pub const DOWNLOAD_PAGE: &str = "https://github.com/valiant8086/mder/releases/latest";

/// One version in changelog.json.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub version: String,
    pub date: String,
    pub changes: Vec<String>,
}

/// changelog.json: a list, newest first, of `{version, date, blurb, changes}`.
pub fn parse_changelog(json: &str) -> Result<Vec<Entry>, String> {
    let value: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("the changelog isn't valid: {e}"))?;
    let list = value.as_array().ok_or("the changelog isn't a list")?;
    let text = |v: &serde_json::Value| v.as_str().unwrap_or_default().to_string();
    list.iter()
        .map(|item| {
            let version = item["version"].as_str().ok_or("a changelog entry has no version")?.to_string();
            let changes = item["changes"].as_array().map(|c| c.iter().map(text).collect()).unwrap_or_default();
            Ok(Entry { version, date: text(&item["date"]), changes })
        })
        .collect()
}

/// A version as numbers, for comparing: "0.10.2" is [0, 10, 2].
fn numbers(version: &str) -> Option<Vec<u64>> {
    version.trim().split('.').map(|part| part.parse().ok()).collect()
}

/// Whether version `a` is newer than `b`. "0.2" and "0.2.0" are the same.
pub fn is_newer(a: &str, b: &str) -> bool {
    let (Some(mut a), Some(mut b)) = (numbers(a), numbers(b)) else { return false };
    let len = a.len().max(b.len());
    a.resize(len, 0);
    b.resize(len, 0);
    a > b
}

fn same_version(a: &str, b: &str) -> bool {
    !is_newer(a, b) && !is_newer(b, a) && numbers(a).is_some() && numbers(b).is_some()
}

/// The release date of `version` in the changelog, if it's there.
pub fn release_date(entries: &[Entry], version: &str) -> Option<String> {
    entries.iter().find(|e| same_version(&e.version, version)).map(|e| e.date.clone()).filter(|d| !d.is_empty())
}

/// The newest version, if it's newer than `current`.
pub fn newer_than<'a>(entries: &'a [Entry], current: &str) -> Option<&'a Entry> {
    entries.iter().filter(|e| numbers(&e.version).is_some()).max_by(|a, b| {
        if is_newer(&a.version, &b.version) {
            std::cmp::Ordering::Greater
        } else if is_newer(&b.version, &a.version) {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Equal
        }
    })
    .filter(|e| is_newer(&e.version, current))
}

/// What changed since `current`, as the update dialog shows it.
pub fn changes_text(entries: &[Entry], current: &str, latest: &str) -> String {
    let mut text = format!("Changes between your version ({current}) and {latest}:\n\n");
    for entry in entries {
        if is_newer(&entry.version, current) {
            text.push_str(&format!("Version {}, released {}\n", entry.version, entry.date));
            for change in &entry.changes {
                text.push_str(&format!("- {change}\n"));
            }
            text.push('\n');
        } else if same_version(&entry.version, current) {
            text.push_str(&format!("You are running version {}, released {}\n", entry.version, entry.date));
            break;
        }
    }
    text
}

/// This system's download for `version`, named with the whole version as
/// scripts/dist.ps1, macos-app.sh and linux-pack.sh name it.
pub fn asset_name(version: &str) -> String {
    let chip = if cfg!(target_arch = "aarch64") { "arm64" } else { "x64" };
    if cfg!(windows) {
        format!("mder{version}-windows-{chip}.exe")
    } else if cfg!(target_os = "macos") {
        format!("mder{version}-mac-universal.zip")
    } else {
        format!("mder{version}-linux-{chip}.tar.gz")
    }
}

fn in_folder(folder: &str, name: &str) -> String {
    format!("{}/{name}", folder.trim_end_matches('/'))
}

fn changelog_url(source: Option<&str>) -> String {
    match source {
        Some(folder) => in_folder(folder, "changelog.json"),
        None => format!("{RELEASES}/latest/download/changelog.json"),
    }
}

fn download_url(source: Option<&str>, version: &str) -> String {
    let name = asset_name(version);
    match source {
        Some(folder) => in_folder(folder, &name),
        None => format!("{RELEASES}/download/v{version}/{name}"),
    }
}

/// Secure connections through the system's own support and certificates:
/// Windows' own, the Mac's own, and OpenSSL on Linux.
fn agent() -> ureq::Agent {
    use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
    let tls = TlsConfig::builder().provider(TlsProvider::NativeTls).root_certs(RootCerts::PlatformVerifier).build();
    ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_per_read(Some(Duration::from_secs(30)))
        .user_agent(format!("mder/{VERSION}"))
        .build()
        .into()
}

fn describe(e: ureq::Error) -> String {
    match e {
        ureq::Error::StatusCode(404) => "the file isn't on the update server (404)".to_string(),
        ureq::Error::StatusCode(code) => format!("the update server answered with error {code}"),
        e => e.to_string(),
    }
}

/// Reads the newest release's changelog. Blocks, so run it off the UI thread.
pub fn fetch_changelog(source: Option<&str>) -> Result<Vec<Entry>, String> {
    let mut response = agent().get(&changelog_url(source)).call().map_err(describe)?;
    let text = response.body_mut().read_to_string().map_err(describe)?;
    parse_changelog(&text)
}

/// Downloads this system's file for `version` into the temp folder, and
/// returns where it is, or None if `progress` said to stop. `progress` gets
/// the bytes so far and the total, if the server said, and returns whether
/// to carry on. Blocks, so run it off the UI thread.
pub fn download(
    source: Option<&str>,
    version: &str,
    mut progress: impl FnMut(u64, Option<u64>) -> bool,
) -> Result<Option<PathBuf>, String> {
    let dir = std::env::temp_dir().join("mder-update");
    std::fs::create_dir_all(&dir).map_err(|e| format!("couldn't make a folder for the download: {e}"))?;
    let path = dir.join(asset_name(version));
    let partial = dir.join(format!("{}.part", asset_name(version)));

    let response = agent().get(&download_url(source, version)).call().map_err(describe)?;
    let total = response.body().content_length();
    let mut reader = response.into_body().into_reader();
    let mut file = std::fs::File::create(&partial).map_err(|e| format!("couldn't save the download: {e}"))?;
    let mut buffer = vec![0; 64 * 1024];
    let mut done = 0;
    loop {
        if !progress(done, total) {
            drop(file);
            let _ = std::fs::remove_file(&partial);
            return Ok(None);
        }
        let n = reader.read(&mut buffer).map_err(|e| format!("the download stopped: {e}"))?;
        if n == 0 {
            break;
        }
        file.write_all(&buffer[..n]).map_err(|e| format!("couldn't save the download: {e}"))?;
        done += n as u64;
    }
    if total.is_some_and(|t| t != done) {
        return Err("the download stopped before the end".to_string());
    }
    drop(file);
    let _ = std::fs::remove_file(&path);
    std::fs::rename(&partial, &path).map_err(|e| format!("couldn't save the download: {e}"))?;
    Ok(Some(path))
}

/// This program, as it was when mder started. On Linux, once the program
/// is renamed, the system reports the new name, so it's noted at startup.
static EXE: OnceLock<PathBuf> = OnceLock::new();

/// Notes where this program is, and deletes what an update left behind.
/// Call once, first thing.
pub fn start() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = EXE.set(exe);
    }
    clean_up();
}

pub fn exe() -> Option<&'static Path> {
    EXE.get().map(PathBuf::as_path)
}

/// What an update replaces: the program's folder on Windows and Linux, and
/// mder.app on the Mac. None if mder isn't running as an app on the Mac.
pub fn install_location() -> Option<PathBuf> {
    let exe = exe()?;
    if cfg!(target_os = "macos") {
        let app = exe.ancestors().nth(3)?;
        app.extension().is_some_and(|e| e == "app").then(|| app.to_path_buf())
    } else {
        exe.parent().map(Path::to_path_buf)
    }
}

/// Whether mder can replace itself: it needs to make files beside itself.
pub fn can_replace_itself() -> bool {
    let Some(place) = install_location() else { return false };
    let folder = if cfg!(target_os = "macos") { place.parent().map(Path::to_path_buf) } else { Some(place) };
    let Some(folder) = folder else { return false };
    let probe = folder.join(format!(".mder-update-check-{}", std::process::id()));
    let ok = std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

/// Renames a replaced program (or mder.app) to its name plus ".old", such as
/// "mder.exe.old", or with a number if an older one is still in use by
/// another window. Returns the new name.
fn set_aside(path: &Path) -> Result<PathBuf, String> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    for n in 0..100 {
        let aside = path.with_file_name(if n == 0 { format!("{name}.old") } else { format!("{name}.old{n}") });
        if aside.is_dir() {
            let _ = std::fs::remove_dir_all(&aside);
        } else {
            let _ = std::fs::remove_file(&aside);
        }
        if !aside.exists() {
            std::fs::rename(path, &aside).map_err(|e| format!("couldn't move the old version aside: {e}"))?;
            return Ok(aside);
        }
    }
    Err("couldn't move the old version aside".to_string())
}

/// Deletes copies set aside by an update, unless another window still runs one.
fn clean_up() {
    let Some(place) = install_location() else { return };
    let (Some(folder), Some(name)) = (place.parent(), place.file_name()) else { return };
    // On Windows and Linux the program was set aside, not its folder.
    let (folder, name) = if cfg!(target_os = "macos") {
        (folder.to_path_buf(), name.to_string_lossy().into_owned())
    } else {
        let Some(exe) = exe() else { return };
        (place.clone(), exe.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
    };
    let Ok(entries) = std::fs::read_dir(&folder) else { return };
    let prefix = format!("{name}.old");
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().starts_with(&prefix) {
            let path = entry.path();
            let _ = if path.is_dir() { std::fs::remove_dir_all(&path) } else { std::fs::remove_file(&path) };
        }
    }
}

/// Puts the downloaded version in place of this one. On failure, this
/// version stays where it was.
pub fn install(downloaded: &Path) -> Result<(), String> {
    let place = install_location().ok_or("mder doesn't know where it's running from")?;
    platform_install(downloaded, &place)?;
    mark_updated();
    Ok(())
}

/// Windows: run the self-extractor over mder's folder.
#[cfg(windows)]
fn platform_install(downloaded: &Path, folder: &Path) -> Result<(), String> {
    let exe = exe().ok_or("mder doesn't know where it's running from")?;
    let aside = set_aside(exe)?;
    let result = Command::new(downloaded).arg("-y").arg(format!("-o{}", folder.display())).status();
    match result {
        Ok(status) if status.success() && exe.exists() => Ok(()),
        other => {
            if !exe.exists() {
                let _ = std::fs::rename(&aside, exe);
            }
            Err(match other {
                Ok(status) => format!("unpacking the new version failed ({status})"),
                Err(e) => format!("couldn't run the downloaded file: {e}"),
            })
        }
    }
}

/// The Mac: unpack the zip with ditto, then swap the new mder.app in.
#[cfg(target_os = "macos")]
fn platform_install(downloaded: &Path, app: &Path) -> Result<(), String> {
    let unpacked = fresh_folder("mac")?;
    run(Command::new("ditto").arg("-x").arg("-k").arg(downloaded).arg(&unpacked), "unpacking")?;
    let new_app = unpacked.join("mder.app");
    if !new_app.is_dir() {
        return Err("the download had no mder.app in it".to_string());
    }
    let aside = set_aside(app)?;
    if let Err(e) = run(Command::new("ditto").arg(&new_app).arg(app), "copying the new version") {
        let _ = std::fs::remove_dir_all(app);
        let _ = std::fs::rename(&aside, app);
        return Err(e);
    }
    let _ = std::fs::remove_dir_all(&unpacked);
    Ok(())
}

/// Linux: unpack the tar.gz, then put its files where these are.
#[cfg(all(unix, not(target_os = "macos")))]
fn platform_install(downloaded: &Path, folder: &Path) -> Result<(), String> {
    let exe = exe().ok_or("mder doesn't know where it's running from")?;
    let unpacked = fresh_folder("linux")?;
    run(Command::new("tar").arg("-xzf").arg(downloaded).arg("-C").arg(&unpacked), "unpacking")?;
    let new_files = unpacked.join("mder");
    let new_exe = new_files.join("mder");
    if !new_exe.is_file() {
        return Err("the download had no mder program in it".to_string());
    }
    let aside = set_aside(exe)?;
    if let Err(e) = std::fs::copy(&new_exe, exe) {
        let _ = std::fs::rename(&aside, exe);
        return Err(format!("couldn't put the new version in place: {e}"));
    }
    // The README and licenses are nice to have; the program is what matters.
    for name in ["README.md", "LICENSE-MIT.txt", "LICENSE-APACHE.txt", "NOTICE.txt"] {
        let _ = std::fs::copy(new_files.join(name), folder.join(name));
    }
    let _ = std::fs::remove_dir_all(&unpacked);
    Ok(())
}

#[cfg(unix)]
fn fresh_folder(name: &str) -> Result<PathBuf, String> {
    let folder = std::env::temp_dir().join("mder-update").join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).map_err(|e| format!("couldn't make a folder to unpack into: {e}"))?;
    Ok(folder)
}

#[cfg(unix)]
fn run(command: &mut Command, what: &str) -> Result<(), String> {
    match command.status() {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(format!("{what} failed ({status})")),
        Err(e) => Err(format!("{what} failed: {e}")),
    }
}

/// Starts the new version with `args`, such as the file this window had.
pub fn relaunch(args: &[std::ffi::OsString]) -> std::io::Result<()> {
    if cfg!(target_os = "macos") {
        let app = install_location().ok_or_else(|| std::io::Error::other("not running as mder.app"))?;
        Command::new("open").arg("-n").arg(app).arg("--args").args(args).spawn()?;
    } else {
        let exe = exe().ok_or_else(|| std::io::Error::other("mder doesn't know where it's running from"))?;
        Command::new(exe).args(args).spawn()?;
    }
    Ok(())
}

/// A note in the temp folder saying this copy of mder was just updated, so
/// its other windows restart on the new version (see `updated_since`).
fn marker() -> Option<PathBuf> {
    let place = install_location()?;
    let mut s = place.to_string_lossy().into_owned();
    if cfg!(windows) {
        s = s.to_lowercase();
    }
    // FNV-1a, as in instance.rs: the same in every mder process.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100000001b3);
    }
    Some(std::env::temp_dir().join(format!("mder-updated-{h:016x}")))
}

fn mark_updated() {
    if let Some(marker) = marker() {
        let _ = std::fs::write(marker, VERSION);
    }
}

/// Whether this copy of mder was updated after `since`, by another window.
pub fn updated_since(since: std::time::SystemTime) -> bool {
    marker()
        .and_then(|m| std::fs::metadata(m).ok())
        .and_then(|meta| meta.modified().ok())
        .is_some_and(|modified| modified > since)
}

/// The version another window last updated this copy of mder to.
pub fn updated_to() -> Option<String> {
    std::fs::read_to_string(marker()?).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHANGELOG: &str = r#"[
{"version":"0.3.0","date":"12/1/2026","blurb":"Third","changes":["Saving new files","Faster 'rendering'"]},
{"version":"0.2.1","date":"11/10/2026","blurb":"Fix","changes":["A fix"]},
{"version":"0.2.0","date":"11/1/2026","blurb":"Second","changes":["Updates"]},
{"version":"0.1.0","date":"10/21/2026","blurb":"First","changes":["Everything"]}]"#;

    #[test]
    fn compares_versions() {
        assert!(is_newer("0.2.0", "0.1.9"));
        assert!(is_newer("0.10.0", "0.9.0"));
        assert!(is_newer("1.0", "0.99.99"));
        assert!(!is_newer("0.2", "0.2.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("garbage", "0.1.0"));
    }

    #[test]
    fn reads_the_changelog() {
        let entries = parse_changelog(CHANGELOG).unwrap();
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].changes[1], "Faster 'rendering'");
        assert_eq!(newer_than(&entries, "0.2.0").unwrap().version, "0.3.0");
        assert_eq!(newer_than(&entries, "0.3.0"), None);
        assert_eq!(release_date(&entries, "0.2.1").as_deref(), Some("11/10/2026"));
        assert!(parse_changelog("{}").is_err());
    }

    #[test]
    fn lists_changes_since_your_version() {
        let entries = parse_changelog(CHANGELOG).unwrap();
        let text = changes_text(&entries, "0.2.0", "0.3.0");
        assert_eq!(
            text,
            "Changes between your version (0.2.0) and 0.3.0:\n\n\
             Version 0.3.0, released 12/1/2026\n- Saving new files\n- Faster 'rendering'\n\n\
             Version 0.2.1, released 11/10/2026\n- A fix\n\n\
             You are running version 0.2.0, released 11/1/2026\n"
        );
    }

    #[test]
    fn names_downloads_like_the_packing_scripts() {
        let name = asset_name("0.2.1");
        assert!(name.starts_with("mder0.2.1-"), "{name}");
        assert_eq!(
            download_url(None, "0.2.1"),
            format!("https://github.com/valiant8086/mder/releases/download/v0.2.1/{name}")
        );
        assert_eq!(download_url(Some("http://pc:8086/"), "0.2.1"), format!("http://pc:8086/{name}"));
        assert_eq!(changelog_url(Some("http://pc:8086")), "http://pc:8086/changelog.json");
    }
}
