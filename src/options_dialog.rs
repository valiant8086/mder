//! Tools, Options (Settings, in the app menu on macOS). Two tabs so far:
//! - General: which pane a file opens in.
//! - Integration: which file types offer mder in the system's Open with
//!   menu, or on macOS, which ones open in mder on double-click.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

#[cfg(windows)]
use wxdragon::accessible::AccRole;
use wxdragon::prelude::*;

use crate::check_tree::{Branch, CheckTree};
use crate::integration::{self, Change, Confirmation, Item, KINDS, Status};
use crate::settings::{Pane, Settings};

pub fn show(parent: &Frame) {
    let dialog = Dialog::builder(parent, "Options")
        .with_style(DialogStyle::DefaultDialogStyle | DialogStyle::ResizeBorder)
        .with_size(560, 480)
        .build();
    let sizer = BoxSizer::builder(Orientation::Vertical).build();

    let notebook = Notebook::builder(&dialog).build();
    #[cfg(windows)]
    tab_names::fix(notebook.get_handle());
    let general_page = Panel::builder(&notebook).build();
    let general = GeneralPage::new(&general_page, dialog);
    notebook.add_page(&general_page, "General", true, None);
    let page = Panel::builder(&notebook).build();
    let integration = IntegrationPage::new(&page, dialog);
    notebook.add_page(&page, "Integration", false, None);
    // Name each page after its tab, so screen readers say which page you're
    // on, as they do in Windows' own dialogs. The name works on Windows and
    // macOS (a no-op on GTK); setting a role is Windows-only in wxDragon.
    for (panel, name) in [(&general_page, "General"), (&page, "Integration")] {
        panel.set_accessibility_label(name);
        #[cfg(windows)]
        panel.set_accessibility_role(AccRole::PropertyPage);
    }
    sizer.add(&notebook, 1, SizerFlag::Expand | SizerFlag::All, 8);
    #[cfg(not(windows))]
    let tab_keys = switch_tabs_by_keyboard(&dialog, notebook);

    let ok = Button::builder(&dialog).with_id(ID_OK).with_label("OK").build();
    let cancel = Button::builder(&dialog).with_id(ID_CANCEL).with_label("Cancel").build();
    let apply = Button::builder(&dialog).with_id(ID_APPLY).with_label("&Apply").build();
    let buttons = StdDialogButtonSizerBuilder::new().build();
    buttons.add_button(&ok);
    buttons.add_button(&cancel);
    buttons.add_button(&apply);
    buttons.realize();
    sizer.add_sizer(&buttons, 0, SizerFlag::Expand | SizerFlag::All, 8);
    dialog.set_sizer(sizer, true);

    {
        let integration = integration.clone();
        ok.on_click(move |_| {
            // Apply both tabs, and only close if both worked.
            let saved = general.apply();
            if integration.apply() && saved {
                dialog.end_modal(ID_OK);
            }
        });
    }
    {
        let integration = integration.clone();
        apply.on_click(move |_| {
            general.apply();
            integration.apply();
        });
    }

    general.focus();
    dialog.show_modal();
    #[cfg(not(windows))]
    drop(tab_keys);
    integration.close();
    dialog.destroy();
}

/// What keeps the tab keys working until the dialog closes.
#[cfg(target_os = "macos")]
type TabKeys = Option<crate::macos::ControlTab>;
#[cfg(not(any(windows, target_os = "macos")))]
type TabKeys = ();

/// Control+Tab and Control+Shift+Tab move to the next and previous tab, from
/// anywhere in the dialog, then into that tab's first control. Windows' tab
/// control does this itself; macOS's doesn't. On the Mac, Command+Shift+]
/// and [ work too, as they do for tabs in Safari and Finder.
#[cfg(not(windows))]
fn switch_tabs_by_keyboard(dialog: &Dialog, notebook: Notebook) -> TabKeys {
    let show_tab = move |forward: bool| {
        notebook.advance_selection(forward);
        if let Some(page) = usize::try_from(notebook.selection()).ok().and_then(|i| notebook.get_page(i)) {
            page.set_focus();
        }
    };
    dialog.on_char_hook(move |event| {
        if let WindowEventData::Keyboard(ref key) = event {
            // macOS turns Control+Tab into focus movement before the dialog
            // sees it, so there it's caught earlier, below.
            #[cfg(not(target_os = "macos"))]
            let forward = match key.get_key_code() {
                Some(wxdragon::keycode::WXK_TAB) if key.control_down() && !key.alt_down() => Some(!key.shift_down()),
                _ => None,
            };
            // wxWidgets reports Command as "control" on macOS; cmd_down is Command.
            #[cfg(target_os = "macos")]
            let forward = match key.get_key_code() {
                Some(0x5D | 0x7D) if key.cmd_down() && key.shift_down() => Some(true), // ] or }
                Some(0x5B | 0x7B) if key.cmd_down() && key.shift_down() => Some(false), // [ or {
                _ => None,
            };
            if let Some(forward) = forward {
                show_tab(forward);
                return;
            }
        }
        event.skip(true);
    });
    #[cfg(target_os = "macos")]
    return crate::macos::ControlTab::new(show_tab);
}

/// To avoid flicker, wxWidgets gives its tab control a private copy of the
/// Windows class, called "_wx_SysTabCtl32". Windows' accessibility support
/// picks how to describe a control by its class name and doesn't recognise
/// that one, so screen readers hear tabs with no names. (wxWidgets'
/// wxFULL_REPAINT_ON_RESIZE should avoid the copy, but it checks the style
/// before storing it, so it never does.) The documented fix: answer the
/// screen reader with Windows' standard tab control description.
#[cfg(windows)]
mod tab_names {
    use std::ffi::c_void;

    #[repr(C)]
    struct Guid(u32, u16, u16, [u8; 8]);
    const IID_IACCESSIBLE: Guid = Guid(0x618736E0, 0x3C3D, 0x11CF, [0x81, 0x0C, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71]);
    const WM_GETOBJECT: u32 = 0x003D;
    const WM_NCDESTROY: u32 = 0x0082;
    const OBJID_CLIENT: i32 = -4;
    const SUBCLASS_ID: usize = 0x6D64_6572; // "mder"

    type SubclassProc = unsafe extern "system" fn(*mut c_void, u32, usize, isize, usize, usize) -> isize;

    #[link(name = "comctl32")]
    unsafe extern "system" {
        fn SetWindowSubclass(hwnd: *mut c_void, proc: SubclassProc, id: usize, data: usize) -> i32;
        fn RemoveWindowSubclass(hwnd: *mut c_void, proc: SubclassProc, id: usize) -> i32;
        fn DefSubclassProc(hwnd: *mut c_void, msg: u32, wparam: usize, lparam: isize) -> isize;
    }
    #[link(name = "oleacc")]
    unsafe extern "system" {
        fn CreateStdAccessibleProxyW(hwnd: *mut c_void, class: *const u16, id: i32, iid: *const Guid, out: *mut *mut c_void) -> i32;
        fn LresultFromObject(iid: *const Guid, wparam: usize, object: *mut c_void) -> isize;
    }

    pub fn fix(hwnd: *mut c_void) {
        unsafe {
            SetWindowSubclass(hwnd, proc, SUBCLASS_ID, 0);
        }
    }

    unsafe extern "system" fn proc(hwnd: *mut c_void, msg: u32, wparam: usize, lparam: isize, _id: usize, _data: usize) -> isize {
        unsafe {
            if msg == WM_GETOBJECT && lparam as i32 == OBJID_CLIENT {
                let class: Vec<u16> = "SysTabControl32\0".encode_utf16().collect();
                let mut object: *mut c_void = std::ptr::null_mut();
                if CreateStdAccessibleProxyW(hwnd, class.as_ptr(), OBJID_CLIENT, &IID_IACCESSIBLE, &mut object) >= 0 && !object.is_null() {
                    let result = LresultFromObject(&IID_IACCESSIBLE, wparam, object);
                    // LresultFromObject keeps its own reference; drop ours.
                    let vtable = *(object as *const *const [unsafe extern "system" fn(*mut c_void) -> u32; 3]);
                    ((*vtable)[2])(object);
                    return result;
                }
            }
            if msg == WM_NCDESTROY {
                RemoveWindowSubclass(hwnd, proc, SUBCLASS_ID);
            }
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
    }
}

/// The General tab.
#[derive(Clone, Copy)]
struct GeneralPage {
    dialog: Dialog,
    start: RadioBox,
    at_location: RadioBox,
}

/// The radio buttons' choices, in order.
const PANE_CHOICES: [&str; 2] = ["Rendered view", "Editor"];

fn pane_index(pane: Pane) -> i32 {
    match pane {
        Pane::View => 0,
        Pane::Editor => 1,
    }
}

fn pane_at(index: i32) -> Pane {
    if index == 1 { Pane::Editor } else { Pane::View }
}

impl GeneralPage {
    fn new(page: &Panel, dialog: Dialog) -> Self {
        let sizer = BoxSizer::builder(Orientation::Vertical).build();
        let start = RadioBox::builder(page, &PANE_CHOICES)
            .with_label("When opening a file, start in:")
            .build();
        sizer.add(&start, 0, SizerFlag::Expand | SizerFlag::All, 8);
        let at_location = RadioBox::builder(page, &PANE_CHOICES)
            .with_label("When a command opens a file at a location, start in:")
            .build();
        sizer.add(&at_location, 0, SizerFlag::Expand | SizerFlag::All, 8);
        let note = StaticText::builder(page)
            .with_label("--edit or --view on the command line overrides these for that launch. A file that's already open stays in the pane you're in.")
            .build();
        note.wrap(500);
        sizer.add(&note, 0, SizerFlag::All, 8);
        page.set_sizer(sizer, true);

        let settings = Settings::load();
        start.set_selection(pane_index(settings.start_pane));
        at_location.set_selection(pane_index(settings.location_pane));
        GeneralPage { dialog, start, at_location }
    }

    /// Saves the choices. Returns false if that failed.
    fn apply(&self) -> bool {
        // Start from what's saved, to keep what other tabs store there.
        let settings = Settings {
            start_pane: pane_at(self.start.get_selection()),
            location_pane: pane_at(self.at_location.get_selection()),
            ..Settings::load()
        };
        match settings.save() {
            Ok(_) => true,
            Err(e) => {
                MessageDialog::builder(&self.dialog, &format!("The settings couldn't be saved: {e}"), "Options")
                    .with_style(MessageDialogStyle::OK | MessageDialogStyle::IconError)
                    .build()
                    .show_modal();
                false
            }
        }
    }

    fn focus(&self) {
        self.start.set_focus();
    }
}

/// What the Integration tab says above its boxes.
#[cfg(target_os = "macos")]
const INTEGRATION_INTRO: &str = "Double-clicking a checked file type opens it in mder. Unchecking one gives it back to the app that opened it before, or asks which app to use. mder is always in Finder's Open With menu for these files.";
#[cfg(not(target_os = "macos"))]
const INTEGRATION_INTRO: &str = "Offer mder in the Open with menu for the checked file types. This doesn't change which program opens them by default.";

/// The Integration tab.
#[derive(Clone)]
struct IntegrationPage {
    dialog: Dialog,
    tree: Option<CheckTree>,
    /// The boxes, per kind, as the system groups file types now.
    items: Rc<Vec<Vec<Item>>>,
    /// What each box showed when last read from the system. Only boxes that
    /// differ from this get applied, so a type registered to another copy
    /// isn't touched unless you change it.
    shown: Rc<RefCell<Vec<Vec<bool>>>>,
    /// macOS: checks for its answer while it asks you to confirm a change.
    /// Dropped in `close`, before the dialog goes.
    watch: Rc<RefCell<Option<Timer<Dialog>>>>,
}

/// How often to look for macOS's answer.
const WATCH_MS: i32 = 250;

impl IntegrationPage {
    fn new(page: &Panel, dialog: Dialog) -> Self {
        let sizer = BoxSizer::builder(Orientation::Vertical).build();
        let mut this =
            IntegrationPage { dialog, tree: None, items: Rc::default(), shown: Rc::default(), watch: Rc::default() };

        if !integration::supported() {
            let message = if cfg!(target_os = "macos") {
                "Choosing which files open in mder only works when mder runs as mder.app."
            } else {
                "Adding mder to the Open with menu isn't available on this system yet."
            };
            let text = StaticText::builder(page).with_label(message).build();
            sizer.add(&text, 0, SizerFlag::All, 8);
            page.set_sizer(sizer, true);
            return this;
        }

        let intro = StaticText::builder(page).with_label(INTEGRATION_INTRO).build();
        intro.wrap(500);
        sizer.add(&intro, 0, SizerFlag::All, 8);

        let label = StaticText::builder(page).with_label("&File types:").build();
        sizer.add(&label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 8);
        let items: Vec<Vec<Item>> = KINDS.iter().map(integration::items).collect();
        let branches: Vec<Branch> = KINDS
            .iter()
            .zip(&items)
            .map(|(k, items)| Branch { label: k.name.to_string(), leaves: items.iter().map(|i| i.label.clone()).collect() })
            .collect();
        let tree = CheckTree::new(page, &branches);
        tree.add_to(&sizer, 1, SizerFlag::Expand | SizerFlag::All, 8);
        // VoiceOver says when a branch is expanded, but not when it's
        // collapsed, so nothing says the kinds have boxes inside them.
        #[cfg(target_os = "macos")]
        {
            let hint = StaticText::builder(page)
                .with_label("Expand a file type to choose its extensions one by one.")
                .build();
            sizer.add(&hint, 0, SizerFlag::Left | SizerFlag::Right, 8);
        }

        let row = BoxSizer::builder(Orientation::Horizontal).build();
        let select_all = Button::builder(page).with_label("&Select all").build();
        let clear_all = Button::builder(page).with_label("&Clear all").build();
        row.add(&select_all, 0, SizerFlag::Right, 8);
        row.add(&clear_all, 0, SizerFlag::Right, 8);
        sizer.add_sizer(&row, 0, SizerFlag::All, 8);
        page.set_sizer(sizer, true);

        {
            let tree = tree.clone();
            select_all.on_click(move |_| tree.set_all(true));
        }
        {
            let tree = tree.clone();
            clear_all.on_click(move |_| tree.set_all(false));
        }

        this.tree = Some(tree);
        this.items = Rc::new(items);
        this.refresh();
        this
    }

    /// Reads what's registered, and shows it.
    fn refresh(&self) {
        let Some(tree) = &self.tree else { return };
        let mut shown = Vec::new();
        for (k, (kind, items)) in KINDS.iter().zip(self.items.iter()).enumerate() {
            let mut row = Vec::new();
            for (i, item) in items.iter().enumerate() {
                let status = integration::status(kind, item);
                let label = match &status {
                    Status::RegisteredElsewhere(path) if cfg!(target_os = "macos") => {
                        format!("{}, opens in another copy of mder, at {path}", item.label)
                    }
                    Status::RegisteredElsewhere(path) => format!("{}, registered to another copy at {path}", item.label),
                    Status::OpensIn(app) => format!("{}, opens in {app}", item.label),
                    _ => item.label.clone(),
                };
                tree.set_leaf_label(k, i, &label);
                let checked = status == Status::Registered;
                tree.set_checked(k, i, checked);
                row.push(checked);
            }
            shown.push(row);
        }
        *self.shown.borrow_mut() = shown;
    }

    /// Applies the boxes you changed. Returns false if that failed, or you
    /// cancelled picking an app.
    fn apply(&self) -> bool {
        let Some(tree) = &self.tree else { return true };
        if integration::confirmation() == Confirmation::Waiting {
            self.tell(
                "macOS is still asking you to confirm the last change, in a dialog of its own. Answer that first.",
                MessageDialogStyle::IconInformation,
            );
            return false;
        }
        let shown = self.shown.borrow().clone();
        let mut changes = Vec::new();
        for (k, (kind, items)) in KINDS.iter().zip(self.items.iter()).enumerate() {
            for (i, item) in items.iter().enumerate() {
                let checked = tree.is_checked(k, i);
                if checked != shown[k][i] {
                    changes.push(Change { kind, item, register: checked });
                }
            }
        }
        let instead = if integration::needs_choice(&changes) {
            match self.choose_app(&changes) {
                Some(app) => Some(app),
                None => return false,
            }
        } else {
            None
        };
        if let Err(e) = integration::apply(&changes, instead.as_deref()) {
            self.refresh();
            self.tell(&format!("Some changes couldn't be saved: {e}"), MessageDialogStyle::IconError);
            return false;
        }
        match integration::confirmation() {
            // The boxes stay as you set them until macOS answers.
            Confirmation::Waiting => self.watch_for_answer(),
            answer => self.answered(answer),
        }
        true
    }

    fn watch_for_answer(&self) {
        let timer = Timer::new(&self.dialog);
        let page = self.clone();
        timer.on_tick(move |_| {
            let answer = integration::confirmation();
            if answer != Confirmation::Waiting {
                if let Some(timer) = page.watch.borrow().as_ref() {
                    timer.stop();
                }
                page.answered(answer);
            }
        });
        timer.start(WATCH_MS, false);
        *self.watch.borrow_mut() = Some(timer);
    }

    /// Shows what the system did with the changes.
    fn answered(&self, answer: Confirmation) {
        self.refresh();
        if answer == Confirmation::Refused {
            self.tell(
                "macOS kept the app you had for some of these file types.",
                MessageDialogStyle::IconInformation,
            );
        }
    }

    fn tell(&self, message: &str, icon: MessageDialogStyle) {
        MessageDialog::builder(&self.dialog, message, "Options")
            .with_style(MessageDialogStyle::OK | icon)
            .build()
            .show_modal();
    }

    /// macOS: asks which app should open the types you unchecked, when mder
    /// doesn't know which one did before. None if you cancel.
    fn choose_app(&self, changes: &[Change]) -> Option<PathBuf> {
        let apps = integration::choices(changes);
        if apps.is_empty() {
            MessageDialog::builder(
                &self.dialog,
                "No other app on this Mac can open these files, so mder has to stay their default for now.",
                "Options",
            )
            .with_style(MessageDialogStyle::OK | MessageDialogStyle::IconInformation)
            .build()
            .show_modal();
            return None;
        }
        let names: Vec<&str> = apps.iter().map(|(name, _)| name.as_str()).collect();
        let dialog = SingleChoiceDialog::builder(
            &self.dialog,
            "mder doesn't know which app opened these files before. Which app should open them now?",
            "Choose an app",
            &names,
        )
        .build();
        let chosen = (dialog.show_modal() == ID_OK).then(|| dialog.get_selection());
        dialog.destroy();
        let index = usize::try_from(chosen?).ok()?;
        apps.into_iter().nth(index).map(|(_, app)| app)
    }

    fn close(&self) {
        self.watch.borrow_mut().take();
        if let Some(tree) = &self.tree {
            tree.close();
        }
    }
}
