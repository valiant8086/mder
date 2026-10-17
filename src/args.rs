//! Command line: `mder [--edit] [--line N] [--find TEXT] [FILE[:LINE][#ANCHOR]]`.

use std::path::{Path, PathBuf};

/// Where to put the user once the file is open.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Target {
    pub line: Option<usize>,
    pub anchor: Option<String>,
    pub find: Option<String>,
    /// Land in the editor instead of the rendered view.
    pub edit: bool,
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
        if self.edit {
            out.push_str("edit=1\n");
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
                "edit" => t.edit = value == "1",
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
}

pub fn parse(args: impl IntoIterator<Item = String>) -> Launch {
    let mut launch = Launch::default();
    let mut file: Option<String> = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--edit" => launch.target.edit = true,
            "--line" => launch.target.line = args.next().and_then(|v| v.parse().ok()),
            "--find" => launch.target.find = args.next(),
            "--anchor" => launch.target.anchor = args.next(),
            _ => file = Some(arg),
        }
    }
    if let Some(file) = file {
        let (path, line, anchor) = split_location(&file);
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
        parse(s.split_whitespace().map(String::from))
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
        assert!(l.target.edit);
        assert_eq!(l.target.line, Some(7));
        assert_eq!(l.path, Some(PathBuf::from("notes.md")));
    }

    #[test]
    fn target_round_trips() {
        let t = Target { line: Some(3), anchor: None, find: Some("a \\ b\nc".into()), edit: true };
        assert_eq!(Target::decode(&t.encode()), t);
    }
}
