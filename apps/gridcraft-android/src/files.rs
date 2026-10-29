//! Documents through Android's Storage Access Framework (SAF): the picker opens them, the system
//! save dialog creates them, and plain Save writes back to the document's content URI.
//!
//! Rust never calls Kotlin. Rust queues a line (`take_request`); `MainActivity.kt` polls it, shows
//! the system dialog or copies the bytes from a cache file, then reports through `save_done`,
//! `opened` or `notice`.
//!
//! A picked document's *location* is its content URI, a slash, then the provider's display name,
//! as in `content://…/document/…/quokka.xlsx`. The name is the last segment, so the title and the
//! file-type rules work unchanged. Only `.xlsx` documents get a location: other formats are opened
//! as copies and Save asks where to put the workbook.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

/// The app's private files folder, set at start-up. Cache files for Kotlin to copy live here.
pub static DIR: OnceLock<PathBuf> = OnceLock::new();

/// Set when GridCraft asks for a document; `take_request` hands it to Kotlin once.
static OPEN_REQUESTED: AtomicBool = AtomicBool::new(false);
static NEXT_TICKET: AtomicU64 = AtomicU64::new(1);
/// Lines for Kotlin, oldest first: `save\tticket\tname\tpath` or `write\tticket\turi\tpath`.
static REQUESTS: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());
/// Saves Kotlin has not reported on yet.
static FLIGHTS: Mutex<Vec<Flight>> = Mutex::new(Vec::new());
/// Picked documents: (location, cache file path).
static OPENED: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
/// Successful workbook saves: (the workbook revision that was saved, the document's location).
/// The app marks that revision clean and adopts the location (plain Save writes there next time).
static SAVED: Mutex<Vec<(u64, Option<String>)>> = Mutex::new(Vec::new());
/// The active workbook's revision at the start of this frame; a save queued now saves that state.
static SAVE_REV: AtomicU64 = AtomicU64::new(0);
/// Messages for the status line.
static NOTICES: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// A save in flight: its ticket, the name it saves under, the workbook revision it saves, and
/// whether a success makes the file the workbook's location (a `.xlsx`, Save As or write back).
struct Flight {
    ticket: u64,
    name: String,
    rev: u64,
    adopt: bool,
}

/// Called by the app each frame with the active workbook's revision.
pub fn set_revision(rev: u64) {
    SAVE_REV.store(rev, Ordering::Relaxed);
}

/// Asks Kotlin to show the document picker.
pub fn request_open() {
    OPEN_REQUESTED.store(true, Ordering::Relaxed);
    crate::ime::wake();
}

/// Kotlin: the next request, or an empty string when there is none.
pub fn take_request() -> String {
    if OPEN_REQUESTED.swap(false, Ordering::Relaxed) {
        return "open\tdocument".to_string();
    }
    REQUESTS.lock().ok().and_then(|mut q| q.pop_front()).unwrap_or_default()
}

/// The location of the document at `uri` whose provider name is `name`. A slash or control
/// character in the name becomes `_`, so the last slash always starts the name.
pub fn location(uri: &str, name: &str) -> String {
    let name: String = name.chars().map(|c| if c == '/' || c.is_control() { '_' } else { c }).collect();
    let name = if name.trim().is_empty() || name == "." || name == ".." { "document".to_string() } else { name };
    format!("{uri}/{name}")
}

/// The (URI, name) of `loc`, or `None` when it is not a content location.
pub fn parse_location(loc: &str) -> Option<(&str, &str)> {
    let (uri, name) = loc.rsplit_once('/')?;
    (uri.starts_with("content://") && uri.len() > "content://".len() && !name.is_empty()).then_some((uri, name))
}

/// Whether a document named `name` may keep a location (plain Save writes it back in place).
pub fn keeps_location(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".xlsx")
}

fn tidy(name: &str) -> String {
    name.replace(['\t', '\n'], " ")
}

/// Writes `bytes` to a new cache file for Kotlin to copy. Failures go to the status line.
fn stage(name: &str, bytes: &[u8]) -> Option<(u64, String)> {
    let Some(dir) = DIR.get() else {
        notice(format!("Couldn't save {name}: no storage folder"));
        return None;
    };
    let ticket = NEXT_TICKET.fetch_add(1, Ordering::Relaxed);
    let file = dir.join(format!("save-{ticket}.bin"));
    match std::fs::write(&file, bytes) {
        Ok(()) => Some((ticket, file.to_string_lossy().into_owned())),
        Err(e) => {
            notice(format!("Couldn't save {name}: {e}"));
            None
        }
    }
}

fn queue(line: String, flight: Flight) {
    if let Ok(mut f) = FLIGHTS.lock() {
        f.push(flight);
    }
    if let Ok(mut q) = REQUESTS.lock() {
        q.push_back(line);
    }
    crate::ime::wake();
}

/// Save As: the system dialog creates a document named `name` holding `bytes`.
pub fn save_as(name: &str, bytes: &[u8]) {
    let Some((ticket, path)) = stage(name, bytes) else { return };
    let line = format!("save\t{ticket}\t{}\t{path}", tidy(name));
    let rev = SAVE_REV.load(Ordering::Relaxed);
    queue(line, Flight { ticket, name: name.to_string(), rev, adopt: keeps_location(name) });
}

/// Plain Save of a picked document: writes `bytes` over `loc`. Returns `false` when `loc` is not a
/// content location, so the caller can fall back to the engine's own Save.
pub fn save_in_place(loc: &str, bytes: &[u8]) -> bool {
    let Some((uri, name)) = parse_location(loc) else { return false };
    let Some((ticket, path)) = stage(name, bytes) else { return true };
    let line = format!("write\t{ticket}\t{uri}\t{path}");
    let rev = SAVE_REV.load(Ordering::Relaxed);
    queue(line, Flight { ticket, name: name.to_string(), rev, adopt: true });
    true
}

/// Kotlin: save `ticket` ended. Status 1 saved (`msg` is the document URI, `display` its name),
/// 0 cancelled, anything else failed with `msg`.
pub fn save_done(ticket: u64, status: i32, msg: String, display: String) {
    let flight = FLIGHTS.lock().ok().and_then(|mut f| f.iter().position(|x| x.ticket == ticket).map(|i| f.swap_remove(i)));
    let Some(flight) = flight else { return };
    // The revision's outcome for the app: the document's location when saved (empty when it has none),
    // None when cancelled or failed (`app::Shell::finish_save`).
    let outcome = match status {
        1 => {
            notice(format!("Saved {display}"));
            let loc = if flight.adopt && !msg.is_empty() { location(&msg, &display) } else { String::new() };
            Some(loc)
        }
        0 => None,
        _ => {
            notice(format!("Couldn't save {}: {msg}", flight.name));
            None
        }
    };
    if let Ok(mut s) = SAVED.lock() {
        s.push((flight.rev, outcome));
    }
    crate::ime::wake();
}

/// Kotlin: the user picked the document at `uri`; its bytes are in the cache file at `path`.
pub fn opened(name: String, path: String, uri: String) {
    if let Ok(mut v) = OPENED.lock() {
        v.push((location(&uri, &name), path));
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

/// Successful saves since the last call: (revision saved, location).
pub fn drain_saved() -> Vec<(u64, Option<String>)> {
    SAVED.lock().map(|mut s| s.drain(..).collect()).unwrap_or_default()
}

/// Picked documents as (location, bytes). The cache files are removed; unreadable ones are reported.
pub fn drain_opened() -> Vec<(String, Vec<u8>)> {
    let picked: Vec<(String, String)> = OPENED.lock().map(|mut v| v.drain(..).collect()).unwrap_or_default();
    picked
        .into_iter()
        .filter_map(|(loc, path)| {
            let bytes = std::fs::read(&path);
            let _ = std::fs::remove_file(&path);
            match bytes {
                Ok(b) => Some((loc, b)),
                Err(e) => {
                    notice(format!("Couldn't read {loc}: {e}"));
                    None
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locations_keep_the_name_last_and_round_trip() {
        let uri = "content://auth/doc/primary%3ADownload%2Fquokka.xlsx";
        let loc = location(uri, "quokka (1).xlsx");
        assert_eq!(parse_location(&loc), Some((uri, "quokka (1).xlsx")));
        assert_eq!(location("content://a/b", "x/y\tz.xlsx"), "content://a/b/x_y_z.xlsx");
        assert_eq!(location("content://a/b", ".."), "content://a/b/document");
        assert_eq!(parse_location("/data/user/0/app/files/quokka.xlsx"), None, "a file path is not a location");
        assert_eq!(parse_location("content://a"), None, "no name");
    }

    #[test]
    fn a_save_as_of_a_workbook_is_adopted_and_an_export_is_not() {
        let (ok, ex) = (7_000_001, 7_000_002);
        queue("save".into(), Flight { ticket: ok, name: "Book1.xlsx".into(), rev: 5, adopt: keeps_location("Book1.xlsx") });
        queue("save".into(), Flight { ticket: ex, name: "Book1.csv".into(), rev: 5, adopt: keeps_location("Book1.csv") });
        save_done(ok, 1, "content://d/u".into(), "Book1.xlsx".into());
        save_done(ex, 1, "content://d/p".into(), "Book1.csv".into());
        // The export is reported with an empty location: it did not adopt one, and it does not clean the workbook.
        assert_eq!(drain_saved(), vec![(5, Some("content://d/u/Book1.xlsx".to_string())), (5, Some(String::new()))]);
        assert!(drain_notices().iter().any(|n| n == "Saved Book1.csv"));
    }
}
