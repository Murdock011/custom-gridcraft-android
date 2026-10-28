//! Plain-text clipboard shared with Android's `ClipboardManager`.
//!
//! Rust never calls Kotlin: a copy queues its text for `MainActivity.kt`, which polls
//! [`take_outgoing`] and sets the system clip. Kotlin sends the system clip's text through
//! [`from_system`] when it changes or the window regains focus, so Paste reads [`text`] without
//! waiting on the UI thread. No Android or egui dependency, so it runs in host tests.

use std::sync::Mutex;

/// What Paste reads: the last copy here or the last clip Android reported.
static TEXT: Mutex<Option<String>> = Mutex::new(None);
/// A copy that Kotlin has not put on the system clipboard yet.
static OUTGOING: Mutex<Option<String>> = Mutex::new(None);

/// A copy in GridCraft: paste it here at once and hand it to Android.
pub fn copied(t: String) {
    if let Ok(mut v) = TEXT.lock() {
        *v = Some(t.clone());
    }
    if let Ok(mut v) = OUTGOING.lock() {
        *v = Some(t);
    }
}

/// Kotlin: the system clipboard's text (another app's copy, or our own echoed back).
pub fn from_system(t: String) {
    if let Ok(mut v) = TEXT.lock() {
        *v = Some(t);
    }
}

/// The text Paste inserts.
pub fn text() -> Option<String> {
    TEXT.lock().ok().and_then(|v| v.clone())
}

/// For Kotlin's poll: the copy to set on the system clipboard, prefixed with `c`, or empty when
/// there is none (a copied picture sets empty text, which must still clear the clip).
pub fn take_outgoing() -> String {
    OUTGOING.lock().ok().and_then(|mut v| v.take()).map(|t| format!("c{t}")).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_reaches_paste_and_android_once_then_system_clip_wins() {
        copied(String::new());
        assert_eq!(take_outgoing(), "c", "an empty copy still clears the system clip");
        copied("Zebra".into());
        assert_eq!(text().as_deref(), Some("Zebra"));
        assert_eq!(take_outgoing(), "cZebra");
        assert_eq!(take_outgoing(), "");
        from_system("from Settings".into());
        assert_eq!(text().as_deref(), Some("from Settings"));
    }
}
