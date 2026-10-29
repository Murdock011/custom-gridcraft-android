//! Android entry point and the JNI exports that `MainActivity.kt` calls.
//!
//! The exports only push onto the `ime` queue or read an atomic flag, so they never block the UI
//! thread. The workspace forbids `unsafe`; the only unsafe here is the `no_mangle` attribute that
//! JNI symbol names need.

use android_activity::AndroidApp;
use jni::EnvUnowned;
use jni::objects::{JObject, JString};
use jni::sys::{jboolean, jint, jlong};
use std::sync::atomic::Ordering;

use crate::files;
use crate::ime::{self, Msg};

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("gridcraft"));
    if let Some(dir) = app.internal_data_path() {
        let _ = files::DIR.set(dir);
    }
    let options = eframe::NativeOptions { android_app: Some(app), ..Default::default() };
    let result = eframe::run_native(
        "GridCraft",
        options,
        Box::new(|cc| Ok(Box::new(crate::app::Shell::new(&cc.egui_ctx)) as Box<dyn eframe::App>)),
    );
    if let Err(e) = result {
        log::error!("eframe stopped: {e}");
    }
}

/// `ImeView` -> Rust. `kind`: 0 commit, 1 composing text, 2 finish composing, 3 delete surrounding
/// (`a` before, `b` after, in code points), 4 key press (`a` is the KeyEvent code), 5 composing
/// region (`a`..`b`, UTF-16 offsets into the context text).
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_storyteller_gridcraft_nativeapp_MainActivity_nativeIme<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    kind: jint,
    text: JString<'local>,
    a: jint,
    b: jint,
) {
    let text: String = env.with_env(|env| text.try_to_string(env)).resolve::<jni::errors::ThrowRuntimeExAndDefault>();
    let msg = match kind {
        0 => Msg::Commit(text),
        1 => Msg::Compose(text),
        2 => Msg::Finish,
        3 => Msg::Delete { before: a.max(0) as usize, after: b.max(0) as usize },
        4 => Msg::Key(a),
        _ => return,
    };
    ime::push(msg);
}

/// The text around the caret for the input method (see `ime::context`).
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_storyteller_gridcraft_nativeapp_MainActivity_nativeContext<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
) -> JString<'local> {
    env.with_env(|env| env.new_string(ime::context())).resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

/// Whether egui wants the soft keyboard. Kotlin polls it to show or hide the keyboard.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_storyteller_gridcraft_nativeapp_MainActivity_nativeWantsKeyboard<'local>(
    _env: EnvUnowned<'local>,
    _this: JObject<'local>,
) -> jboolean {
    ime::WANTS_KEYBOARD.load(Ordering::Relaxed) as jboolean
}

/// Whether the cell is being edited: the same as wanting the keyboard, as there is no separate
/// canvas here (Kotlin uses it to decide whether to show the keyboard on resume).
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_storyteller_gridcraft_nativeapp_MainActivity_nativeCanvasEditing<'local>(
    _env: EnvUnowned<'local>,
    _this: JObject<'local>,
) -> jboolean {
    ime::WANTS_KEYBOARD.load(Ordering::Relaxed) as jboolean
}

/// Whether Close asked to end the app. GridCraft has no unsaved-changes prompt on Android yet.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_storyteller_gridcraft_nativeapp_MainActivity_nativeQuitRequested<'local>(
    _env: EnvUnowned<'local>,
    _this: JObject<'local>,
) -> jboolean {
    false as jboolean
}

/// Whether Android Back has a sheet, palette or Backstage to close. Kotlin sends Escape then.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_storyteller_gridcraft_nativeapp_MainActivity_nativeCanGoBack<'local>(
    _env: EnvUnowned<'local>,
    _this: JObject<'local>,
) -> jboolean {
    ime::CAN_GO_BACK.load(Ordering::Relaxed) as jboolean
}

/// The next file request for Kotlin (see `files::take_request`); empty when there is none.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_storyteller_gridcraft_nativeapp_MainActivity_nativeTakeRequest<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
) -> JString<'local> {
    env.with_env(|env| env.new_string(files::take_request())).resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

/// A file operation failed on the Kotlin side: say why on the status line.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_storyteller_gridcraft_nativeapp_MainActivity_nativeNotice<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    msg: JString<'local>,
) {
    let msg: String = env.with_env(|env| msg.try_to_string(env)).resolve::<jni::errors::ThrowRuntimeExAndDefault>();
    files::notice(msg);
}

/// The user picked the document `name` at content URI `uri`; its bytes are in the cache file at `path`.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_storyteller_gridcraft_nativeapp_MainActivity_nativeOpened<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    name: JString<'local>,
    path: JString<'local>,
    uri: JString<'local>,
) {
    let name: String = env.with_env(|env| name.try_to_string(env)).resolve::<jni::errors::ThrowRuntimeExAndDefault>();
    let path: String = env.with_env(|env| path.try_to_string(env)).resolve::<jni::errors::ThrowRuntimeExAndDefault>();
    let uri: String = env.with_env(|env| uri.try_to_string(env)).resolve::<jni::errors::ThrowRuntimeExAndDefault>();
    files::opened(name, path, uri);
}

/// A save ended (see `files::save_done`): status 1 saved (`msg` is the URI, `display` its name),
/// 0 cancelled, anything else failed with `msg`.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_storyteller_gridcraft_nativeapp_MainActivity_nativeSaveDone<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    ticket: jlong,
    status: jint,
    msg: JString<'local>,
    display: JString<'local>,
) {
    let msg: String = env.with_env(|env| msg.try_to_string(env)).resolve::<jni::errors::ThrowRuntimeExAndDefault>();
    let display: String = env.with_env(|env| display.try_to_string(env)).resolve::<jni::errors::ThrowRuntimeExAndDefault>();
    files::save_done(ticket as u64, status, msg, display);
}

/// Whether GridCraft opens a document named `name` (the MIME type is not needed: the name decides).
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_storyteller_gridcraft_nativeapp_MainActivity_nativeAccepts<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    name: JString<'local>,
    _mime: JString<'local>,
) -> jboolean {
    let name: String = env.with_env(|env| name.try_to_string(env)).resolve::<jni::errors::ThrowRuntimeExAndDefault>();
    gridcraft_engine::io::FileKind::from_path(&name).is_some() as jboolean
}

/// The activity is being backgrounded: the next frame writes recovery copies of unsaved workbooks.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_storyteller_gridcraft_nativeapp_MainActivity_nativeFlushRecovery<'local>(
    _env: EnvUnowned<'local>,
    _this: JObject<'local>,
) {
    // Writes the latest copies on this thread: no frame is needed, so this works while the window is gone.
    crate::app::write_pending();
}

/// The next copy for the system clipboard, prefixed with `c`; empty when there is none.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_storyteller_gridcraft_nativeapp_MainActivity_nativeTakeClip<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
) -> JString<'local> {
    env.with_env(|env| env.new_string(crate::clip::take_outgoing())).resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

/// The system clipboard's text changed (or the window regained focus).
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_storyteller_gridcraft_nativeapp_MainActivity_nativeClip<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    text: JString<'local>,
) {
    let text: String = env.with_env(|env| text.try_to_string(env)).resolve::<jni::errors::ThrowRuntimeExAndDefault>();
    crate::clip::from_system(text);
}

/// The system's night mode, at start and on each configuration change.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_storyteller_gridcraft_nativeapp_MainActivity_nativeSetNight<'local>(
    _env: EnvUnowned<'local>,
    _this: JObject<'local>,
    night: jboolean,
) {
    ime::NIGHT.store(night, Ordering::Relaxed);
    ime::wake();
}

/// Padding that touch positions must be shifted by.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_ai_storyteller_gridcraft_nativeapp_MainActivity_nativeSetInsets<'local>(
    _env: EnvUnowned<'local>,
    _this: JObject<'local>,
    left: jint,
    top: jint,
) {
    ime::INSET_LEFT.store(left, Ordering::Relaxed);
    ime::INSET_TOP.store(top, Ordering::Relaxed);
}
