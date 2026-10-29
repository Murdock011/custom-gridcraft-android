//! Text from the soft keyboard. `MainActivity.kt`'s ImeView pushes what the keyboard sent (through
//! the JNI exports in `android.rs`); `app.rs` turns it into egui events on the next frame.
//!
//! The editing context Kotlin reads back is always empty: the keyboard only sees what it types.
//! That is the simplest thing that works for a spreadsheet cell; see `notes/native-port.md`.

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

/// Whether egui wants the soft keyboard (a text position has focus). Kotlin polls it.
pub static WANTS_KEYBOARD: AtomicBool = AtomicBool::new(false);
/// Whether Android Back has a sheet, palette or Backstage to close (`SheetApp::can_go_back`).
pub static CAN_GO_BACK: AtomicBool = AtomicBool::new(false);
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
pub fn pop() -> Option<Msg> {
    let m = QUEUE.lock().ok()?.pop_front()?;
    TAKEN.fetch_add(1, Ordering::Relaxed);
    Some(m)
}

/// What Kotlin reads: the number of messages taken, caret, composing start and end (-1 for none),
/// then the text around the caret, U+0001 separated. The text is always empty (see the module doc).
pub fn context() -> String {
    format!("{}\u{1}0\u{1}-1\u{1}-1\u{1}", TAKEN.load(Ordering::Relaxed))
}
