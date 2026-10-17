//! One window per file. A second launch for the same file hands its target
//! to the window that already has it, then exits.

use std::path::Path;
use std::time::Duration;

use wxdragon::ipc::{IPCClient, IPCConnection, IPCFormat, IPCServer};
use wxdragon::prelude::SingleInstanceChecker;

use crate::args::Target;

const TOPIC: &str = "goto";

/// A name for this file that every mder process derives the same way.
fn key(path: &Path) -> String {
    let full = std::fs::canonicalize(path)
        .or_else(|_| std::path::absolute(path))
        .unwrap_or_else(|_| path.to_path_buf());
    let mut s = full.to_string_lossy().into_owned();
    if cfg!(windows) {
        s = s.to_lowercase();
    }
    // FNV-1a: stable across builds, unlike std's hasher.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("mder-{h:016x}")
}

fn service(key: &str) -> String {
    if cfg!(windows) {
        key.to_string()
    } else {
        std::env::temp_dir().join(format!("{key}.sock")).to_string_lossy().into_owned()
    }
}

pub enum Claim {
    /// This process owns the file. Keep both alive for as long as it's open.
    #[allow(dead_code)]
    Owner(SingleInstanceChecker, Option<IPCServer>),
    /// Another process has it and took the target.
    HandedOff,
    /// Another process has it but didn't answer.
    Unreachable,
}

/// Claims `path` for this process, or hands `target` to the process that has it.
/// `on_target` runs on the UI thread when a later launch hands over a target.
pub fn claim(path: &Path, target: &Target, on_target: impl Fn(Target) + Clone + 'static) -> Option<Claim> {
    let key = key(path);
    let checker = SingleInstanceChecker::new(&key, None)?;
    if !checker.is_another_running() {
        let server = IPCServer::new(move |_topic| {
            let on_target = on_target.clone();
            Some(
                IPCConnection::builder()
                    .on_execute(move |_topic, data, format| {
                        on_target(Target::decode(&decode_text(data, format)));
                        true
                    })
                    .build(),
            )
        });
        let server = server.create(&service(&key)).then_some(server);
        return Some(Claim::Owner(checker, server));
    }

    allow_foreground();
    let client = IPCClient::new();
    // The owner may still be starting up.
    for _ in 0..15 {
        if let Some(conn) = client.make_connection("localhost", &service(&key), TOPIC) {
            let ok = conn.execute_string(&target.encode());
            conn.disconnect();
            if ok {
                return Some(Claim::HandedOff);
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Some(Claim::Unreachable)
}

/// DDE on Windows hands text over as UTF-16, whatever format it reports.
fn decode_text(data: &[u8], format: IPCFormat) -> String {
    let utf16 = format == IPCFormat::UnicodeText || (data.len() >= 2 && data[1] == 0);
    let text = if utf16 {
        let units: Vec<u16> = data.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(data).into_owned()
    };
    text.trim_end_matches('\0').to_string()
}

/// Lets the owning process bring its window to the front.
#[cfg(windows)]
fn allow_foreground() {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn AllowSetForegroundWindow(process_id: u32) -> i32;
    }
    const ASFW_ANY: u32 = u32::MAX;
    unsafe {
        AllowSetForegroundWindow(ASFW_ANY);
    }
}

#[cfg(not(windows))]
fn allow_foreground() {}
