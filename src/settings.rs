//! mder's settings, in `mder.ini`: one `key=value` per line.
//!
//! The file lives next to mder.exe, so mder stays portable. If that folder
//! can't be written to, it goes in `%APPDATA%\mder` instead (on macOS,
//! `~/Library/Application Support/mder`; elsewhere `$XDG_CONFIG_HOME/mder`
//! or `~/.config/mder`). Inside a macOS app bundle, it never goes next to the
//! program: that's part of the app, and writing there breaks its signature.

use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;

const FILE_NAME: &str = "mder.ini";

/// The editor or the rendered view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Editor,
    View,
}

impl Pane {
    pub fn as_str(self) -> &'static str {
        match self {
            Pane::Editor => "editor",
            Pane::View => "view",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "editor" => Some(Pane::Editor),
            "view" => Some(Pane::View),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// Where a file opens when no location is given.
    pub start_pane: Pane,
    /// Where a file opens when a command gives a location.
    pub location_pane: Pane,
    /// macOS: the app that opened each file type by default before mder
    /// took over, by type identifier, so unchecking can give it back.
    pub previous_defaults: BTreeMap<String, String>,
}

const PREVIOUS_DEFAULT: &str = "previous_default.";

impl Default for Settings {
    fn default() -> Self {
        Settings { start_pane: Pane::View, location_pane: Pane::View, previous_defaults: BTreeMap::new() }
    }
}

impl Settings {
    /// Reads the settings file, if there is one. Unknown or broken lines are
    /// skipped, so a hand edit can't stop mder starting.
    pub fn load() -> Self {
        match candidates().into_iter().find_map(|p| std::fs::read_to_string(p).ok()) {
            Some(text) => Settings::parse(&text),
            None => Settings::default(),
        }
    }

    fn parse(text: &str) -> Self {
        let mut settings = Settings::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else { continue };
            match key.trim() {
                "start_pane" => settings.start_pane = Pane::parse(value).unwrap_or(settings.start_pane),
                "location_pane" => settings.location_pane = Pane::parse(value).unwrap_or(settings.location_pane),
                key => {
                    if let Some(file_type) = key.strip_prefix(PREVIOUS_DEFAULT) {
                        settings.previous_defaults.insert(file_type.to_string(), value.trim().to_string());
                    }
                }
            }
        }
        settings
    }

    pub fn save(&self) -> io::Result<PathBuf> {
        let text = self.to_text();
        let mut last_error = io::Error::other("no place to save settings");
        for path in candidates() {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            match std::fs::write(&path, &text) {
                Ok(()) => return Ok(path),
                Err(e) => last_error = e,
            }
        }
        Err(last_error)
    }

    fn to_text(&self) -> String {
        let mut text = format!(
            "# mder settings. Change them in Options, or edit them here.\n\
             # Where a file opens with no location: view or editor.\n\
             start_pane={}\n\
             # Where a file opens when a command gives a location: view or editor.\n\
             location_pane={}\n",
            self.start_pane.as_str(),
            self.location_pane.as_str()
        );
        if !self.previous_defaults.is_empty() {
            text.push_str("# macOS: the app each file type goes back to when mder stops being its default.\n");
            for (file_type, app) in &self.previous_defaults {
                text.push_str(&format!("{PREVIOUS_DEFAULT}{file_type}={app}\n"));
            }
        }
        text
    }
}

/// Next to the exe first, then the per-user folder.
fn candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        let in_app_bundle = exe.to_string_lossy().contains(".app/Contents/MacOS/");
        if !in_app_bundle {
            out.push(exe.with_file_name(FILE_NAME));
        }
    }
    if let Some(dir) = user_config_dir() {
        out.push(dir.join("mder").join(FILE_NAME));
    }
    out
}

fn user_config_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library").join("Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panes_round_trip() {
        for pane in [Pane::Editor, Pane::View] {
            assert_eq!(Pane::parse(pane.as_str()), Some(pane));
        }
        assert_eq!(Pane::parse(" editor\r"), Some(Pane::Editor));
        assert_eq!(Pane::parse("sideways"), None);
    }

    #[test]
    fn round_trips() {
        let mut s = Settings { start_pane: Pane::Editor, ..Settings::default() };
        s.previous_defaults.insert("net.daringfireball.markdown".into(), "/System/Applications/TextEdit.app".into());
        s.previous_defaults.insert("dyn.ah62d4rv4ge80445e".into(), "/Applications/Some App.app".into());
        assert_eq!(Settings::parse(&s.to_text()), s);
        assert_eq!(Settings::parse(&Settings::default().to_text()), Settings::default());
    }

    #[test]
    fn defaults_to_the_rendered_view() {
        let s = Settings::default();
        assert_eq!(s.start_pane, Pane::View);
        assert_eq!(s.location_pane, Pane::View);
    }
}
