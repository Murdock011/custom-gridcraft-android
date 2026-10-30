//! Text from the soft keyboard. `MainActivity.kt`'s ImeView pushes what the keyboard sent (through
//! the JNI exports in `android.rs`); `app.rs` turns it into egui events on the next frame.
//!
//! The editing context Kotlin reads back is the text around the caret of the edit when the keyboard's target
//! changes (`retarget`); Kotlin then keeps its own copy until the next target change. See `notes/native-port.md`.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};

/// One message from the keyboard. `kind` in the JNI export: 0 commit, 1 composing text,
/// 2 finish composing, 3 delete around the caret, 4 key press, 5 composing region (ignored).
#[derive(Debug, PartialEq)]
pub enum Msg {
    Commit(String),
    Compose(String),
    Finish,
    Delete { before: usize, after: usize },
    Key(i32),
}

static QUEUE: Mutex<VecDeque<Msg>> = Mutex::new(VecDeque::new());
/// Messages Rust has taken so far. Kotlin only trusts the context when this matches its own count.
static TAKEN: AtomicU64 = AtomicU64::new(0);

/// Changes whenever the keyboard's target changes (the active cell moved, or another sheet or document came up).
/// Kotlin starts its copy of the text over on a change; it keeps its own copy in between (see `context`).
/// Bumped only by `retarget`, under the `SEED` lock.
static TARGET: AtomicU64 = AtomicU64::new(0);
/// The text Kotlin starts from on a target change, and its caret in UTF-16 units. Set by `retarget`.
static SEED: Mutex<(String, usize)> = Mutex::new((String::new(), 0));
/// Characters kept either side of the caret in the seed; a longer cell is clipped around the caret.
const SEED_SIDE: usize = 200;
/// Whether egui wants the soft keyboard (a text position has focus). Kotlin polls it.
pub static WANTS_KEYBOARD: AtomicBool = AtomicBool::new(false);
/// Set by Enter or Tab from the keyboard, so it stays up for the next cell; cleared by a touch, a scroll, or Escape.
pub static KEEP_TYPING: AtomicBool = AtomicBool::new(false);
/// Whether File › Close (or the quit prompt) ended the app: Kotlin finishes the activity (`SheetApp::quit_requested`).
pub static QUIT: AtomicBool = AtomicBool::new(false);
/// Whether Android Back has a sheet, palette or Backstage to close (`SheetApp::can_go_back`).
pub static CAN_GO_BACK: AtomicBool = AtomicBool::new(false);
/// The GPU could not be set up (no adapter, or the window surface failed before the first frame). Kotlin
/// shows a dialog for it (`nativeGpuFailed`).
pub static GPU_FAILED: AtomicBool = AtomicBool::new(false);
/// Whether eframe has run the first `ui` call, i.e. the window surface was set up (see `android::gpu_setup`).
pub static UI_STARTED: AtomicBool = AtomicBool::new(false);
/// The system's night mode, from Kotlin.
pub static NIGHT: AtomicBool = AtomicBool::new(false);
/// Left and top padding of the surface in pixels (set by Kotlin); touch positions arrive in window
/// pixels, so the host subtracts it.
pub static INSET_LEFT: AtomicI32 = AtomicI32::new(0);
pub static INSET_TOP: AtomicI32 = AtomicI32::new(0);

/// Wakes the event loop, which sleeps until something arrives. Set on the first frame.
pub static WAKE: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

pub fn wake() {
    if let Some(f) = WAKE.get() {
        f();
    }
}

/// Queues a message from Kotlin (JNI thread) and wakes the event loop.
pub fn push(m: Msg) {
    if let Ok(mut q) = QUEUE.lock() {
        q.push_back(m);
    }
    wake();
}

/// The oldest queued message, if any.
/// Splits keyboard commit text at newlines ("\n", "\r\n" or a lone "\r"); the pieces are the lines between Enter presses.
pub fn ime_lines(text: &str) -> Vec<&str> {
    text.split("\r\n").flat_map(|s| s.split(['\r', '\n'])).collect()
}

pub fn pop() -> Option<Msg> {
    let m = QUEUE.lock().ok()?.pop_front()?;
    TAKEN.fetch_add(1, Ordering::Relaxed);
    Some(m)
}

/// Moves the keyboard to a new target: `text` is what the edit holds (empty when typing will replace the cell),
/// `caret` its caret in characters. Kotlin starts its copy from this on the next pull.
pub fn retarget(text: &str, caret: usize) {
    let caret = caret.min(text.chars().count());
    let start = caret.saturating_sub(SEED_SIDE);
    let end = (caret + SEED_SIDE).min(text.chars().count());
    let chars: Vec<char> = text.chars().collect();
    let clean = |cs: &[char]| cs.iter().map(|c| if *c == '\u{1}' { ' ' } else { *c }).collect::<String>();
    let before = clean(&chars[start..caret]);
    let seed = (format!("{before}{}", clean(&chars[caret..end])), before.encode_utf16().count());
    let mut s = SEED.lock().unwrap_or_else(|e| e.into_inner());
    *s = seed;
    TARGET.fetch_add(1, Ordering::Relaxed);
}

/// What Kotlin reads: the number of messages taken, caret (UTF-16 units), composing start and end (-1 for none),
/// the target counter (`TARGET`), then the text around the caret, U+0001 separated. The text and caret are the
/// seed from `retarget`; Kotlin keeps its own copy while the target stays the same.
pub fn context() -> String {
    let seed = SEED.lock().unwrap_or_else(|e| e.into_inner());
    format!("{}\u{1}{}\u{1}-1\u{1}-1\u{1}{}\u{1}{}", TAKEN.load(Ordering::Relaxed), seed.1, TARGET.load(Ordering::Relaxed), seed.0)
}

#[cfg(test)]
mod tests {
    use super::{context, ime_lines, retarget};

    #[test]
    fn commit_newlines_split_into_lines() {
        assert_eq!(ime_lines("Hello"), ["Hello"]);
        assert_eq!(ime_lines("ab\ncd"), ["ab", "cd"]);
        assert_eq!(ime_lines("ab\r\ncd\re"), ["ab", "cd", "e"]);
        assert_eq!(ime_lines("\n"), ["", ""]);
    }

    /// The seed is the text and caret in UTF-16 units (an emoji is two), and a long cell is clipped around the caret.
    #[test]
    fn retarget_seeds_text_and_utf16_caret() {
        let field = |c: &str| c.split('\u{1}').map(str::to_string).collect::<Vec<_>>();
        retarget("Plan", 4);
        let f = field(&context());
        assert_eq!((f[1].as_str(), f[5].as_str()), ("4", "Plan"));
        retarget("a\u{1f600}b", 2);
        let f = field(&context());
        assert_eq!((f[1].as_str(), f[5].as_str()), ("3", "a\u{1f600}b"));
        retarget(&"x".repeat(500), 300);
        let f = field(&context());
        assert_eq!((f[1].as_str(), f[5].len()), ("200", 400));
    }
}
