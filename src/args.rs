//! Command line: `mder [--edit | --view] [--line N] [--find TEXT] [--anchor SLUG] [--log FILE] [FILE[:LINE][#ANCHOR]]`,
//! or `mder --register` / `mder --unregister`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::settings::Pane;

/// Where to put the user once the file is open.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Target {
    pub line: Option<usize>,
    pub anchor: Option<String>,
    pub find: Option<String>,
    /// `--edit` or `--view`: open in that pane, whatever the settings say.
    pub pane: Option<Pane>,
    /// Set for the launch that opens the window, as opposed to a later
    /// launch handing a target to it. Not passed between processes.
    pub fresh: bool,
    /// Write a debug log to this file. It travels with the target so that a
    /// window that already has the file can start logging too.
    pub log: Option<PathBuf>,
    /// Linux on Wayland: the token this launch was given for bringing a
    /// window forward. A window that already has the file needs it to come
    /// forward, since Wayland won't let a window just take the front.
    pub activation_token: Option<String>,
}

impl Target {
    pub fn is_empty(&self) -> bool {
        self.line.is_none() && self.anchor.is_none() && self.find.is_none()
    }

    /// One `key=value` per line, for handing to another instance.
    pub fn encode(&self) -> String {
        let mut out = String::new();
        if let Some(l) = self.line {
            out.push_str(&format!("line={l}\n"));
        }
        if let Some(a) = &self.anchor {
            out.push_str(&format!("anchor={}\n", escape(a)));
        }
        if let Some(f) = &self.find {
            out.push_str(&format!("find={}\n", escape(f)));
        }
        if let Some(pane) = self.pane {
            out.push_str(&format!("pane={}\n", pane.as_str()));
        }
        if let Some(log) = &self.log {
            out.push_str(&format!("log={}\n", escape(&log.to_string_lossy())));
        }
        if let Some(token) = &self.activation_token {
            out.push_str(&format!("activation={}\n", escape(token)));
        }
        out
    }

    pub fn decode(s: &str) -> Self {
        let mut t = Target::default();
        for line in s.lines() {
            let Some((key, value)) = line.split_once('=') else { continue };
            match key {
                "line" => t.line = value.parse().ok(),
                "anchor" => t.anchor = Some(unescape(value)),
                "find" => t.find = Some(unescape(value)),
                "pane" => t.pane = Pane::parse(value),
                "log" => t.log = Some(PathBuf::from(unescape(value))),
                "activation" => t.activation_token = Some(unescape(value)),
                _ => {}
            }
        }
        t
    }
}

#[derive(Debug, Default)]
pub struct Launch {
    pub path: Option<PathBuf>,
    pub target: Target,
    /// Run this instead of opening a window.
    pub command: Option<Command>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Command {
    /// Register mder for every file type it opens, as Options' Integration
    /// tab would with every box checked (see integration/mod.rs).
    Register,
    Unregister,
}

/// Arguments arrive as OS strings: a file name doesn't have to be valid Unicode.
pub fn parse(args: impl IntoIterator<Item = OsString>) -> Launch {
    let mut launch = Launch::default();
    let mut file: Option<OsString> = None;
    let mut args = args.into_iter();
    fn next_text(args: &mut impl Iterator<Item = OsString>) -> Option<String> {
        args.next().map(|v| v.to_string_lossy().into_owned())
    }
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--edit") => launch.target.pane = Some(Pane::Editor),
            Some("--view") => launch.target.pane = Some(Pane::View),
            Some("--register") => launch.command = Some(Command::Register),
            Some("--unregister") => launch.command = Some(Command::Unregister),
            Some("--line") => launch.target.line = next_text(&mut args).and_then(|v| v.parse().ok()),
            Some("--find") => launch.target.find = next_text(&mut args),
            Some("--anchor") => launch.target.anchor = next_text(&mut args),
            Some("--log") => {
                // Made absolute now: a window that's already open may be
                // running in a different folder.
                launch.target.log = args.next().map(|p| std::path::absolute(&p).unwrap_or_else(|_| p.into()));
            }
            _ => file = Some(arg),
        }
    }
    if let Some(file) = file {
        let (path, line, anchor) = match file.to_str() {
            Some(s) => split_location(s),
            None => (PathBuf::from(file), None, None),
        };
        launch.path = Some(path);
        launch.target.line = launch.target.line.or(line);
        launch.target.anchor = launch.target.anchor.take().or(anchor);
    }
    launch
}

/// Splits `notes.md:42` or `notes.md#setup`, unless a file by the full name exists.
fn split_location(arg: &str) -> (PathBuf, Option<usize>, Option<String>) {
    if Path::new(arg).exists() {
        return (arg.into(), None, None);
    }
    if let Some((path, anchor)) = arg.rsplit_once('#') {
        if !anchor.is_empty() && !path.is_empty() {
            return (path.into(), None, Some(anchor.to_string()));
        }
    }
    if let Some((path, line)) = arg.rsplit_once(':') {
        // Leave drive letters like C: alone.
        if let Ok(n) = line.parse() {
            if path.len() > 1 {
                return (path.into(), Some(n), None);
            }
        }
    }
    (arg.into(), None, None)
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\n', "\\n").replace('\r', "")
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some(other) => out.push(other),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_str(s: &str) -> Launch {
        parse(s.split_whitespace().map(OsString::from))
    }

    #[test]
    fn register_command() {
        assert_eq!(parse_str("--register").command, Some(Command::Register));
        assert_eq!(parse_str("--unregister").command, Some(Command::Unregister));
        assert_eq!(parse_str("notes.md").command, None);
    }

    #[test]
    fn line_suffix_and_drive_letter() {
        let l = parse_str(r"C:\nowhere\notes.md:42");
        assert_eq!(l.path, Some(PathBuf::from(r"C:\nowhere\notes.md")));
        assert_eq!(l.target.line, Some(42));
    }

    #[test]
    fn anchor_suffix() {
        let l = parse_str(r"C:\nowhere\notes.md#setup-steps");
        assert_eq!(l.path, Some(PathBuf::from(r"C:\nowhere\notes.md")));
        assert_eq!(l.target.anchor.as_deref(), Some("setup-steps"));
    }

    #[test]
    fn flags() {
        let l = parse_str("--edit --line 7 notes.md");
        assert_eq!(l.target.pane, Some(Pane::Editor));
        assert_eq!(l.target.line, Some(7));
        assert_eq!(l.path, Some(PathBuf::from("notes.md")));
        assert_eq!(parse_str("--view notes.md").target.pane, Some(Pane::View));
        assert_eq!(parse_str("notes.md").target.pane, None);
    }

    #[test]
    fn target_round_trips() {
        let t = Target {
            line: Some(3),
            anchor: None,
            find: Some("a \\ b\nc".into()),
            pane: Some(Pane::Editor),
            fresh: false,
            log: Some(PathBuf::from(r"C:\logs\mder.log")),
            activation_token: Some("a1b2_c3".into()),
        };
        assert_eq!(Target::decode(&t.encode()), t);
    }

    #[test]
    fn log_path_is_made_absolute() {
        let l = parse_str("--log mder.log notes.md");
        let log = l.target.log.clone().unwrap();
        assert!(log.is_absolute(), "{log:?}");
        assert!(log.ends_with("mder.log"));
        assert!(l.target.is_empty(), "a log alone isn't a location");
    }
}
