//! The update dialogs: the daily check, Help, Check for update, the "A new
//! update is available!" dialog with Install Now, Ignore Version and Maybe
//! Later, and the download. The work itself is in update.rs.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use wxdragon::prelude::*;

use crate::settings::Settings;
use crate::update::{self, Entry, VERSION};

/// The daily check waits this long between checks.
const DAILY: u64 = 24 * 60 * 60;
/// Help, Check for update waits this long between checks.
const MANUAL_WAIT: u64 = 5 * 60;
/// Two windows starting together share one daily check.
const CHECK_LOCK_SECS: u64 = 120;

const ID_IGNORE: i32 = 7001;
const ID_INSTALLED_ELSEWHERE: i32 = 7002;

/// A newer version, ready to show.
pub struct Offer {
    pub latest: String,
    current_date: Option<String>,
    text: String,
}

fn offer(entries: &[Entry]) -> Option<Offer> {
    let latest = update::newer_than(entries, VERSION)?;
    Some(Offer {
        latest: latest.version.clone(),
        current_date: update::release_date(entries, VERSION),
        text: update::changes_text(entries, VERSION, &latest.version),
    })
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Notes that mder just heard from the update server.
fn note_checked() {
    let mut settings = Settings::load();
    settings.last_update_check = now();
    let _ = settings.save();
}

/// Starts the daily check in the background, if it's on and due. Read the
/// answer with `daily_result` once it arrives.
pub fn start_daily_check() -> Option<Receiver<Result<Vec<Entry>, String>>> {
    let settings = Settings::load();
    if !settings.check_for_updates || now().saturating_sub(settings.last_update_check) < DAILY {
        return None;
    }
    // Several files opened at once start several windows together; only
    // the first of them checks.
    let lock = std::env::temp_dir().join("mder-update-check.lock");
    let recent = std::fs::metadata(&lock)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|m| m.elapsed().ok())
        .is_some_and(|age| age.as_secs() < CHECK_LOCK_SECS);
    if recent || std::fs::write(&lock, b"").is_err() {
        return None;
    }
    let (send, receive) = mpsc::channel();
    let source = settings.update_source;
    std::thread::spawn(move || {
        let _ = send.send(update::fetch_changelog(source.as_deref()));
    });
    Some(receive)
}

/// What the daily check found: a newer version, unless it's the one you
/// chose to ignore. A failed check says nothing, and tries again next time.
pub fn daily_result(result: Result<Vec<Entry>, String>) -> Option<Offer> {
    let entries = result.ok()?;
    note_checked();
    let offer = offer(&entries)?;
    let ignored = Settings::load().ignored_version;
    if !ignored.is_empty() && !update::is_newer(&offer.latest, &ignored) {
        return None;
    }
    Some(offer)
}

/// Help, Check for update: always says what it found. Returns a newer
/// version to offer, even one you ignored.
pub fn check_now(parent: &Frame) -> Option<Offer> {
    let settings = Settings::load();
    let since = now().saturating_sub(settings.last_update_check);
    if since < MANUAL_WAIT {
        let wait = MANUAL_WAIT - since;
        let (minutes, seconds) = (wait / 60, wait % 60);
        let mut time = String::new();
        if minutes > 0 {
            time.push_str(&format!("{minutes} {} and ", if minutes == 1 { "minute" } else { "minutes" }));
        }
        time.push_str(&format!("{seconds} {}", if seconds == 1 { "second" } else { "seconds" }));
        tell(parent, &format!("Please wait another {time} before checking again."), MessageDialogStyle::IconInformation);
        return None;
    }

    let source = settings.update_source;
    let result = with_progress(parent, "Update Check", "Checking for updates...", None, move |_, _| {
        update::fetch_changelog(source.as_deref())
    })?;
    match result {
        Err(e) => {
            tell(parent, &format!("Update check failed: {e}"), MessageDialogStyle::IconError);
            None
        }
        Ok(entries) => {
            note_checked();
            let found = offer(&entries);
            if found.is_none() {
                let mut message = format!("You are running the latest version ({VERSION})");
                if let Some(date) = update::release_date(&entries, VERSION) {
                    message.push_str(&format!(", released {date}"));
                }
                message.push('.');
                tell(parent, &message, MessageDialogStyle::IconInformation);
            }
            found
        }
    }
}

/// The answer to the update dialog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Choice {
    Install,
    Ignore,
    Later,
    /// Another window installed it while this one asked.
    InstalledElsewhere,
}

/// "A new update is available!", with what changed since your version.
/// Ignore Version is remembered here. If another window of this mder
/// installs the update meanwhile (after `opened`), the dialog closes.
pub fn ask(parent: &Frame, offer: &Offer, opened: SystemTime) -> Choice {
    let dialog = Dialog::builder(parent, "mder update")
        .with_style(DialogStyle::DefaultDialogStyle | DialogStyle::ResizeBorder)
        .with_size(600, 480)
        .build();
    let sizer = BoxSizer::builder(Orientation::Vertical).build();

    let heading = StaticText::builder(&dialog).with_label("A new update is available!").build();
    sizer.add(&heading, 0, SizerFlag::All, 10);
    let label = StaticText::builder(&dialog)
        .with_label("Changes between your version and the new version:")
        .build();
    sizer.add(&label, 0, SizerFlag::Left | SizerFlag::Right, 10);
    let changes = TextCtrl::builder(&dialog)
        .with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly)
        .build();
    changes.set_value(&offer.text);
    changes.set_accessibility_label("Changes between your version and the new version");
    sizer.add(&changes, 1, SizerFlag::Expand | SizerFlag::All, 10);

    let row = BoxSizer::builder(Orientation::Horizontal).build();
    let install = Button::builder(&dialog).with_id(ID_OK).with_label("&Install Now").build();
    let ignore = Button::builder(&dialog).with_id(ID_IGNORE).with_label("I&gnore Version").build();
    let later = Button::builder(&dialog).with_id(ID_CANCEL).with_label("Maybe &Later").build();
    row.add(&install, 0, SizerFlag::Right, 8);
    row.add(&ignore, 0, SizerFlag::Right, 8);
    row.add_stretch_spacer(1);
    row.add(&later, 0, SizerFlag::empty(), 0);
    sizer.add_sizer(&row, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right, 10);

    let mut running = format!("You are running version {VERSION}");
    if let Some(date) = &offer.current_date {
        running.push_str(&format!(", released {date}"));
    }
    let footer = StaticText::builder(&dialog).with_label(&running).build();
    sizer.add(&footer, 0, SizerFlag::All, 10);
    dialog.set_sizer(sizer, true);

    for (button, id) in [(install, ID_OK), (ignore, ID_IGNORE), (later, ID_CANCEL)] {
        button.on_click(move |_| dialog.end_modal(id));
    }
    install.set_default();
    // Start at the top of the changes, ready to read.
    changes.set_insertion_point(0);
    changes.set_focus();

    let watch = Timer::new(&dialog);
    watch.on_tick(move |_| {
        if update::updated_since(opened) {
            dialog.end_modal(ID_INSTALLED_ELSEWHERE);
        }
    });
    watch.start(1000, false);

    let answer = dialog.show_modal();
    drop(watch);
    dialog.destroy();
    match answer {
        ID_OK => Choice::Install,
        ID_INSTALLED_ELSEWHERE => Choice::InstalledElsewhere,
        ID_IGNORE => {
            let mut settings = Settings::load();
            settings.ignored_version = offer.latest.clone();
            let _ = settings.save();
            Choice::Ignore
        }
        _ => Choice::Later,
    }
}

/// Downloads the new version and puts it in place of this one. True once
/// it's there, for the caller to restart on it; false if it failed or you
/// cancelled, having said why.
pub fn download_and_install(parent: &Frame, offer: &Offer) -> bool {
    if !update::can_replace_itself() {
        let answer = MessageDialog::builder(
            parent,
            "mder can't replace itself where it is, because that folder can't be written to. \
             Open the download page, to download the new version yourself?",
            "mder update",
        )
        .with_style(MessageDialogStyle::YesNo | MessageDialogStyle::IconInformation)
        .build()
        .show_modal();
        if answer == ID_YES {
            wxdragon::utils::launch_default_browser(update::DOWNLOAD_PAGE, BrowserLaunchFlags::Default);
        }
        return false;
    }

    let source = Settings::load().update_source;
    let version = offer.latest.clone();
    let result = with_progress(
        parent,
        "mder update",
        &format!("Downloading mder {version}..."),
        Some(100),
        move |cancelled, progress| {
            update::download(source.as_deref(), &version, |done, total| {
                let _ = progress.send((done, total));
                !cancelled.load(Ordering::Relaxed)
            })
        },
    );
    let downloaded = match result {
        None | Some(Ok(None)) => return false,
        Some(Err(e)) => {
            tell(parent, &format!("Installation failed: {e}"), MessageDialogStyle::IconError);
            return false;
        }
        Some(Ok(Some(path))) => path,
    };
    match update::install(&downloaded) {
        Ok(()) => {
            let _ = std::fs::remove_file(&downloaded);
            true
        }
        Err(e) => {
            tell(parent, &format!("Installation failed: {e}"), MessageDialogStyle::IconError);
            false
        }
    }
}

/// Runs `work` on another thread behind a progress dialog with Cancel.
/// `work` gets a flag that Cancel sets, and somewhere to report bytes done
/// and the total; with no `range`, the bar just shows activity. None if
/// you cancelled.
fn with_progress<T: Send + 'static>(
    parent: &Frame,
    title: &str,
    message: &str,
    range: Option<i32>,
    work: impl FnOnce(Arc<AtomicBool>, mpsc::Sender<(u64, Option<u64>)>) -> T + Send + 'static,
) -> Option<T> {
    let cancelled = Arc::new(AtomicBool::new(false));
    let (progress_send, progress) = mpsc::channel();
    let (done_send, done) = mpsc::channel();
    {
        let cancelled = cancelled.clone();
        std::thread::spawn(move || {
            let _ = done_send.send(work(cancelled, progress_send));
        });
    }
    let dialog = ProgressDialog::builder(parent, title, message, range.unwrap_or(100))
        .with_style(ProgressDialogStyle::AppModal | ProgressDialogStyle::CanAbort | ProgressDialogStyle::AutoHide)
        .build();
    let mut percent = 0;
    loop {
        let carry_on = match range {
            Some(_) => dialog.update(percent.min(99), None),
            None => dialog.pulse(None),
        };
        if !carry_on {
            cancelled.store(true, Ordering::Relaxed);
            return None;
        }
        match done.recv_timeout(Duration::from_millis(100)) {
            Ok(result) => return Some(result),
            Err(RecvTimeoutError::Disconnected) => return None,
            Err(RecvTimeoutError::Timeout) => {}
        }
        while let Ok((bytes, total)) = progress.try_recv() {
            if let Some(total) = total.filter(|&t| t > 0) {
                percent = (bytes * 100 / total) as i32;
            }
        }
    }
}

fn tell(parent: &Frame, message: &str, icon: MessageDialogStyle) {
    let title = if message.starts_with("Installation") { "mder update" } else { "Update Check" };
    MessageDialog::builder(parent, message, title)
        .with_style(MessageDialogStyle::OK | icon)
        .build()
        .show_modal();
}
