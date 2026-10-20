//! The few macOS things wxDragon has no call for, asked of AppKit directly
//! through the Objective-C runtime.

use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char, c_void};
use std::sync::OnceLock;

/// An Objective-C object or class.
pub type Id = *mut c_void;

#[link(name = "objc")]
unsafe extern "C" {
    fn objc_getClass(name: *const c_char) -> Id;
    fn sel_registerName(name: *const c_char) -> Id;
    fn objc_msgSend();
}

// UTType, for file types.
#[link(name = "UniformTypeIdentifiers", kind = "framework")]
unsafe extern "C" {}

/// A class by name, or null if there's no such class.
pub fn class(name: &CStr) -> Id {
    unsafe { objc_getClass(name.as_ptr()) }
}

// objc_msgSend has to be called through a pointer of the right type, so one
// of these per number of arguments.

pub unsafe fn send0<R>(receiver: Id, selector: &CStr) -> R {
    unsafe {
        let f: unsafe extern "C" fn(Id, Id) -> R = std::mem::transmute(objc_msgSend as *const ());
        f(receiver, sel_registerName(selector.as_ptr()))
    }
}

pub unsafe fn send1<A, R>(receiver: Id, selector: &CStr, a: A) -> R {
    unsafe {
        let f: unsafe extern "C" fn(Id, Id, A) -> R = std::mem::transmute(objc_msgSend as *const ());
        f(receiver, sel_registerName(selector.as_ptr()), a)
    }
}

pub unsafe fn send2<A, B, R>(receiver: Id, selector: &CStr, a: A, b: B) -> R {
    unsafe {
        let f: unsafe extern "C" fn(Id, Id, A, B) -> R = std::mem::transmute(objc_msgSend as *const ());
        f(receiver, sel_registerName(selector.as_ptr()), a, b)
    }
}

pub unsafe fn send3<A, B, C, R>(receiver: Id, selector: &CStr, a: A, b: B, c: C) -> R {
    unsafe {
        let f: unsafe extern "C" fn(Id, Id, A, B, C) -> R = std::mem::transmute(objc_msgSend as *const ());
        f(receiver, sel_registerName(selector.as_ptr()), a, b, c)
    }
}

pub unsafe fn send4<A, B, C, D, R>(receiver: Id, selector: &CStr, a: A, b: B, c: C, d: D) -> R {
    unsafe {
        let f: unsafe extern "C" fn(Id, Id, A, B, C, D) -> R = std::mem::transmute(objc_msgSend as *const ());
        f(receiver, sel_registerName(selector.as_ptr()), a, b, c, d)
    }
}

/// An NSString. Autoreleased, so only good until the event loop runs on.
pub fn ns_string(s: &str) -> Id {
    let Ok(s) = CString::new(s) else { return std::ptr::null_mut() };
    unsafe { send1(class(c"NSString"), c"stringWithUTF8String:", s.as_ptr()) }
}

/// An NSString's text, or None for nil.
pub fn rust_string(ns: Id) -> Option<String> {
    if ns.is_null() {
        return None;
    }
    let text: *const c_char = unsafe { send0(ns, c"UTF8String") };
    (!text.is_null()).then(|| unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned())
}

/// mder.app's bundle identifier, set in scripts/macos-app.sh.
pub const BUNDLE_ID: &str = "com.valiant8086.mder";

fn ns_app() -> Id {
    unsafe { send0(class(c"NSApplication"), c"sharedApplication") }
}

fn responds_to(object: Id, selector: &CStr) -> bool {
    !object.is_null() && unsafe { send1::<Id, bool>(object, c"respondsToSelector:", sel_registerName(selector.as_ptr())) }
}

/// Brings mder in front of other apps. Since macOS 14, that only works if
/// the active app has let it: see `yield_to_mder`.
pub fn activate() {
    let app = ns_app();
    if responds_to(app, c"activate") {
        unsafe { send0::<()>(app, c"activate") };
    } else {
        unsafe { send1::<bool, ()>(app, c"activateIgnoringOtherApps:", true) };
    }
}

/// Lets another mder take the front from this one, as Windows'
/// AllowSetForegroundWindow does. A second launch calls it before handing
/// over its target: macOS made it the active app, so only it can pass that
/// on. It first waits to become active, since macOS does that through the
/// event loop, which a second launch never gets to run. Returns whether it
/// was active.
pub fn yield_to_mder() -> bool {
    let app = ns_app();
    let active = wait_until_active(app, std::time::Duration::from_millis(1500));
    if !responds_to(app, c"yieldActivationToApplication:") {
        return active;
    }
    // Every mder shares one bundle identifier, so yielding to that would be
    // yielding to this launch itself. Yield to each other mder instead; only
    // the one with the file asks for the front.
    let me = std::process::id() as i32;
    unsafe {
        let others: Id =
            send1(class(c"NSRunningApplication"), c"runningApplicationsWithBundleIdentifier:", ns_string(BUNDLE_ID));
        let count: usize = if others.is_null() { 0 } else { send0(others, c"count") };
        for i in 0..count {
            let other: Id = send1(others, c"objectAtIndex:", i);
            if send0::<i32>(other, c"processIdentifier") != me {
                send1::<Id, ()>(app, c"yieldActivationToApplication:", other);
            }
        }
    }
    active
}

/// After handing over its target, a second launch waits for the mder it
/// yielded to to take the front, before it exits: if it exits first, macOS
/// gives the front back to the app before it, and the other mder is too
/// late. Returns whether that happened in time.
pub fn wait_for_mder_to_take_front() -> bool {
    pump_events_until(ns_app(), std::time::Duration::from_secs(2), false)
}

fn wait_until_active(app: Id, limit: std::time::Duration) -> bool {
    pump_events_until(app, limit, true)
}

/// Handles events until this app's active state is `active`, for up to
/// `limit`. Returns whether it got there. Handling events is also what lets
/// macOS change that state, and lets the app answer VoiceOver meanwhile.
fn pump_events_until(app: Id, limit: std::time::Duration, active: bool) -> bool {
    const ANY_EVENT: u64 = u64::MAX;
    let deadline = std::time::Instant::now() + limit;
    let mode = ns_string("kCFRunLoopDefaultMode"); // NSDefaultRunLoopMode
    loop {
        if unsafe { send0::<bool>(app, c"isActive") } == active {
            return true;
        }
        if std::time::Instant::now() > deadline {
            return false;
        }
        unsafe {
            let until: Id = send1(class(c"NSDate"), c"dateWithTimeIntervalSinceNow:", 0.05f64);
            let event: Id = send4(app, c"nextEventMatchingMask:untilDate:inMode:dequeue:", ANY_EVENT, until, mode, true);
            if !event.is_null() {
                send1::<Id, ()>(app, c"sendEvent:", event);
            }
        }
    }
}

/// macOS adds "Show Tab Bar" and "Show All Tabs" to an app's View menu, for
/// merging its windows into tabs. Each mder window is its own process, and
/// macOS only tabs one process's windows together, so they'd do nothing.
/// This is `[NSWindow setAllowsAutomaticWindowTabbing:NO]`. Call it before
/// any window exists.
pub fn disable_window_tabbing() {
    let window = class(c"NSWindow");
    if !window.is_null() {
        unsafe { send1::<bool, ()>(window, c"setAllowsAutomaticWindowTabbing:", false) };
    }
}

/// Calls Control+Tab's handler while it's alive: `forward` is false with
/// Shift. macOS uses Control+Tab to move between controls before a window
/// sees the key, so this watches keys as the app receives them, with
/// `[NSEvent addLocalMonitorForEventsMatchingMask:handler:]`.
pub struct ControlTab {
    monitor: Id,
}

thread_local! {
    static CONTROL_TAB: RefCell<Option<Box<dyn Fn(bool)>>> = RefCell::new(None);
}

impl ControlTab {
    pub fn new(handler: impl Fn(bool) + 'static) -> Option<Self> {
        const KEY_DOWN_MASK: u64 = 1 << 10; // NSEventMaskKeyDown
        static BLOCK: OnceLock<usize> = OnceLock::new();
        let event = class(c"NSEvent");
        if event.is_null() {
            return None;
        }
        CONTROL_TAB.with(|h| *h.borrow_mut() = Some(Box::new(handler)));
        let block = global_block(&BLOCK, on_key_down as *const c_void);
        let monitor = unsafe { send2(event, c"addLocalMonitorForEventsMatchingMask:handler:", KEY_DOWN_MASK, block) };
        Some(ControlTab { monitor })
    }
}

impl Drop for ControlTab {
    fn drop(&mut self) {
        CONTROL_TAB.with(|h| *h.borrow_mut() = None);
        unsafe { send1::<Id, ()>(class(c"NSEvent"), c"removeMonitor:", self.monitor) };
    }
}

/// The monitor's handler: swallows Control+Tab and Control+Shift+Tab once
/// the handler has them, and passes every other key on.
unsafe extern "C" fn on_key_down(_block: *const c_void, event: Id) -> Id {
    const TAB: u16 = 48; // kVK_Tab
    const SHIFT: u64 = 1 << 17;
    const CONTROL: u64 = 1 << 18;
    const OPTION: u64 = 1 << 19;
    const COMMAND: u64 = 1 << 20;
    let key: u16 = unsafe { send0(event, c"keyCode") };
    let flags: u64 = unsafe { send0(event, c"modifierFlags") };
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

/// An Objective-C block for a plain function, as AppKit wants for handlers:
/// a global one, which needs no copying or freeing, made once and kept in
/// `cell`. `invoke` takes the block itself first, then the block's
/// arguments. See clang's Block ABI.
pub fn global_block(cell: &'static OnceLock<usize>, invoke: *const c_void) -> *const c_void {
    #[repr(C)]
    struct Block {
        isa: *const c_void,
        flags: i32,
        reserved: i32,
        invoke: *const c_void,
        descriptor: *const Descriptor,
    }
    #[repr(C)]
    struct Descriptor {
        reserved: usize,
        size: usize,
    }
    unsafe extern "C" {
        static _NSConcreteGlobalBlock: c_void;
    }
    const BLOCK_IS_GLOBAL: i32 = 1 << 28;
    static DESCRIPTOR: Descriptor = Descriptor { reserved: 0, size: std::mem::size_of::<Block>() };
    *cell.get_or_init(|| {
        let block = Box::new(Block {
            isa: &raw const _NSConcreteGlobalBlock,
            flags: BLOCK_IS_GLOBAL,
            reserved: 0,
            invoke,
            descriptor: &DESCRIPTOR,
        });
        Box::into_raw(block) as usize
    }) as *const c_void
}
