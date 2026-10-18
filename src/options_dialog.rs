//! Tools, Options. One tab so far, Integration: which file types offer mder
//! in the system's Open with menu.

use std::cell::RefCell;
use std::rc::Rc;

use wxdragon::prelude::*;

use crate::check_tree::{Branch, CheckTree};
use crate::integration::{self, Change, KINDS, Status};

pub fn show(parent: &Frame) {
    let dialog = Dialog::builder(parent, "Options")
        .with_style(DialogStyle::DefaultDialogStyle | DialogStyle::ResizeBorder)
        .with_size(560, 480)
        .build();
    let sizer = BoxSizer::builder(Orientation::Vertical).build();

    let notebook = Notebook::builder(&dialog).build();
    let page = Panel::builder(&notebook).build();
    let integration = IntegrationPage::new(&page, dialog);
    notebook.add_page(&page, "Integration", true, None);
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
            if integration.apply() {
                dialog.end_modal(ID_OK);
            }
        });
    }
    {
        let integration = integration.clone();
        apply.on_click(move |_| {
            integration.apply();
        });
    }

    integration.focus();
    dialog.show_modal();
    integration.close();
    dialog.destroy();
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

    fn focus(&self) {
        if let Some(tree) = &self.tree {
            tree.set_focus();
        }
    }

    fn close(&self) {
        if let Some(tree) = &self.tree {
            tree.close();
        }
    }
}
