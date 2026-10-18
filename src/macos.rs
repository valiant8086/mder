//! The few macOS things wxDragon has no call for, asked of AppKit directly
//! through the Objective-C runtime.

use std::cell::RefCell;
use std::ffi::{CStr, c_char, c_void};
use std::sync::OnceLock;

#[link(name = "objc")]
unsafe extern "C" {
    fn objc_getClass(name: *const c_char) -> *mut c_void;
    fn sel_registerName(name: *const c_char) -> *mut c_void;
    fn objc_msgSend();
}

/// A class and one of its selectors, or None if AppKit has no such class.
fn class_method(class: &CStr, selector: &CStr) -> Option<(*mut c_void, *mut c_void)> {
    unsafe {
        let class = objc_getClass(class.as_ptr());
        if class.is_null() {
            return None;
        }
        Some((class, sel_registerName(selector.as_ptr())))
    }
}

/// macOS adds "Show Tab Bar" and "Show All Tabs" to an app's View menu, for
/// merging its windows into tabs. Each mder window is its own process, and
/// macOS only tabs one process's windows together, so they'd do nothing.
/// This is `[NSWindow setAllowsAutomaticWindowTabbing:NO]`. Call it before
/// any window exists.
pub fn disable_window_tabbing() {
    let Some((class, selector)) = class_method(c"NSWindow", c"setAllowsAutomaticWindowTabbing:") else { return };
    unsafe {
        // objc_msgSend has to be called through a pointer of the right type.
        let send: unsafe extern "C" fn(*mut c_void, *mut c_void, bool) = std::mem::transmute(objc_msgSend as *const ());
        send(class, selector, false);
    }
}

/// Calls Control+Tab's handler while it's alive: `forward` is false with
/// Shift. macOS uses Control+Tab to move between controls before a window
/// sees the key, so this watches keys as the app receives them, with
/// `[NSEvent addLocalMonitorForEventsMatchingMask:handler:]`.
pub struct ControlTab {
    monitor: *mut c_void,
}

thread_local! {
    static CONTROL_TAB: RefCell<Option<Box<dyn Fn(bool)>>> = RefCell::new(None);
}

impl ControlTab {
    pub fn new(handler: impl Fn(bool) + 'static) -> Option<Self> {
        const KEY_DOWN_MASK: u64 = 1 << 10; // NSEventMaskKeyDown
        let (class, selector) = class_method(c"NSEvent", c"addLocalMonitorForEventsMatchingMask:handler:")?;
        CONTROL_TAB.with(|h| *h.borrow_mut() = Some(Box::new(handler)));
        let monitor = unsafe {
            let send: unsafe extern "C" fn(*mut c_void, *mut c_void, u64, *const Block) -> *mut c_void =
                std::mem::transmute(objc_msgSend as *const ());
            send(class, selector, KEY_DOWN_MASK, key_down_block())
        };
        Some(ControlTab { monitor })
    }
}

impl Drop for ControlTab {
    fn drop(&mut self) {
        CONTROL_TAB.with(|h| *h.borrow_mut() = None);
        if let Some((class, selector)) = class_method(c"NSEvent", c"removeMonitor:") {
            unsafe {
                let send: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) = std::mem::transmute(objc_msgSend as *const ());
                send(class, selector, self.monitor);
            }
        }
    }
}

/// The monitor's handler: swallows Control+Tab and Control+Shift+Tab once
/// the handler has them, and passes every other key on.
unsafe extern "C" fn on_key_down(_block: *const Block, event: *mut c_void) -> *mut c_void {
    const TAB: u16 = 48; // kVK_Tab
    const SHIFT: u64 = 1 << 17;
    const CONTROL: u64 = 1 << 18;
    const OPTION: u64 = 1 << 19;
    const COMMAND: u64 = 1 << 20;
    let (key, flags) = unsafe {
        let key: unsafe extern "C" fn(*mut c_void, *mut c_void) -> u16 = std::mem::transmute(objc_msgSend as *const ());
        let flags: unsafe extern "C" fn(*mut c_void, *mut c_void) -> u64 = std::mem::transmute(objc_msgSend as *const ());
        (
            key(event, sel_registerName(c"keyCode".as_ptr())),
            flags(event, sel_registerName(c"modifierFlags".as_ptr())),
        )
    };
    if key == TAB && flags & CONTROL != 0 && flags & (OPTION | COMMAND) == 0 {
        let forward = flags & SHIFT == 0;
        let handled = CONTROL_TAB.with(|h| match h.try_borrow().as_deref() {
            Ok(Some(handler)) => {
                handler(forward);
                true
            }
            _ => false,
        });
        if handled {
            return std::ptr::null_mut();
        }
    }
    event
}

/// An Objective-C block, as the monitor wants its handler: a global one,
/// which needs no copying or freeing. See clang's Block ABI.
#[repr(C)]
struct Block {
    isa: *const c_void,
    flags: i32,
    reserved: i32,
    invoke: unsafe extern "C" fn(*const Block, *mut c_void) -> *mut c_void,
    descriptor: *const BlockDescriptor,
}

#[repr(C)]
struct BlockDescriptor {
    reserved: usize,
    size: usize,
}

unsafe extern "C" {
    static _NSConcreteGlobalBlock: c_void;
}

fn key_down_block() -> *const Block {
    const BLOCK_IS_GLOBAL: i32 = 1 << 28;
    static DESCRIPTOR: BlockDescriptor = BlockDescriptor { reserved: 0, size: std::mem::size_of::<Block>() };
    static BLOCK: OnceLock<usize> = OnceLock::new();
    *BLOCK.get_or_init(|| {
        let block = Box::new(Block {
            isa: &raw const _NSConcreteGlobalBlock,
            flags: BLOCK_IS_GLOBAL,
            reserved: 0,
            invoke: on_key_down,
            descriptor: &DESCRIPTOR,
        });
        Box::into_raw(block) as usize
    }) as *const Block
}
