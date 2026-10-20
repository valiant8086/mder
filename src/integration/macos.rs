//! macOS: which app opens each Markdown file type on double-click.
//!
//! mder.app's Info.plist already puts mder in Finder's Open With for every
//! extension here, so a box means "mder is the default app for this type".
//! macOS keeps defaults per file type, not per extension: .md and .markdown
//! share its Markdown type, and the rarer extensions each get a type made up
//! from the extension. Types are looked up fresh every time, because another
//! app can declare a type that changes which one an extension belongs to.

use std::ffi::c_void;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use super::{Change, Confirmation, FileKind, Item, Status};
use crate::macos::{Id, class, global_block, ns_string, rust_string, send0, send1, send3};
use crate::settings::Settings;

/// mder.app's bundle identifier, set in scripts/macos-app.sh.
const BUNDLE_ID: &str = "com.valiant8086.mder";

/// How long to wait for macOS to confirm a change before letting the dialog
/// carry on. macOS may ask you first, in its own dialog, so it can take as
/// long as you do; `confirmation` reports on it after that.
const QUICK_CONFIRM: Duration = Duration::from_millis(1500);

/// mder.app, if mder is running as one.
fn this_app() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let app = exe.ancestors().nth(3)?;
    (app.extension()? == "app").then(|| app.to_path_buf())
}

pub fn supported() -> bool {
    this_app().is_some()
}

pub fn items(kind: &FileKind) -> Vec<Item> {
    let mut items: Vec<Item> = Vec::new();
    for &ext in kind.extensions {
        let key = identifier(file_type(ext)).unwrap_or_else(|| ext.to_string());
        match items.iter_mut().find(|i| i.key == key) {
            Some(item) => item.extensions.push(ext),
            None => items.push(Item { label: String::new(), key, extensions: vec![ext] }),
        }
    }
    for item in &mut items {
        item.label = super::join_extensions(&item.extensions);
    }
    items
}

pub fn status(_kind: &FileKind, item: &Item) -> Status {
    let Some(app) = default_app(item_type(item)) else { return Status::NotRegistered };
    if !is_mder(&app) {
        return Status::OpensIn(app_name(&app));
    }
    match this_app() {
        Some(me) if same_place(&me, &app) => Status::Registered,
        _ => Status::RegisteredElsewhere(app.display().to_string()),
    }
}

pub fn needs_choice(changes: &[Change]) -> bool {
    let settings = Settings::load();
    changes.iter().any(|c| !c.register && opens_in_mder(c.item) && remembered(&settings, c.item).is_none())
}

pub fn choices(changes: &[Change]) -> Vec<(String, PathBuf)> {
    let mut types: Vec<Id> = changes.iter().filter(|c| !c.register).map(|c| item_type(c.item)).collect();
    // Only mder claims the rarer extensions' types, so offer the apps for
    // Markdown and for plain text too.
    types.push(file_type(".md"));
    types.push(unsafe { send1(class(c"UTType"), c"typeWithIdentifier:", ns_string("public.plain-text")) });
    let mut apps: Vec<PathBuf> = Vec::new();
    for t in types {
        for app in apps_for(t) {
            if !is_mder(&app) && !apps.contains(&app) {
                apps.push(app);
            }
        }
    }
    let mut named: Vec<(String, PathBuf)> = apps.into_iter().map(|a| (app_name(&a), a)).collect();
    named.sort_by_key(|(name, _)| name.to_lowercase());
    // Two copies of one app: say where each is.
    let names: Vec<String> = named.iter().map(|(n, _)| n.clone()).collect();
    for (name, app) in &mut named {
        if names.iter().filter(|n| *n == name).count() > 1 {
            *name = format!("{name}, in {}", app.parent().unwrap_or(app).display());
        }
    }
    named
}

pub fn apply(changes: &[Change], instead: Option<&Path>) -> io::Result<()> {
    let me = this_app().ok_or_else(|| {
        io::Error::new(io::ErrorKind::Unsupported, "Making mder the default only works when it runs as mder.app.")
    })?;
    let mut settings = Settings::load();
    let before = settings.clone();
    let mut requests: Vec<(Id, PathBuf)> = Vec::new();
    let mut stuck: Vec<&str> = Vec::new();
    for c in changes {
        let t = item_type(c.item);
        if t.is_null() {
            continue;
        }
        let current = default_app(t);
        if c.register {
            match &current {
                Some(app) if is_mder(app) => {}
                Some(app) => {
                    settings.previous_defaults.insert(c.item.key.clone(), app.display().to_string());
                }
                None => {
                    settings.previous_defaults.remove(&c.item.key);
                }
            }
            requests.push((t, me.clone()));
        } else {
            if !current.as_deref().is_some_and(is_mder) {
                continue;
            }
            match remembered(&settings, c.item).or_else(|| instead.map(Path::to_path_buf)) {
                Some(app) => {
                    requests.push((t, app));
                    settings.previous_defaults.remove(&c.item.key);
                }
                None => stuck.extend(c.item.extensions.iter().copied()),
            }
        }
    }
    // Saved before macOS answers: it may wait for you to confirm.
    if settings != before {
        settings.save()?;
    }
    set_defaults(&requests);
    if !stuck.is_empty() {
        return Err(io::Error::other(format!(
            "mder doesn't know which app opened {} files before. To choose one, select one of those files in Finder, then File, Get Info, Open with, and Change All.",
            super::join_extensions(&stuck)
        )));
    }
    Ok(())
}

/// The type macOS gives files with this extension now. `ext` has its dot.
fn file_type(ext: &str) -> Id {
    let ext = ext.trim_start_matches('.');
    unsafe { send1(class(c"UTType"), c"typeWithFilenameExtension:", ns_string(ext)) }
}

/// An item's type, from its first extension: they all share it.
fn item_type(item: &Item) -> Id {
    item.extensions.first().map_or(std::ptr::null_mut(), |ext| file_type(ext))
}

fn identifier(file_type: Id) -> Option<String> {
    if file_type.is_null() {
        return None;
    }
    rust_string(unsafe { send0(file_type, c"identifier") })
}

fn workspace() -> Id {
    unsafe { send0(class(c"NSWorkspace"), c"sharedWorkspace") }
}

fn file_url(path: &Path) -> Id {
    unsafe { send1(class(c"NSURL"), c"fileURLWithPath:", ns_string(&path.to_string_lossy())) }
}

fn path_of(url: Id) -> Option<PathBuf> {
    if url.is_null() {
        return None;
    }
    rust_string(unsafe { send0(url, c"path") }).map(PathBuf::from)
}

/// The app that opens this type on double-click.
fn default_app(file_type: Id) -> Option<PathBuf> {
    if file_type.is_null() {
        return None;
    }
    path_of(unsafe { send1(workspace(), c"URLForApplicationToOpenContentType:", file_type) })
}

/// Every app that can open this type.
fn apps_for(file_type: Id) -> Vec<PathBuf> {
    if file_type.is_null() {
        return Vec::new();
    }
    let urls: Id = unsafe { send1(workspace(), c"URLsForApplicationsToOpenContentType:", file_type) };
    if urls.is_null() {
        return Vec::new();
    }
    let count: usize = unsafe { send0(urls, c"count") };
    (0..count).filter_map(|i| path_of(unsafe { send1(urls, c"objectAtIndex:", i) })).collect()
}

/// Whether an app is some copy of mder.
fn is_mder(app: &Path) -> bool {
    let bundle: Id = unsafe { send1(class(c"NSBundle"), c"bundleWithURL:", file_url(app)) };
    !bundle.is_null() && rust_string(unsafe { send0(bundle, c"bundleIdentifier") }).as_deref() == Some(BUNDLE_ID)
}

fn opens_in_mder(item: &Item) -> bool {
    default_app(item_type(item)).is_some_and(|app| is_mder(&app))
}

/// The app this type had before mder, if it's still there.
fn remembered(settings: &Settings, item: &Item) -> Option<PathBuf> {
    settings.previous_defaults.get(&item.key).map(PathBuf::from).filter(|app| app.exists() && !is_mder(app))
}

fn same_place(a: &Path, b: &Path) -> bool {
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    canon(a) == canon(b)
}

/// "TextEdit" for /System/Applications/TextEdit.app.
fn app_name(app: &Path) -> String {
    app.file_stem().map_or_else(|| app.display().to_string(), |s| s.to_string_lossy().into_owned())
}

/// Changes macOS has answered, and how many of those it refused, ever.
static CONFIRMED: AtomicUsize = AtomicUsize::new(0);
static REFUSED: AtomicUsize = AtomicUsize::new(0);
/// What CONFIRMED reaches once every change asked for is answered.
static EXPECTED: AtomicUsize = AtomicUsize::new(0);
/// REFUSED when `confirmation` last reported.
static REFUSED_SEEN: AtomicUsize = AtomicUsize::new(0);

/// macOS answers each change on a queue of its own, so this only counts.
/// An error means it refused, for example because you chose to keep the
/// app you had when it asked.
unsafe extern "C" fn answered(_block: *const c_void, error: Id) {
    if !error.is_null() {
        REFUSED.fetch_add(1, Ordering::SeqCst);
    }
    CONFIRMED.fetch_add(1, Ordering::SeqCst);
}

pub fn confirmation() -> Confirmation {
    if CONFIRMED.load(Ordering::SeqCst) < EXPECTED.load(Ordering::SeqCst) {
        return Confirmation::Waiting;
    }
    let refused = REFUSED.load(Ordering::SeqCst);
    if REFUSED_SEEN.swap(refused, Ordering::SeqCst) < refused {
        return Confirmation::Refused;
    }
    Confirmation::Done
}

/// Asks macOS to set each type's default app. Waits a moment, since it's
/// usually quick, but not for you to answer a question macOS asks.
fn set_defaults(requests: &[(Id, PathBuf)]) {
    if requests.is_empty() {
        return;
    }
    static BLOCK: OnceLock<usize> = OnceLock::new();
    let block = global_block(&BLOCK, answered as *const c_void);
    EXPECTED.fetch_add(requests.len(), Ordering::SeqCst);
    for (file_type, app) in requests {
        unsafe {
            send3::<Id, Id, *const c_void, ()>(
                workspace(),
                c"setDefaultApplicationAtURL:toOpenContentType:completionHandler:",
                file_url(app),
                *file_type,
                block,
            )
        };
    }
    let deadline = Instant::now() + QUICK_CONFIRM;
    while CONFIRMED.load(Ordering::SeqCst) < EXPECTED.load(Ordering::SeqCst) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
}
