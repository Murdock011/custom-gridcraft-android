//! Recovery copies of unsaved workbooks, so work survives Android closing GridCraft in the background.
//!
//! A copy is `doc-N.xlsx` (the workbook) and `doc-N.txt` (two lines: the display name, then the
//! document's content location if it has one). They live in `<app files>/recovery`. `save_all`
//! makes the folder match the dirty documents; `load_all` reads them back at launch. `app.rs`
//! decides when to sync (background, a save, a close) and never lets a failure block editing.

use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

/// Edits must stop this long before the recovery copies are written (see `write_due`).
pub const IDLE: Duration = Duration::from_secs(2);

/// Flush requests from Kotlin: `ASKED` counts them, `DONE` is the last one the UI has written.
static ASKED: AtomicU64 = AtomicU64::new(0);
static DONE: Mutex<u64> = Mutex::new(0);
static DONE_CV: Condvar = Condvar::new();

/// Asks the UI to write the copies now, wakes it with `wake`, and waits up to `timeout` for it.
/// A UI that never runs a frame (the window is gone) makes this wait the full `timeout`.
pub fn request_and_wait(timeout: Duration, wake: impl FnOnce()) {
    let n = ASKED.fetch_add(1, Ordering::SeqCst) + 1;
    wake();
    let Ok(done) = DONE.lock() else { return };
    let _ = DONE_CV.wait_timeout_while(done, timeout, |d| *d < n);
}

/// The newest flush request not yet written, if any. Pass it to `flush_done` once the copies are written.
pub fn flush_requested() -> Option<u64> {
    let asked = ASKED.load(Ordering::SeqCst);
    let done = DONE.lock().map_or(asked, |d| *d);
    (asked > done).then_some(asked)
}

/// Marks the requests up to `n` written and releases the waiting Kotlin thread.
pub fn flush_done(n: u64) {
    if let Ok(mut done) = DONE.lock() {
        *done = (*done).max(n);
        DONE_CV.notify_all();
    }
}

/// Whether the copies should be written now. `state` lists the dirty documents as (uid, revision),
/// `written` is what the copies on disk hold, and `changed_at` is when `state` last changed.
/// Writes only when the state differs and it has been unchanged for `IDLE`. A clean state (no dirty
/// documents) differs from a written one that had copies, so the stale copies are removed too.
pub fn write_due(state: &[(u64, u64)], written: &[(u64, u64)], changed_at: Instant, now: Instant) -> bool {
    state != written && now.saturating_duration_since(changed_at) >= IDLE
}

/// One unsaved workbook: the bytes are an `.xlsx` file.
#[derive(Debug, PartialEq)]
pub struct Copy {
    pub name: String,
    pub location: Option<String>,
    pub bytes: Vec<u8>,
}

/// `<app files>/recovery`, or `None` before the app's files folder is known.
pub fn dir() -> Option<std::path::PathBuf> {
    crate::files::DIR.get().map(|d| d.join("recovery"))
}

/// Makes `dir` hold exactly `copies`: copy N goes to `doc-N`, and any other `doc-*` file is removed.
/// Each file is written under a temporary name first, so a failed write keeps the old copy.
pub fn save_all(dir: &Path, copies: &[Copy]) -> io::Result<()> {
    if !copies.is_empty() {
        std::fs::create_dir_all(dir)?;
    }
    for (i, c) in copies.iter().enumerate() {
        let (xlsx, note) = slot(dir, i);
        let text = format!("{}\n{}\n", tidy(&c.name), tidy(c.location.as_deref().unwrap_or("")));
        write_atomic(&xlsx, &c.bytes)?;
        write_atomic(&note, text.as_bytes())?;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return Ok(()) };
    for entry in entries.flatten() {
        let file = entry.file_name();
        if parse(&file.to_string_lossy()).is_some_and(|(n, _)| n >= copies.len()) {
            std::fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

/// The copies in `dir`, in order. A copy whose workbook can't be read is skipped.
pub fn load_all(dir: &Path) -> Vec<Copy> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut indices: Vec<usize> =
        entries.flatten().filter_map(|e| parse(&e.file_name().to_string_lossy()).filter(|(_, ext)| *ext == ".xlsx").map(|(n, _)| n)).collect();
    indices.sort_unstable();
    indices
        .into_iter()
        .filter_map(|i| {
            let (xlsx, note) = slot(dir, i);
            let bytes = std::fs::read(&xlsx).ok()?;
            let text = std::fs::read_to_string(&note).unwrap_or_default();
            let mut lines = text.lines();
            let name = lines.next().filter(|s| !s.trim().is_empty()).unwrap_or("Book1.xlsx").to_string();
            let location = lines.next().filter(|s| !s.is_empty()).map(str::to_string);
            Some(Copy { name, location, bytes })
        })
        .collect()
}

fn slot(dir: &Path, i: usize) -> (std::path::PathBuf, std::path::PathBuf) {
    (dir.join(format!("doc-{i}.xlsx")), dir.join(format!("doc-{i}.txt")))
}

/// The copy number and the rest of a `doc-N.ext` file name (`".xlsx"`), or `None` for any other name.
/// Temporary files start with a dot, so they never match.
fn parse(file: &str) -> Option<(usize, &str)> {
    let rest = file.strip_prefix("doc-")?;
    let digits = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    let n = rest[..digits].parse().ok()?;
    let ext = &rest[digits..];
    ext.starts_with('.').then_some((n, ext))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let file = path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = path.with_file_name(format!(".{file}.tmp"));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// Newlines and other control characters become spaces, so each note line stays one line.
fn tidy(s: &str) -> String {
    s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh folder under the system temp dir (the tests share no state).
    fn scratch(tag: &str) -> std::path::PathBuf {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("gc-recovery-{tag}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn copy(name: &str, location: Option<&str>, bytes: &[u8]) -> Copy {
        Copy { name: name.into(), location: location.map(Into::into), bytes: bytes.to_vec() }
    }

    #[test]
    fn copies_round_trip_with_their_name_location_and_bytes() {
        let dir = scratch("round");
        let copies = vec![copy("quokka.xlsx", Some("content://a/doc/1/quokka.xlsx"), b"one"), copy("Book1.xlsx", None, b"two")];
        save_all(&dir, &copies).expect_or_fail();
        assert_eq!(load_all(&dir), copies);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_empty_set_removes_every_copy_and_keeps_other_files() {
        let dir = scratch("empty");
        save_all(&dir, &[copy("a.xlsx", None, b"x"), copy("b.xlsx", None, b"y")]).expect_or_fail();
        std::fs::write(dir.join("notes.txt"), b"keep").expect_or_fail();
        save_all(&dir, &[]).expect_or_fail();
        assert!(load_all(&dir).is_empty());
        assert!(dir.join("notes.txt").exists(), "only copy files are removed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_smaller_set_drops_the_extra_copies() {
        let dir = scratch("smaller");
        save_all(&dir, &[copy("a.xlsx", None, b"a"), copy("b.xlsx", None, b"b"), copy("c.xlsx", None, b"c")]).expect_or_fail();
        save_all(&dir, &[copy("b.xlsx", None, b"b2")]).expect_or_fail();
        assert_eq!(load_all(&dir), vec![copy("b.xlsx", None, b"b2")]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_newline_in_the_name_cannot_split_the_note() {
        let dir = scratch("tidy");
        save_all(&dir, &[copy("odd\nname.xlsx", Some("content://x/y/odd name.xlsx"), b"z")]).expect_or_fail();
        let loaded = load_all(&dir);
        assert_eq!(loaded[0].name, "odd name.xlsx");
        assert_eq!(loaded[0].location.as_deref(), Some("content://x/y/odd name.xlsx"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn only_copy_file_names_are_parsed() {
        assert_eq!(parse("doc-12.xlsx"), Some((12, ".xlsx")));
        assert_eq!(parse("doc-0.txt"), Some((0, ".txt")));
        assert_eq!(parse(".doc-1.xlsx.tmp"), None, "temporary files are not copies");
        assert_eq!(parse("doc-x.xlsx"), None);
        assert_eq!(parse("doc-3"), None, "needs an extension");
        assert_eq!(load_all(Path::new("/nonexistent/gc-recovery")), Vec::new(), "a missing folder has no copies");
    }

    #[test]
    fn writes_wait_for_two_seconds_without_edits() {
        // A base instant only: the test never reads the clock, it adds offsets to this.
        #[allow(clippy::disallowed_methods)]
        let t0 = Instant::now();
        let dirty = [(1, 5)];
        let at = |secs: u64| t0 + Duration::from_secs(secs);
        assert!(!write_due(&dirty, &[], t0, at(1)), "one second after an edit: wait");
        assert!(write_due(&dirty, &[], t0, at(2)), "two seconds idle: write");
        assert!(!write_due(&dirty, &dirty, t0, at(9)), "copies already match: nothing to write");
        assert!(!write_due(&[(1, 6)], &dirty, at(10), at(11)), "a new edit restarts the wait");
        assert!(write_due(&[(1, 6)], &dirty, at(10), at(12)), "the newer revision is written after idle");
        assert!(write_due(&[], &dirty, t0, at(2)), "a clean state removes the old copies");
        assert!(!write_due(&[], &[], t0, at(9)), "clean and no copies: nothing to do");
    }

    #[test]
    fn a_flush_request_is_pending_until_it_is_done() {
        // Only this test touches the flush counters, so the state it reads is its own.
        let n = ASKED.fetch_add(1, Ordering::SeqCst) + 1;
        assert_eq!(flush_requested(), Some(n));
        flush_done(n);
        assert_eq!(flush_requested(), None);
    }

    /// Fails the test on an I/O error (the crate's lints forbid `unwrap` and `panic!`, even in tests).
    trait ExpectOrFail {
        fn expect_or_fail(self);
    }

    impl<T, E: std::fmt::Debug> ExpectOrFail for Result<T, E> {
        fn expect_or_fail(self) {
            assert!(self.is_ok(), "io error in test: {:?}", self.err());
        }
    }
}
