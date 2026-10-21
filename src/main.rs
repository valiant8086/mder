#![windows_subsystem = "windows"]

mod args;
mod instance;
mod check_tree;
mod integration;
#[cfg(all(unix, not(target_os = "macos")))]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
mod options_dialog;
mod render;
mod settings;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};

use wxdragon::event::WebViewEvents;
use wxdragon::prelude::*;
use wxdragon::widgets::{WebView, WebViewBackend};

use args::Target;
use instance::Claim;
use settings::{Pane, Settings};

const ID_SAVE: i32 = 1001;
const ID_SWITCH: i32 = 2001;
// Standard IDs: on macOS, wxWidgets moves these into the app menu, as About
// mder, Quit mder, and Settings (Cmd+Comma). Elsewhere they stay where they are.
const ID_EXIT: i32 = wxdragon::id::ID_EXIT;
const ID_ABOUT: i32 = wxdragon::id::ID_ABOUT;
/// wxWidgets' wxID_PREFERENCES, which wxDragon doesn't name.
const ID_OPTIONS: i32 = 5022;

const ABOUT: &str = concat!(
    "mder ",
    env!("CARGO_PKG_VERSION"),
    "\n\nA markdown editor and renderer built for screen readers.\n\n",
    "Copyright 2026 valiant8086.\n",
    "Licensed under the MIT license or the Apache License 2.0, at your option.\n\n",
    "https://github.com/valiant8086/mder"
);

const PAGE_JS: &str = include_str!("page/page.js");
const PAGE_CSS: &str = include_str!("page/page.css");

/// How often to look at the file on disk for outside changes.
const DISK_POLL_MS: i32 = 1000;

/// What a window opened with no file shows.
const SAMPLE: &str = "# Welcome to mder

Press F6 to switch between this editor and the rendered view.

In the rendered view, press Enter on any paragraph, list item or table to edit that line here.

## A list

- First item
- Second item
  - Nested item
- Third item

## A table

| Key | Value |
|-----|-------|
| Alpha | 1 |
| Beta | 2 |

## Some code

```rust
fn main() {
    println!(\"hello\");
}
```

> A quote to finish.
";

/// Where `--log FILE` asked for a debug log. Set at startup, or when a later
/// launch with `--log` hands this window a target and it isn't logging yet.
static LOG_FILE: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

fn start_logging(path: &Path) {
    let mut log = LOG_FILE.lock().unwrap_or_else(|e| e.into_inner());
    if log.is_none() {
        *log = Some(path.to_path_buf());
    }
}

/// Appends to the `--log` file, if there is one.
fn debug_log(msg: &str) {
    use std::io::Write;
    let log = LOG_FILE.lock().unwrap_or_else(|e| e.into_inner());
    let Some(path) = log.as_ref() else { return };
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "[{}] {msg}", std::process::id());
    }
}

/// Modification time and size, to notice outside changes cheaply.
type Stamp = Option<(SystemTime, u64)>;

fn stamp_of(path: &Path) -> Stamp {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

/// The file's text with `\n` line endings, and whether it used `\r\n`.
fn read_text(path: &Path) -> std::io::Result<(String, bool)> {
    let bytes = std::fs::read(path)?;
    let text = String::from_utf8_lossy(&bytes);
    let crlf = text.contains("\r\n");
    Ok((text.replace("\r\n", "\n"), crlf))
}

struct Doc {
    path: Option<PathBuf>,
    /// The text as last loaded or saved, with `\n` line endings.
    saved: String,
    crlf: bool,
    stamp: Stamp,
}

impl Doc {
    fn open(path: Option<PathBuf>) -> Self {
        let Some(path) = path else {
            return Doc { path: None, saved: SAMPLE.to_string(), crlf: false, stamp: None };
        };
        let (saved, crlf) = read_text(&path).unwrap_or_default();
        let stamp = stamp_of(&path);
        Doc { path: Some(path), saved, crlf, stamp }
    }

    fn name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map_or("untitled".to_string(), |n| n.to_string_lossy().into_owned())
    }
}

struct App {
    frame: Frame,
    editor: TextCtrl,
    view: WebView,
    doc: Doc,
    last_switch: Instant,
    page_loaded: bool,
    /// Targets waiting for the page to load, or handed over by later launches.
    targets: Rc<RefCell<VecDeque<Target>>>,
    /// Keeps this process the owner of the file.
    _claim: Option<Claim>,
    /// Timers stop when dropped.
    timers: Vec<Timer<Frame>>,
    /// Whether the window is the active one, from its activate events.
    active: bool,
    /// A jump waits for the window to become active before moving NVDA.
    waiting_for_active: bool,
    /// Moves NVDA anyway if the activate event never comes.
    focus_timer: Option<Timer<Frame>>,
    /// Which pane had focus when the window last stopped being active.
    pane_when_inactive: Pane,
}

impl App {
    /// The editor's caret line, 1-based to match data-line.
    fn caret_line(&self) -> i64 {
        self.editor
            .position_to_xy(self.editor.get_insertion_point())
            .map_or(1, |(_, y)| y + 1)
    }

    fn set_caret_line(&self, line: i64) {
        let pos = self.editor.xy_to_position(0, (line - 1).max(0));
        if pos >= 0 {
            self.editor.set_insertion_point(pos);
        }
        debug_log(&format!(
            "caret to line {line}: pos {pos}, reads back {:?}, insertion {}",
            self.editor.position_to_xy(pos),
            self.editor.get_insertion_point()
        ));
    }

    fn go_to_render(&mut self) {
        self.focus_view();
        self.focus_render_block();
    }

    /// Puts keyboard focus in the rendered view. On GTK, wxWidgets wraps
    /// WebKit's view in a scrolled window and focuses that instead, so keys
    /// never reach the page, and Orca doesn't treat it as web content. So
    /// there, focus WebKit's own view, and only that: going through the
    /// wrapper first makes Orca say "scroll pane".
    fn focus_view(&self) {
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            #[link(name = "gtk-3")]
            unsafe extern "C" {
                fn gtk_widget_grab_focus(widget: *mut std::ffi::c_void);
            }
            let web_view = self.view.get_native_backend();
            if !web_view.is_null() {
                unsafe { gtk_widget_grab_focus(web_view) };
                return;
            }
        }
        self.view.set_focus();
    }

    /// Focuses the block for the caret's line inside the page. The page
    /// focuses an empty element first, so NVDA never sees the move as focus
    /// returning to the element it last saw, which it would ignore.
    /// See plans\focus-timing.md.
    fn focus_render_block(&self) {
        let line = self.caret_line();
        let result = self.view.run_script(&format!("mderFocusLine({line})"));
        debug_log(&format!("focus render line {line}: {result:?}"));
    }

    fn go_to_editor(&mut self, line: Option<i64>) {
        debug_log(&format!("to the editor, line {line:?}"));
        if let Some(line) = line {
            self.set_caret_line(line);
        }
        self.editor.set_focus();
    }

    /// F6 can arrive both from the page script and from the frame's key
    /// hook, so ignore a second request that comes right after the first.
    fn switch(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.last_switch) < Duration::from_millis(300) {
            return;
        }
        self.last_switch = now;
        if self.editor.has_focus() {
            self.go_to_render();
        } else {
            self.go_to_editor(None);
        }
    }

    /// F6 pressed in the rendered view. With Orca, the page's caret follows
    /// where you're reading, so the editor goes there, as Enter would. NVDA
    /// and VoiceOver keep their reading place to themselves, so elsewhere
    /// the editor stays where you left it.
    fn switch_from_view(&mut self, caret: Option<i64>) {
        #[cfg(all(unix, not(target_os = "macos")))]
        if let Some(line) = caret {
            if !self.editor.has_focus() && Instant::now().duration_since(self.last_switch) >= Duration::from_millis(300) {
                self.last_switch = Instant::now();
                self.go_to_editor(Some(line));
                return;
            }
        }
        let _ = caret;
        self.switch();
    }

    fn update_render(&self) {
        let html = render::to_html(&self.editor.get_value());
        self.view.run_script(&format!("mderUpdate({})", js_string(&html)));
    }

    fn save(&mut self) {
        let Some(path) = self.doc.path.clone() else { return };
        let text = self.editor.get_value();
        let on_disk = if self.doc.crlf { text.replace('\n', "\r\n") } else { text.clone() };
        match std::fs::write(&path, on_disk) {
            Ok(()) => {
                self.doc.saved = text;
                self.doc.stamp = stamp_of(&path);
                self.frame.set_status_text("Saved", 0);
            }
            Err(e) => self.frame.set_status_text(&format!("Save failed: {e}"), 0),
        }
    }

    /// Picks up changes made to the file by other programs.
    fn check_disk(&mut self) {
        let Some(path) = self.doc.path.clone() else { return };
        let stamp = stamp_of(&path);
        if stamp == self.doc.stamp || stamp.is_none() {
            return;
        }
        self.doc.stamp = stamp;
        let Ok((text, crlf)) = read_text(&path) else { return };
        if text == self.doc.saved {
            return;
        }
        let dirty = self.editor.get_value() != self.doc.saved;
        debug_log(&format!("file changed on disk, dirty={dirty}"));
        if dirty {
            let answer = MessageDialog::builder(
                &self.frame,
                &format!("{} changed on disk, and you have unsaved edits. Reload it and lose your edits?", self.doc.name()),
                "mder",
            )
            .with_style(MessageDialogStyle::YesNo | MessageDialogStyle::NoDefault | MessageDialogStyle::IconQuestion)
            .build()
            .show_modal();
            if answer != ID_YES {
                return;
            }
        }
        self.doc.saved = text.clone();
        self.doc.crlf = crlf;
        self.replace_text(&text);
        self.update_render();
        self.frame.set_status_text("Reloaded from disk", 0);
    }

    /// Replaces the editor's text, keeping the caret where it was.
    fn replace_text(&self, text: &str) {
        let (x, y) = self.editor.position_to_xy(self.editor.get_insertion_point()).unwrap_or((0, 0));
        self.editor.set_value(text);
        let mut pos = self.editor.xy_to_position(x, y);
        if pos < 0 {
            pos = self.editor.xy_to_position(0, y);
        }
        if pos < 0 {
            pos = self.editor.get_last_position();
        }
        self.editor.set_insertion_point(pos);
    }

    /// Acts on targets from the command line or from later launches.
    fn run_targets(&mut self) {
        if !self.page_loaded {
            return;
        }
        loop {
            let Some(target) = self.targets.borrow_mut().pop_front() else { break };
            self.go_to_target(&target);
        }
    }

    /// Opens a target in the right pane:
    /// - `--edit` or `--view` wins;
    /// - otherwise a fresh window follows the settings (one for a plain open,
    ///   one for an open at a location);
    /// - otherwise a window that already had the file stays in whichever
    ///   pane you were in, and moves to the location there.
    ///
    /// A location that isn't found still opens the pane; it just doesn't move.
    fn go_to_target(&mut self, target: &Target) {
        debug_log(&format!("target {target:?}"));
        // A later launch usually means the file just changed.
        self.check_disk();
        let has_location = !target.is_empty();
        let line = if has_location { self.target_line(target) } else { None };
        if has_location && line.is_none() {
            self.frame.set_status_text("That location wasn't found in the file", 0);
        }
        let current = self.current_pane();
        let pane = match target.pane {
            Some(pane) => pane,
            None if target.fresh => {
                let settings = Settings::load();
                if has_location { settings.location_pane } else { settings.start_pane }
            }
            None => current,
        };
        debug_log(&format!("pane {pane:?} (was {current:?}), line {line:?}"));
        if let Some(line) = line {
            self.set_caret_line(line as i64);
        }
        match pane {
            Pane::Editor => {
                self.bring_forward_then(|app| app.editor.set_focus());
            }
            Pane::View => {
                // Move the screen reader when there's somewhere to go, or
                // when this launch puts you in the view. Otherwise leave your
                // reading place alone.
                let move_reader = line.is_some() || target.fresh || current != Pane::View;
                self.update_render();
                self.last_switch = Instant::now();
                #[cfg(target_os = "macos")]
                self.bring_forward_then(|app| {
                    app.focus_view();
                    if move_reader {
                        app.focus_render_block();
                    }
                });
                #[cfg(not(target_os = "macos"))]
                self.bring_forward_then(|app| {
                    app.focus_view();
                    if move_reader {
                        app.move_reader_when_active();
                    }
                });
            }
        }
    }

    /// Brings the window forward and moves focus, in the order each screen
    /// reader needs. NVDA follows a move made once the window is active, so
    /// Windows comes forward first (and waits; see `move_reader_when_active`).
    /// VoiceOver puts its cursor back where it was when an app comes
    /// forward, unless focus has already moved, so macOS moves first.
    fn bring_forward_then(&mut self, move_focus: impl FnOnce(&mut Self)) {
        if cfg!(target_os = "macos") {
            move_focus(self);
            self.bring_forward();
        } else {
            self.bring_forward();
            move_focus(self);
        }
    }

    /// Moves the screen reader to the caret's block once Windows says the
    /// window is active, so NVDA sees the move live. This was the only
    /// approach that moved NVDA every time; see plans\focus-timing.md.
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    fn move_reader_when_active(&mut self) {
        if self.active {
            self.when_active();
        } else {
            self.waiting_for_active = true;
            if let Some(timer) = &self.focus_timer {
                timer.start(1500, true);
            }
        }
    }

    /// The pane you're in, or were in when the window was last active.
    fn current_pane(&self) -> Pane {
        if !self.active {
            self.pane_when_inactive
        } else if self.editor.has_focus() {
            Pane::Editor
        } else {
            Pane::View
        }
    }

    fn when_active(&mut self) {
        self.waiting_for_active = false;
        if let Some(timer) = &self.focus_timer {
            timer.stop();
        }
        self.focus_render_block();
    }

    /// Opens the menu bar, as Alt or F10 would, or with a letter, the menu it
    /// stands for, as Alt plus that letter would. For the rendered view,
    /// which keeps those keys to itself. Posted rather than sent, so the menu
    /// opens after this event and its commands can reach the app.
    fn open_menu(&self, letter: Option<char>) {
        #[cfg(windows)]
        {
            #[link(name = "user32")]
            unsafe extern "system" {
                fn PostMessageW(hwnd: *mut std::ffi::c_void, msg: u32, wparam: usize, lparam: isize) -> i32;
            }
            const WM_SYSCOMMAND: u32 = 0x0112;
            const SC_KEYMENU: usize = 0xF100;
            let key = letter.map_or(0, |c| c as isize);
            unsafe { PostMessageW(self.frame.get_handle(), WM_SYSCOMMAND, SC_KEYMENU, key) };
        }
        #[cfg(not(windows))]
        let _ = letter;
    }

    fn bring_forward(&self) {
        if self.frame.is_iconized() {
            self.frame.iconize(false);
        }
        self.frame.raise();
        // raise() only orders mder's own windows; on macOS the app has to
        // come in front of other apps too, and on Linux the window manager
        // has to be asked the way a taskbar would.
        #[cfg(target_os = "macos")]
        macos::activate();
        #[cfg(all(unix, not(target_os = "macos")))]
        if !linux::activate(self.frame.get_handle()) {
            debug_log("couldn't ask the window manager to bring the window forward (not X11)");
        }
        debug_log("brought the window forward");
    }

    fn target_line(&self, target: &Target) -> Option<usize> {
        let text = self.editor.get_value();
        if let Some(line) = target.line {
            return Some(line);
        }
        if let Some(anchor) = &target.anchor {
            return render::heading_line(&text, anchor);
        }
        let find = target.find.as_deref()?;
        let at = text.find(find).or_else(|| text.to_lowercase().find(&find.to_lowercase()))?;
        Some(render::LineIndex::new(&text).line_of(at))
    }
}

/// Runs `f` unless the app is already in use further up the stack. That
/// happens when a nested event loop (a dialog, or `run_script` waiting for
/// its result) delivers another event.
fn with_app(app: &Rc<RefCell<App>>, f: impl FnOnce(&mut App)) {
    if let Ok(mut app) = app.try_borrow_mut() {
        f(&mut app);
    }
}

/// The rendered view's page. It has no title: the window title already
/// names the file, and screen readers would announce it on every F6.
fn page(body: &str) -> String {
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><style>{PAGE_CSS}</style></head>\
         <body><div id=\"doc\">{body}</div><script>{PAGE_JS}</script></body></html>"
    )
}

/// A JavaScript string literal for `s`.
fn js_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Opening files from Finder on macOS. macOS doesn't put the file on the
/// command line: it sends the app an "open documents" event, which
/// wxWidgets passes on straight after startup, or whenever one arrives
/// while mder runs. A launch with no file gets a "new file" event instead.
/// Each file still gets its own mder, as on the other systems.
#[cfg(target_os = "macos")]
mod finder {
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicBool, Ordering};

    use wxdragon::prelude::*;

    use crate::args::Target;

    /// Whether this process has built its window.
    static HAS_WINDOW: AtomicBool = AtomicBool::new(false);

    pub fn listen(app: wxdragon::app::App, has_window: bool) {
        HAS_WINDOW.store(has_window, Ordering::SeqCst);
        app.on_open_files(|files| {
            crate::debug_log(&format!("macOS asked to open {files:?}"));
            let mut files: Vec<PathBuf> = files.into_iter().map(PathBuf::from).collect();
            // This process takes the first file if it has no window yet.
            let mine = if !files.is_empty() && !HAS_WINDOW.swap(true, Ordering::SeqCst) {
                Some(files.remove(0))
            } else {
                None
            };
            // Start the others first: opening a file that's already open
            // elsewhere ends this process.
            for file in &files {
                open_separately(file);
            }
            if let Some(file) = mine {
                crate::open_window(Some(file), Target::default());
            }
        });
        app.on_new_file(|| {
            if !HAS_WINDOW.swap(true, Ordering::SeqCst) {
                crate::open_window(None, Target::default());
            }
        });
    }

    /// Starts another mder on `file`. That one hands it on if the file is
    /// already open, as a second launch does anywhere.
    fn open_separately(file: &Path) {
        let Ok(exe) = std::env::current_exe() else { return };
        // The program is mder.app/Contents/MacOS/mder. Opening the app with
        // -n, rather than running the program, starts it as a proper app.
        let bundle = exe.ancestors().nth(3).filter(|b| b.extension().is_some_and(|e| e == "app"));
        let started = match bundle {
            Some(bundle) => Command::new("open").arg("-n").arg(bundle).arg("--args").arg(file).spawn(),
            None => Command::new(&exe).arg(file).spawn(),
        };
        match started {
            // Collect it when it exits, so it doesn't linger as a zombie.
            Ok(mut child) => drop(std::thread::spawn(move || child.wait())),
            Err(e) => crate::debug_log(&format!("couldn't start mder for {}: {e}", file.display())),
        }
    }
}

/// An update renames the running exe to `mder.exe.old` before putting the
/// new one in place. Delete it once nothing is running it any more.
fn remove_replaced_exe() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::remove_file(exe.with_extension("exe.old"));
    }
}

/// Runs `--register` or `--unregister`, reports the result in a message box, and exits.
fn run_command(command: args::Command) -> ! {
    let result = match command {
        args::Command::Register => integration::register_all(),
        args::Command::Unregister => integration::unregister_all(),
    };
    let (message, style, code) = match result {
        Ok(message) => (message, MessageDialogStyle::IconInformation, 0),
        Err(e) => (format!("That didn't work: {e}"), MessageDialogStyle::IconError, 1),
    };
    let owner = Frame::builder().with_title("mder").build();
    MessageDialog::builder(&owner, &message, "mder")
        .with_style(MessageDialogStyle::OK | style)
        .build()
        .show_modal();
    std::process::exit(code);
}

fn main() {
    remove_replaced_exe();
    let launch = args::parse(std::env::args_os().skip(1));
    if let Some(log) = &launch.target.log {
        start_logging(log);
        debug_log(&format!("started: {:?} {:?}", launch.path, launch.target));
    }

    let _ = wxdragon::main(move |app| {
        if let Some(command) = launch.command {
            run_command(command);
        }
        #[cfg(target_os = "macos")]
        {
            macos::disable_window_tabbing();
            finder::listen(app, launch.path.is_some());
            if launch.path.is_none() {
                // macOS says straight after this which file to open, if any.
                return;
            }
        }
        #[cfg(not(target_os = "macos"))]
        let _ = app;
        open_window(launch.path, launch.target);
    });
}

/// Builds this process's window, on `path` or on no file. If another mder
/// already has the file open, hands `target` to it and exits instead.
fn open_window(path: Option<PathBuf>, target: Target) {
    let targets: Rc<RefCell<VecDeque<Target>>> = Rc::default();

    let claim = match &path {
        Some(path) => {
            let queue = targets.clone();
            let on_target = move |t: Target| {
                if let Some(log) = &t.log {
                    start_logging(log);
                }
                debug_log(&format!("handed {t:?}"));
                queue.borrow_mut().push_back(t);
                wxdragon::app::wake_up_idle();
            };
            match instance::claim(path, &target, on_target) {
                Some(Claim::HandedOff) => {
                    debug_log("the file is already open; handed the target to that window");
                    std::process::exit(0)
                }
                Some(Claim::Unreachable) => {
                    debug_log("the file is already open, but that window didn't respond");
                    std::process::exit(1)
                }
                other => other,
            }
        }
        None => None,
    };
    // Opening a file always goes through go_to_target, which picks the
    // pane from the settings. With no file, mder starts in the editor.
    if path.is_some() {
        let mut target = target.clone();
        target.fresh = true;
        targets.borrow_mut().push_back(target);
    }

    let doc = Doc::open(path.clone());
    let frame = Frame::builder()
        .with_title(&format!("{} - mder", doc.name()))
        .with_size(Size::new(1200, 800))
        .build();
    wxdragon::app::set_top_window(&frame);

    // Ctrl+Comma for Options everywhere; on macOS, wxWidgets makes it
    // Command+Comma and moves the item into the app menu as Settings.
    const OPTIONS_LABEL: &str = "&Options...\tCtrl+,";
    const OPTIONS_HELP: &str = "Settings, such as which file types offer mder in Open with";
    let file_menu = Menu::builder()
        .append_item(ID_SAVE, "&Save\tCtrl+S", "Save the file")
        .append_separator()
        .append_item(ID_EXIT, "E&xit", "Close mder")
        .build();
    let view_menu = Menu::builder()
        .append_item(ID_SWITCH, "&Switch pane\tF6", "Move between the editor and the rendered view")
        .build();
    let help_menu = Menu::builder()
        .append_item(ID_ABOUT, "&About mder", "Version, credits and license")
        .build();
    // On macOS, Options goes in a menu only for wxWidgets to move it to
    // the app menu; a Tools menu would be left empty, so there isn't one.
    #[cfg(target_os = "macos")]
    file_menu.append(ID_OPTIONS, OPTIONS_LABEL, OPTIONS_HELP, ItemKind::Normal);
    let menu_bar = MenuBar::builder().append(file_menu, "&File").append(view_menu, "&View");
    #[cfg(not(target_os = "macos"))]
    let menu_bar = menu_bar.append(Menu::builder().append_item(ID_OPTIONS, OPTIONS_LABEL, OPTIONS_HELP).build(), "&Tools");
    frame.set_menu_bar(menu_bar.append(help_menu, "&Help").build());
    frame.create_status_bar(1, 0, -1, "status");

    let splitter = SplitterWindow::builder(&frame)
        .with_style(SplitterWindowStyle::LiveUpdate)
        .build();
    let editor = TextCtrl::builder(&splitter)
        .with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::DontWrap | TextCtrlStyle::ProcessTab)
        .build();
    let backend = if WebView::is_backend_available(WebViewBackend::Edge) {
        WebViewBackend::Edge
    } else {
        WebViewBackend::Default
    };
    let view = WebView::builder(&splitter).with_backend(backend).build();
    view.enable_context_menu(false);
    view.add_script_message_handler("mder");
    splitter.split_vertically(&editor, &view, 560);
    splitter.set_minimum_pane_size(100);

    editor.set_value(&doc.saved);
    editor.set_insertion_point(0);
    view.set_page(&page(&render::to_html(&doc.saved)), "about:blank");

    let app = Rc::new(RefCell::new(App {
        frame,
        editor,
        view,
        doc,
        last_switch: Instant::now() - Duration::from_secs(1),
        page_loaded: false,
        targets,
        _claim: claim,
        timers: Vec::new(),
        active: true,
        waiting_for_active: false,
        focus_timer: None,
        pane_when_inactive: Pane::Editor,
    }));

    // Re-render shortly after typing stops.
    let render_timer = Rc::new(Timer::new(&frame));
    {
        let app = app.clone();
        render_timer.on_tick(move |_| with_app(&app, |app| app.update_render()));
    }
    {
        let render_timer = render_timer.clone();
        editor.on_text_updated(move |_| {
            render_timer.start(300, true);
        });
    }

    // Watch the file, and catch any handed-over targets the idle event missed.
    let disk_timer = Timer::new(&frame);
    {
        let app = app.clone();
        disk_timer.on_tick(move |_| {
            with_app(&app, |app| {
                app.check_disk();
                app.run_targets();
            })
        });
    }
    disk_timer.start(DISK_POLL_MS, false);
    app.borrow_mut().timers.push(disk_timer);

    // A jump moves NVDA once the window is active; this covers the case
    // where the activate event never comes.
    let focus_timer = Timer::new(&frame);
    {
        let app = app.clone();
        focus_timer.on_tick(move |_| {
            with_app(&app, |app| {
                if app.waiting_for_active {
                    app.when_active();
                }
            })
        });
    }
    app.borrow_mut().focus_timer = Some(focus_timer);
    {
        let app = app.clone();
        frame.on_activate(move |event| {
            if let WindowEventData::Activate(ref a) = event {
                let active = a.is_active();
                with_app(&app, |app| {
                    if !active {
                        // Focus is still where you left it at this point.
                        app.pane_when_inactive = if app.editor.has_focus() { Pane::Editor } else { Pane::View };
                    }
                    app.active = active;
                    debug_log(&format!("window active: {active}"));
                    if active && app.waiting_for_active {
                        app.when_active();
                    }
                });
            }
            // wxWidgets restores focus to the last focused control.
            event.skip(true);
        });
    }

    {
        let app = app.clone();
        frame.on_idle(move |event| {
            with_app(&app, |app| app.run_targets());
            event.skip(true);
        });
    }

    {
        let app = app.clone();
        view.on_loaded(move |_| {
            with_app(&app, |app| {
                app.page_loaded = true;
                let focused = app.view.run_script(
                    "document.activeElement && document.activeElement !== document.body ? document.activeElement.textContent.slice(0, 40) : 'nothing'",
                );
                debug_log(&format!("page loaded, focused: {focused:?}"));
                app.run_targets();
            })
        });
    }

    {
        let app = app.clone();
        view.on_script_message_received(move |e| {
            let msg = e.get_string().unwrap_or_default();
            if msg == "options" {
                // Outside with_app: the dialog's own event loop runs
                // mder's timers, which need the app.
                let frame = app.borrow().frame;
                options_dialog::show(&frame);
                return;
            }
            with_app(&app, |app| {
                if let Some(caret) = msg.strip_prefix("switch:") {
                    // F6 in the rendered view, with the line the page's caret
                    // is on, if any.
                    let caret: Option<i64> = caret.parse().ok();
                    debug_log(&format!("F6 in the rendered view; the page's caret is on line {caret:?}"));
                    app.switch_from_view(caret);
                } else if msg == "save" {
                    app.save();
                } else if let Some(key) = msg.strip_prefix("menu:") {
                    app.open_menu(key.chars().next());
                } else if let Some(line) = msg.strip_prefix("goto:").and_then(|l| l.parse().ok()) {
                    app.go_to_editor(Some(line));
                }
            });
        });
    }

    {
        let app = app.clone();
        frame.on_char_hook(move |event| {
            if let WindowEventData::Keyboard(ref key) = event {
                if key.get_key_code() == Some(wxdragon::keycode::WXK_F6)
                    && !key.control_down()
                    && !key.alt_down()
                {
                    with_app(&app, |app| app.switch());
                    return;
                }
            }
            event.skip(true);
        });
    }

    {
        let app = app.clone();
        frame.on_menu(move |e| match e.get_id() {
            ID_SAVE => with_app(&app, |app| app.save()),
            ID_EXIT => with_app(&app, |app| app.frame.close(true)),
            ID_SWITCH => with_app(&app, |app| app.switch()),
            ID_OPTIONS => {
                // Not inside with_app: the dialog's own event loop runs
                // mder's timers, which need the app.
                let frame = app.borrow().frame;
                options_dialog::show(&frame);
            }
            ID_ABOUT => with_app(&app, |app| {
                MessageDialog::builder(&app.frame, ABOUT, "About mder")
                    .with_style(MessageDialogStyle::OK | MessageDialogStyle::IconInformation)
                    .build()
                    .show_modal();
            }),
            _ => e.skip(true),
        });
    }

    // On Linux, closing a menu gives focus back to the web view, but not to
    // anything in the page, which leaves Orca outside the document.
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let app = app.clone();
        frame.on_menu_closed(move |event| {
            with_app(&app, |app| {
                if !app.editor.has_focus() {
                    let result = app.view.run_script("mderRestoreReader()");
                    debug_log(&format!("menu closed over the rendered view: {result:?}"));
                }
            });
            event.skip(true);
        });
    }

    frame.show(true);
    frame.centre();
    editor.set_focus();
}
