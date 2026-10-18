//! mder's settings, in `mder.ini`: one `key=value` per line.
//!
//! The file lives next to mder.exe, so mder stays portable. If that folder
//! can't be written to, it goes in `%APPDATA%\mder` instead (on other
//! systems, `$XDG_CONFIG_HOME/mder` or `~/.config/mder`).

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
}

impl Default for Settings {
    fn default() -> Self {
        Settings { start_pane: Pane::View, location_pane: Pane::View }
    }
}

impl Settings {
    /// Reads the settings file, if there is one. Unknown or broken lines are
    /// skipped, so a hand edit can't stop mder starting.
    pub fn load() -> Self {
        let mut settings = Settings::default();
        let Some(text) = candidates().into_iter().find_map(|p| std::fs::read_to_string(p).ok()) else {
            return settings;
        };
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else { continue };
            match key.trim() {
                "start_pane" => settings.start_pane = Pane::parse(value).unwrap_or(settings.start_pane),
                "location_pane" => settings.location_pane = Pane::parse(value).unwrap_or(settings.location_pane),
                _ => {}
            }
        }
        settings
    }

    pub fn save(&self) -> io::Result<PathBuf> {
        let text = format!(
            "# mder settings. Change them in Tools, Options, or edit them here.\n\
             # Where a file opens with no location: view or editor.\n\
             start_pane={}\n\
             # Where a file opens when a command gives a location: view or editor.\n\
             location_pane={}\n",
            self.start_pane.as_str(),
            self.location_pane.as_str()
        );
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
}

/// Next to the exe first, then the per-user folder.
fn candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        out.push(exe.with_file_name(FILE_NAME));
    }
    if let Some(dir) = user_config_dir() {
        out.push(dir.join("mder").join(FILE_NAME));
    }
    out
}

fn user_config_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
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
    fn defaults_to_the_rendered_view() {
        let s = Settings::default();
        assert_eq!(s.start_pane, Pane::View);
        assert_eq!(s.location_pane, Pane::View);
    }
}
