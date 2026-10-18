//! Tools, Options. Two tabs so far:
//! - General: which pane a file opens in.
//! - Integration: which file types offer mder in the system's Open with menu.

use std::cell::RefCell;
use std::rc::Rc;

use wxdragon::accessible::AccRole;
use wxdragon::prelude::*;

use crate::check_tree::{Branch, CheckTree};
use crate::integration::{self, Change, KINDS, Status};
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
    // Name each page after its tab, as a property page, so screen readers
    // say which page you're on, as they do in Windows' own dialogs.
    for (panel, name) in [(&general_page, "General"), (&page, "Integration")] {
        panel.set_accessibility_label(name);
        panel.set_accessibility_role(AccRole::PropertyPage);
    }
    sizer.add(&notebook, 1, SizerFlag::Expand | SizerFlag::All, 8);

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
    integration.close();
    dialog.destroy();
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
        let settings = Settings {
            start_pane: pane_at(self.start.get_selection()),
            location_pane: pane_at(self.at_location.get_selection()),
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

/// The Integration tab.
#[derive(Clone)]
struct IntegrationPage {
    dialog: Dialog,
    tree: Option<CheckTree>,
    /// What each box showed when last read from the system, per kind and
    /// extension. Only boxes that differ from this get applied, so a type
    /// registered to another copy isn't touched unless you change it.
    shown: Rc<RefCell<Vec<Vec<bool>>>>,
}

impl IntegrationPage {
    fn new(page: &Panel, dialog: Dialog) -> Self {
        let sizer = BoxSizer::builder(Orientation::Vertical).build();
        let mut this = IntegrationPage { dialog, tree: None, shown: Rc::default() };

        if !integration::supported() {
            let text = StaticText::builder(page)
                .with_label("Adding mder to the Open with menu isn't available on this system yet.")
                .build();
            sizer.add(&text, 0, SizerFlag::All, 8);
            page.set_sizer(sizer, true);
            return this;
        }

        let intro = StaticText::builder(page)
            .with_label("Offer mder in the Open with menu for the checked file types. This doesn't change which program opens them by default.")
            .build();
        intro.wrap(500);
        sizer.add(&intro, 0, SizerFlag::All, 8);

        let label = StaticText::builder(page).with_label("&File types:").build();
        sizer.add(&label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 8);
        let branches: Vec<Branch> = KINDS
            .iter()
            .map(|k| Branch { label: k.name.to_string(), leaves: k.extensions.iter().map(|e| e.to_string()).collect() })
            .collect();
        let tree = CheckTree::new(page, &branches);
        tree.add_to(&sizer, 1, SizerFlag::Expand | SizerFlag::All, 8);

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
        this.refresh();
        this
    }

    /// Reads what's registered, and shows it.
    fn refresh(&self) {
        let Some(tree) = &self.tree else { return };
        let mut shown = Vec::new();
        for (k, kind) in KINDS.iter().enumerate() {
            let mut row = Vec::new();
            for (e, ext) in kind.extensions.iter().enumerate() {
                let status = integration::status(kind, ext);
                let label = match &status {
                    Status::RegisteredElsewhere(path) => format!("{ext}, registered to another copy at {path}"),
                    _ => ext.to_string(),
                };
                tree.set_leaf_label(k, e, &label);
                let checked = status == Status::Registered;
                tree.set_checked(k, e, checked);
                row.push(checked);
            }
            shown.push(row);
        }
        *self.shown.borrow_mut() = shown;
    }

    /// Applies the boxes you changed. Returns false if that failed.
    fn apply(&self) -> bool {
        let Some(tree) = &self.tree else { return true };
        let shown = self.shown.borrow().clone();
        let mut changes = Vec::new();
        for (k, kind) in KINDS.iter().enumerate() {
            for (e, ext) in kind.extensions.iter().enumerate() {
                let checked = tree.is_checked(k, e);
                if checked != shown[k][e] {
                    changes.push(Change { kind, ext, register: checked });
                }
            }
        }
        let result = integration::apply(&changes);
        self.refresh();
        if let Err(e) = result {
            MessageDialog::builder(&self.dialog, &format!("Some changes couldn't be saved: {e}"), "Options")
                .with_style(MessageDialogStyle::OK | MessageDialogStyle::IconError)
                .build()
                .show_modal();
            return false;
        }
        true
    }

    fn close(&self) {
        if let Some(tree) = &self.tree {
            tree.close();
        }
    }
}
