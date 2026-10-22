//! The few Linux things wxDragon has no call for, asked of GTK and X11
//! directly.

use std::ffi::{CString, c_char, c_int, c_long, c_ulong, c_void};

#[link(name = "gtk-3")]
unsafe extern "C" {
    fn gtk_widget_get_window(widget: *mut c_void) -> *mut c_void;
    fn gtk_window_present(window: *mut c_void);
}

#[link(name = "gdk-3")]
unsafe extern "C" {
    fn gdk_window_get_display(window: *mut c_void) -> *mut c_void;
    fn gdk_wayland_display_get_type() -> usize;
    fn gdk_wayland_display_set_startup_notification_id(display: *mut c_void, id: *const c_char);
    fn gdk_notify_startup_complete_with_id(id: *const c_char);
    fn gdk_x11_display_get_type() -> usize;
    fn gdk_x11_display_get_xdisplay(display: *mut c_void) -> *mut c_void;
    fn gdk_x11_window_get_xid(window: *mut c_void) -> c_ulong;
    fn gdk_x11_get_server_time(window: *mut c_void) -> u32;
}

#[link(name = "gobject-2.0")]
unsafe extern "C" {
    fn g_type_check_instance_is_a(instance: *mut c_void, type_id: usize) -> c_int;
}

#[link(name = "X11")]
unsafe extern "C" {
    fn XInternAtom(display: *mut c_void, name: *const c_char, only_if_exists: c_int) -> c_ulong;
    fn XDefaultRootWindow(display: *mut c_void) -> c_ulong;
    fn XSendEvent(display: *mut c_void, window: c_ulong, propagate: c_int, mask: c_long, event: *mut XEvent) -> c_int;
    fn XFlush(display: *mut c_void) -> c_int;
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XClientMessageEvent {
    kind: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut c_void,
    window: c_ulong,
    message_type: c_ulong,
    format: c_int,
    data: [c_long; 5],
}

/// Xlib's XEvent: a union 24 longs long.
#[repr(C)]
union XEvent {
    client: XClientMessageEvent,
    pad: [c_long; 24],
}

/// Brings a window to the front on an X11 desktop. Window managers refuse a
/// plain raise from an app while you're working elsewhere (focus stealing
/// prevention), so this asks the way a taskbar does, which they honor: a
/// `_NET_ACTIVE_WINDOW` request saying it comes from a pager. Returns
/// whether it could ask; on Wayland it can't.
pub fn activate(widget: *mut c_void) -> bool {
    const CLIENT_MESSAGE: c_int = 33;
    const FROM_PAGER: c_long = 2;
    const SUBSTRUCTURE_NOTIFY: c_long = 1 << 19;
    const SUBSTRUCTURE_REDIRECT: c_long = 1 << 20;
    if widget.is_null() {
        return false;
    }
    unsafe {
        let window = gtk_widget_get_window(widget);
        if window.is_null() {
            return false;
        }
        let display = gdk_window_get_display(window);
        if display.is_null() || g_type_check_instance_is_a(display, gdk_x11_display_get_type()) == 0 {
            return false;
        }
        let xdisplay = gdk_x11_display_get_xdisplay(display);
        let mut event = XEvent { pad: [0; 24] };
        event.client = XClientMessageEvent {
            kind: CLIENT_MESSAGE,
            serial: 0,
            send_event: 1,
            display: xdisplay,
            window: gdk_x11_window_get_xid(window),
            message_type: XInternAtom(xdisplay, c"_NET_ACTIVE_WINDOW".as_ptr(), 0),
            format: 32,
            data: [FROM_PAGER, gdk_x11_get_server_time(window) as c_long, 0, 0, 0],
        };
        XSendEvent(xdisplay, XDefaultRootWindow(xdisplay), 0, SUBSTRUCTURE_NOTIFY | SUBSTRUCTURE_REDIRECT, &mut event);
        XFlush(xdisplay);
    }
    true
}

/// Tells an X11 desktop that a launch is done starting. A launch that hands
/// its file to another window quits without a window of its own, so that
/// window says it for it; otherwise the desktop waits for it a while.
pub fn startup_complete(id: &str) {
    if let Ok(id) = CString::new(id) {
        unsafe { gdk_notify_startup_complete_with_id(id.as_ptr()) };
    }
}

/// Brings a window to the front on Wayland, which only allows it with an
/// activation token: the file manager gives one to each launch, and the
/// launch that found the file already open passes it on. GTK presents the
/// window with it, through the xdg-activation protocol. Returns whether it
/// could try; whether the desktop honors the token, it doesn't say.
pub fn activate_with_token(widget: *mut c_void, token: &str) -> bool {
    let Ok(token) = CString::new(token) else { return false };
    if widget.is_null() {
        return false;
    }
    unsafe {
        let window = gtk_widget_get_window(widget);
        if window.is_null() {
            return false;
        }
        let display = gdk_window_get_display(window);
        if display.is_null() || g_type_check_instance_is_a(display, gdk_wayland_display_get_type()) == 0 {
            return false;
        }
        // GTK uses the display's startup id for the next present, then forgets it.
        gdk_wayland_display_set_startup_notification_id(display, token.as_ptr());
        gtk_window_present(widget);
    }
    true
}
