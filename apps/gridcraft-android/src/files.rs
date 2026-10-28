//! Documents picked with Android's document picker (SAF). Rust asks Kotlin to show the picker; the
//! picked file is copied to a cache file and comes back here as a path. `drain_opened` reads it.
//! Saving goes to the app's own folder for now (see `notes/native-port.md`).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

/// The app's private files folder, set at start-up.
pub static DIR: OnceLock<PathBuf> = OnceLock::new();

/// Set when GridCraft asks for a document; `take_request` hands it to Kotlin once.
static OPEN_REQUESTED: AtomicBool = AtomicBool::new(false);
/// Picked files waiting to be read: (name, cache file path).
static OPENED: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
/// Messages for the status line (for example, a file that couldn't be read).
static NOTICES: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Asks Kotlin to show the document picker.
pub fn request_open() {
    OPEN_REQUESTED.store(true, Ordering::Relaxed);
    crate::ime::wake();
}

/// Kotlin: the next request, `open\tdocument`, or an empty string when there is none.
pub fn take_request() -> String {
    if OPEN_REQUESTED.swap(false, Ordering::Relaxed) { "open\tdocument".to_string() } else { String::new() }
}

/// Kotlin: the user picked a document named `name`; its bytes are in the cache file at `path`.
pub fn opened(name: String, path: String) {
    if let Ok(mut v) = OPENED.lock() {
        v.push((name, path));
    }
    crate::ime::wake();
}

/// Kotlin or Rust: say `msg` on the status line.
pub fn notice(msg: String) {
    if let Ok(mut n) = NOTICES.lock() {
        n.push(msg);
    }
    crate::ime::wake();
}

pub fn drain_notices() -> Vec<String> {
    NOTICES.lock().map(|mut n| n.drain(..).collect()).unwrap_or_default()
}

/// Picked documents as (name, bytes). The cache files are removed; unreadable ones are reported.
pub fn drain_opened() -> Vec<(String, Vec<u8>)> {
    let picked: Vec<(String, String)> = OPENED.lock().map(|mut v| v.drain(..).collect()).unwrap_or_default();
    picked
        .into_iter()
        .filter_map(|(name, path)| {
            let bytes = std::fs::read(&path);
            let _ = std::fs::remove_file(&path);
            match bytes {
                Ok(b) => Some((name, b)),
                Err(e) => {
                    notice(format!("Couldn't read {name}: {e}"));
                    None
                }
            }
        })
        .collect()
}

/// A file GridCraft saves to the app's folder (Save As until the document picker can write).
pub fn save_download(name: &str, bytes: &[u8]) {
    let Some(dir) = DIR.get() else {
        notice(format!("Couldn't save {name}: no storage folder"));
        return;
    };
    let base = std::path::Path::new(name).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Book1.xlsx".into());
    let path = dir.join(&base);
    match std::fs::write(&path, bytes) {
        Ok(()) => notice(format!("Saved {base} to {}", path.display())),
        Err(e) => notice(format!("Couldn't save {base}: {e}")),
    }
}
