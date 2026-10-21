//! A two-level checkable tree: branches, each with checkable leaves.
//! Checking a branch checks all its leaves, and a branch shows half checked
//! when only some of its leaves are checked.
//!
//! Windows gets the native tree view with its own checkboxes, which NVDA reads
//! best. The Mac gets wxTreeListCtrl, which is the system's own tree there.
//! See checkable-tree-findings.md in C:\code\wxdragon UI experiments. On
//! Linux, GTK doesn't pass wxTreeListCtrl's labels to screen readers, so Orca
//! hears nameless rows; there it's plain checkboxes instead.

#[cfg(windows)]
pub use native::CheckTree;
#[cfg(target_os = "macos")]
pub use generic::CheckTree;
#[cfg(all(unix, not(target_os = "macos")))]
pub use flat::CheckTree;

/// One branch and its leaves' labels.
pub struct Branch {
    pub label: String,
    pub leaves: Vec<String>,
}

#[cfg(windows)]
mod native {
    use std::cell::RefCell;
    use std::ffi::c_void;
    use std::rc::Rc;

    use wxdragon::prelude::*;
    use wxdragon::widgets::treectrl::TreeItemId;

    use super::Branch;

    const UNCHECKED: u32 = 1;
    const CHECKED: u32 = 2;
    const PARTIAL: u32 = 3;

    /// How often to look for checkbox changes. The tree view doesn't tell
    /// wxWidgets when a checkbox changes.
    const POLL_MS: i32 = 150;

    #[derive(Clone)]
    pub struct CheckTree {
        tree: TreeCtrl,
        inner: Rc<RefCell<Inner>>,
    }

    struct Inner {
        hwnd: *mut c_void,
        /// Native items and their wx ids, parents before children.
        items: Vec<isize>,
        ids: Vec<TreeItemId>,
        /// For each item, the index of its branch, or None for a branch.
        parents: Vec<Option<usize>>,
        /// Where each branch's item is.
        branch_at: Vec<usize>,
        last: Vec<u32>,
        /// Must be dropped before the tree is destroyed; see `close`.
        timer: Option<Timer<TreeCtrl>>,
    }

    impl CheckTree {
        pub fn new(parent: &dyn WxWidget, branches: &[Branch]) -> Self {
            let tree = TreeCtrl::builder(parent)
                .with_style(TreeCtrlStyle::Default | TreeCtrlStyle::HideRoot)
                .build();
            let hwnd = tree.get_handle();
            // Microsoft: set the checkbox style after creating the tree and
            // before adding any items.
            win::enable_checkboxes(hwnd);

            let mut ids = Vec::new();
            let mut parents = Vec::new();
            let mut branch_at = Vec::new();
            if let Some(root) = tree.add_root("", None, None) {
                for branch in branches {
                    let Some(id) = tree.append_item(&root, &branch.label, None, None) else { continue };
                    let b = ids.len();
                    branch_at.push(b);
                    ids.push(id.clone());
                    parents.push(None);
                    for leaf in &branch.leaves {
                        if let Some(leaf_id) = tree.append_item(&id, leaf, None, None) {
                            ids.push(leaf_id);
                            parents.push(Some(b));
                        }
                    }
                }
            }
            let items = win::items(hwnd);
            let inner = Rc::new(RefCell::new(Inner { hwnd, items, ids, parents, branch_at, last: Vec::new(), timer: None }));
            {
                let i = inner.borrow();
                for &item in &i.items {
                    win::set_state(hwnd, item, UNCHECKED);
                }
            }
            inner.borrow_mut().snapshot();

            let timer = Timer::new(&tree);
            let weak = Rc::downgrade(&inner);
            timer.on_tick(move |_| {
                if let Some(inner) = weak.upgrade() {
                    if let Ok(mut inner) = inner.try_borrow_mut() {
                        inner.sync();
                    }
                }
            });
            timer.start(POLL_MS, false);
            inner.borrow_mut().timer = Some(timer);

            CheckTree { tree, inner }
        }

        /// Stops watching for changes. Call it before the tree's window is
        /// destroyed, or the timer would fire at a window that's gone.
        pub fn close(&self) {
            self.inner.borrow_mut().timer = None;
        }

        pub fn add_to(&self, sizer: &BoxSizer, proportion: i32, flags: SizerFlag, border: i32) {
            sizer.add(&self.tree, proportion, flags, border);
        }

        pub fn is_checked(&self, branch: usize, leaf: usize) -> bool {
            let mut inner = self.inner.borrow_mut();
            inner.sync();
            let at = inner.leaf_at(branch, leaf);
            win::state(inner.hwnd, inner.items[at]) == CHECKED
        }

        pub fn set_checked(&self, branch: usize, leaf: usize, checked: bool) {
            let mut inner = self.inner.borrow_mut();
            let at = inner.leaf_at(branch, leaf);
            win::set_state(inner.hwnd, inner.items[at], if checked { CHECKED } else { UNCHECKED });
            inner.update_branches();
            inner.snapshot();
        }

        pub fn set_all(&self, checked: bool) {
            let mut inner = self.inner.borrow_mut();
            for &item in &inner.items {
                win::set_state(inner.hwnd, item, if checked { CHECKED } else { UNCHECKED });
            }
            inner.snapshot();
        }

        pub fn set_leaf_label(&self, branch: usize, leaf: usize, label: &str) {
            let inner = self.inner.borrow();
            let at = inner.leaf_at(branch, leaf);
            self.tree.set_item_text(&inner.ids[at], label);
        }
    }

    impl Inner {
        fn leaf_at(&self, branch: usize, leaf: usize) -> usize {
            self.branch_at[branch] + 1 + leaf
        }

        fn children(&self, branch_item: usize) -> Vec<usize> {
            (0..self.items.len()).filter(|&i| self.parents[i] == Some(branch_item)).collect()
        }

        fn snapshot(&mut self) {
            self.last = self.items.iter().map(|&i| win::state(self.hwnd, i)).collect();
        }

        /// Each branch shows checked, unchecked or half checked from its leaves.
        fn update_branches(&self) {
            for &b in &self.branch_at {
                let states: Vec<u32> = self.children(b).iter().map(|&c| win::state(self.hwnd, self.items[c])).collect();
                let s = if !states.is_empty() && states.iter().all(|&s| s == CHECKED) {
                    CHECKED
                } else if states.iter().all(|&s| s != CHECKED) {
                    UNCHECKED
                } else {
                    PARTIAL
                };
                win::set_state(self.hwnd, self.items[b], s);
            }
        }

        /// Applies the links after the user changes a checkbox.
        fn sync(&mut self) {
            let now: Vec<u32> = self.items.iter().map(|&i| win::state(self.hwnd, i)).collect();
            if let Some(i) = (0..now.len()).find(|&i| now[i] != self.last[i]) {
                if self.parents[i].is_none() {
                    // Space cycles a branch through half checked too. As a
                    // choice that means nothing, so treat it as unchecked.
                    let on = now[i] == CHECKED;
                    for c in self.children(i) {
                        win::set_state(self.hwnd, self.items[c], if on { CHECKED } else { UNCHECKED });
                    }
                }
                self.update_branches();
            }
            self.snapshot();
        }
    }

    /// The tree view's own checkboxes, which wxWidgets doesn't expose.
    mod win {
        use std::ffi::c_void;

        #[link(name = "user32")]
        unsafe extern "system" {
            fn GetWindowLongPtrW(hwnd: *mut c_void, index: i32) -> isize;
            fn SetWindowLongPtrW(hwnd: *mut c_void, index: i32, value: isize) -> isize;
            fn SendMessageW(hwnd: *mut c_void, msg: u32, wparam: usize, lparam: isize) -> isize;
        }

        const GWL_STYLE: i32 = -16;
        const TVS_CHECKBOXES: isize = 0x0100;
        const TVS_EX_PARTIALCHECKBOXES: isize = 0x0080;
        const TV_FIRST: u32 = 0x1100;
        const TVM_GETNEXTITEM: u32 = TV_FIRST + 10;
        const TVM_GETITEMSTATE: u32 = TV_FIRST + 39;
        const TVM_SETEXTENDEDSTYLE: u32 = TV_FIRST + 44;
        const TVM_SETITEMW: u32 = TV_FIRST + 63;
        const TVGN_ROOT: usize = 0x0;
        const TVGN_NEXT: usize = 0x1;
        const TVGN_CHILD: usize = 0x4;
        const TVIF_STATE: u32 = 0x8;
        const TVIS_STATEIMAGEMASK: u32 = 0xF000;

        #[repr(C)]
        struct TvItemW {
            mask: u32,
            item: isize,
            state: u32,
            state_mask: u32,
            text: *mut u16,
            text_max: i32,
            image: i32,
            selected_image: i32,
            children: i32,
            param: isize,
        }

        pub fn enable_checkboxes(hwnd: *mut c_void) {
            unsafe {
                let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
                SetWindowLongPtrW(hwnd, GWL_STYLE, style | TVS_CHECKBOXES);
                SendMessageW(hwnd, TVM_SETEXTENDEDSTYLE, TVS_EX_PARTIALCHECKBOXES as usize, TVS_EX_PARTIALCHECKBOXES);
            }
        }

        /// Every item, parents before their children, in display order.
        pub fn items(hwnd: *mut c_void) -> Vec<isize> {
            fn walk(hwnd: *mut c_void, mut item: isize, out: &mut Vec<isize>) {
                while item != 0 {
                    out.push(item);
                    walk(hwnd, unsafe { SendMessageW(hwnd, TVM_GETNEXTITEM, TVGN_CHILD, item) }, out);
                    item = unsafe { SendMessageW(hwnd, TVM_GETNEXTITEM, TVGN_NEXT, item) };
                }
            }
            let mut out = Vec::new();
            walk(hwnd, unsafe { SendMessageW(hwnd, TVM_GETNEXTITEM, TVGN_ROOT, 0) }, &mut out);
            out
        }

        /// The checkbox image: 1 unchecked, 2 checked, 3 half checked.
        pub fn state(hwnd: *mut c_void, item: isize) -> u32 {
            let state =
                unsafe { SendMessageW(hwnd, TVM_GETITEMSTATE, item as usize, TVIS_STATEIMAGEMASK as isize) } as u32;
            (state & TVIS_STATEIMAGEMASK) >> 12
        }

        pub fn set_state(hwnd: *mut c_void, item: isize, image: u32) {
            let mut tv = TvItemW {
                mask: TVIF_STATE,
                item,
                state: image << 12,
                state_mask: TVIS_STATEIMAGEMASK,
                text: std::ptr::null_mut(),
                text_max: 0,
                image: 0,
                selected_image: 0,
                children: 0,
                param: 0,
            };
            unsafe { SendMessageW(hwnd, TVM_SETITEMW, 0, &mut tv as *mut TvItemW as isize) };
        }
    }
}

/// Plain checkboxes, one per leaf, each labelled "branch: leaf". Used on
/// Linux; compiled everywhere so it keeps building.
#[cfg_attr(not(all(unix, not(target_os = "macos"))), allow(dead_code))]
mod flat {
    use std::rc::Rc;

    use wxdragon::prelude::*;

    use super::Branch;

    #[derive(Clone)]
    pub struct CheckTree {
        sizer: Rc<BoxSizer>,
        /// Each branch's label and its leaves' boxes.
        boxes: Rc<Vec<(String, Vec<CheckBox>)>>,
    }

    impl CheckTree {
        pub fn new(parent: &dyn WxWidget, branches: &[Branch]) -> Self {
            let sizer = BoxSizer::builder(Orientation::Vertical).build();
            let boxes = branches
                .iter()
                .map(|branch| {
                    let leaves = branch
                        .leaves
                        .iter()
                        .map(|leaf| {
                            let check = CheckBox::builder(parent).with_label(&label(&branch.label, leaf)).build();
                            sizer.add(&check, 0, SizerFlag::All, 4);
                            check
                        })
                        .collect();
                    (branch.label.clone(), leaves)
                })
                .collect();
            CheckTree { sizer: Rc::new(sizer), boxes: Rc::new(boxes) }
        }

        pub fn close(&self) {}

        pub fn add_to(&self, sizer: &BoxSizer, proportion: i32, flags: SizerFlag, border: i32) {
            sizer.add_sizer(&*self.sizer, proportion, flags, border);
        }

        pub fn is_checked(&self, branch: usize, leaf: usize) -> bool {
            self.boxes[branch].1[leaf].is_checked()
        }

        pub fn set_checked(&self, branch: usize, leaf: usize, checked: bool) {
            self.boxes[branch].1[leaf].set_value(checked);
        }

        pub fn set_all(&self, checked: bool) {
            for (_, leaves) in self.boxes.iter() {
                for check in leaves {
                    check.set_value(checked);
                }
            }
        }

        pub fn set_leaf_label(&self, branch: usize, leaf: usize, text: &str) {
            let (name, leaves) = &self.boxes[branch];
            leaves[leaf].set_label(&label(name, text));
        }
    }

    fn label(branch: &str, leaf: &str) -> String {
        format!("{branch}: {leaf}")
    }
}

/// wxTreeListCtrl. Used on macOS; compiled elsewhere too so it keeps
/// building.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod generic {
    use wxdragon::prelude::*;
    use wxdragon::widgets::list_ctrl::ListColumnFormat;
    use wxdragon::widgets::treelistctrl::{CheckboxState, TreeListCtrl, TreeListCtrlStyle, TreeListItem};

    use super::Branch;

    #[derive(Clone)]
    pub struct CheckTree {
        tree: TreeListCtrl,
        branches: Vec<(TreeListItem, Vec<TreeListItem>)>,
    }

    impl CheckTree {
        pub fn new(parent: &dyn WxWidget, branches: &[Branch]) -> Self {
            let tree = TreeListCtrl::builder(parent)
                .with_style(TreeListCtrlStyle::Checkbox | TreeListCtrlStyle::Three_State)
                .build();
            tree.append_column("File type", 400, ListColumnFormat::Left);
            let root = tree.get_root_item();
            let mut items = Vec::new();
            for branch in branches {
                let Some(b) = tree.append_item(&root, &branch.label) else { continue };
                let leaves = branch.leaves.iter().filter_map(|l| tree.append_item(&b, l)).collect();
                items.push((b, leaves));
            }
            let this = CheckTree { tree, branches: items };
            let links = this.clone();
            tree.on_item_checked(move |e| {
                let Some(item) = e.get_item() else { return };
                let state = links.tree.get_checked_state(&item);
                for (b, leaves) in &links.branches {
                    if links.tree.get_item_text(b, 0) == links.tree.get_item_text(&item, 0) && tree.get_first_child(&item).is_some() {
                        // A half checked branch means nothing as a choice.
                        let on = if state == CheckboxState::Checked { CheckboxState::Checked } else { CheckboxState::Unchecked };
                        links.tree.check_item_recursively(b, on);
                        return;
                    }
                    if leaves.iter().any(|l| links.tree.get_item_text(l, 0) == links.tree.get_item_text(&item, 0)) {
                        links.update_branch(b);
                        return;
                    }
                }
            });
            this
        }

        pub fn close(&self) {}

        fn update_branch(&self, branch: &TreeListItem) {
            let state = if self.tree.are_all_children_in_state(branch, CheckboxState::Checked) {
                CheckboxState::Checked
            } else if self.tree.are_all_children_in_state(branch, CheckboxState::Unchecked) {
                CheckboxState::Unchecked
            } else {
                CheckboxState::Undetermined
            };
            self.tree.check_item(branch, state);
        }

        pub fn add_to(&self, sizer: &BoxSizer, proportion: i32, flags: SizerFlag, border: i32) {
            sizer.add(&self.tree, proportion, flags, border);
        }

        pub fn is_checked(&self, branch: usize, leaf: usize) -> bool {
            self.tree.is_checked(&self.branches[branch].1[leaf])
        }

        pub fn set_checked(&self, branch: usize, leaf: usize, checked: bool) {
            let (b, leaves) = &self.branches[branch];
            let state = if checked { CheckboxState::Checked } else { CheckboxState::Unchecked };
            self.tree.check_item(&leaves[leaf], state);
            self.update_branch(b);
        }

        pub fn set_all(&self, checked: bool) {
            let state = if checked { CheckboxState::Checked } else { CheckboxState::Unchecked };
            for (b, _) in &self.branches {
                self.tree.check_item_recursively(b, state);
            }
        }

        pub fn set_leaf_label(&self, branch: usize, leaf: usize, label: &str) {
            self.tree.set_item_text(&self.branches[branch].1[leaf], 0, label);
        }
    }
}
