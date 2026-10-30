//! The egui side: one `SheetApp` in an eframe window, fed by the soft keyboard and the document
//! picker. Keeps the Android specifics (touch insets, night mode, clipboard) out of the UI crate.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use egui::{Event, Key};
use gridcraft_engine::model::Workbook;
use gridcraft_engine::{DocState, Session, Trust, io};
use gridcraft_ui_egui::{Inbox, Services, SheetApp};

use crate::clip;
use crate::files;
use crate::ime::{self, Msg};
use crate::recovery;

/// Android KeyEvent codes the keyboard sends as key presses.
const KEYCODE_ESCAPE: i32 = 111;
const KEYCODE_DEL: i32 = 67;
const KEYCODE_FORWARD_DEL: i32 = 112;
const KEYCODE_ENTER: i32 = 66;

/// Most keys one `Delete` message may erase, so a hostile count can't flood the queue.
const MAX_ERASE: usize = 256;

/// A dirty document as its copy should hold it. The workbook is shared with the document, not copied.
#[derive(Clone)]
pub struct Pending {
    name: String,
    location: Option<String>,
    wb: Arc<Workbook>,
}

/// What the copies should hold, set whenever the dirty documents change. The pause flush (a JNI call,
/// with no frame to wait for) writes it from its own thread.
static PENDING: Mutex<Vec<Pending>> = Mutex::new(Vec::new());
/// Serializes the writes: the timed write and the pause flush may both run.
static WRITE: Mutex<()> = Mutex::new(());

/// The dirty documents of `session`, as the copies should hold them.
fn pending_docs(session: &Session) -> Vec<Pending> {
    session
        .documents()
        .iter()
        .filter(|d| d.is_dirty())
        .map(|d| Pending {
            name: d.display_title(),
            location: d.path.clone().filter(|l| files::parse_location(l).is_some()),
            wb: d.wb.clone(),
        })
        .collect()
}

/// Writes the pending copies to the recovery folder, from any thread (the pause flush calls it from Java's
/// thread). Failures are logged; editing carries on.
pub fn write_pending() {
    let Some(dir) = recovery::dir() else { return };
    let _write = WRITE.lock().unwrap_or_else(|e| e.into_inner());
    let pending = PENDING.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let mut copies = Vec::new();
    for p in pending {
        match io::save_bytes(&p.wb, "recovered.xlsx") {
            Ok(bytes) => copies.push(recovery::Copy { name: p.name, location: p.location, bytes }),
            Err(e) => log::warn!("recovery copy of {} not encoded: {e}", p.name),
        }
    }
    if let Err(e) = recovery::save_all(&dir, &copies) {
        log::warn!("recovery copies not written: {e}");
    }
}

pub struct Shell {
    app: SheetApp,
    /// Events from the keyboard, handed to egui with the next frame's input.
    pending: Vec<Event>,
    /// The keyboard's composing text, which is replaced as the user types (see `replace_composing`).
    composing: String,
    inbox: Inbox,
    /// Locations of the documents queued for the inbox in this frame (see `logic`).
    opening: Vec<String>,
    /// The dirty documents as (uid, revision) when the copies on disk were last written (`sync_recovery`).
    written: Vec<(u64, u64)>,
    /// The dirty documents as (uid, revision) at the last frame, and when that changed (`recovery::write_due`).
    seen: Vec<(u64, u64)>,
    changed_at: Instant,
}

impl Shell {
    pub fn new(ctx: &egui::Context) -> Shell {
        // Unsaved work from a previous run comes back first; a blank book is made only without it.
        let mut session = Session::new();
        let mut restored = false;
        for c in recovery::dir().map(|d| recovery::load_all(&d)).unwrap_or_default() {
            let Ok((wb, _)) = io::open_bytes("recovered.xlsx", &c.bytes) else {
                log::warn!("recovery copy {} can't be read", c.name);
                continue;
            };
            // Plain Save writes back to the original location, if it was one Save can write to.
            let path = c.location.filter(|l| files::parse_location(l).is_some_and(|(_, n)| files::keeps_location(n)));
            let mut doc = DocState::new(wb, path, c.name, Trust::FromFile);
            // The copy holds changes the file does not: the document starts dirty.
            doc.wb = Arc::new((*doc.wb).clone());
            session.add_document(doc);
            restored = true;
        }
        if !restored {
            session.new_workbook();
        }
        let state: Vec<(u64, u64)> = session.documents().iter().filter(|d| d.is_dirty()).map(|d| (d.uid, d.revision)).collect();
        let inbox = Inbox::default();
        let services = Services {
            open_async: Some(Box::new(files::request_open)),
            inbox: Some(inbox.clone()),
            download: Some(Box::new(|name: &str, bytes: &[u8]| files::save_as(name, bytes))),
            save_in_place: Some(Box::new(|loc: &str, bytes: &[u8], rev: u64| files::save_in_place(loc, bytes, rev))),
            paste_text: Some(Box::new(clip::text)),
            close_quits: true,
            ..Default::default()
        };
        let mut app = SheetApp::new(session, services);
        *PENDING.lock().unwrap_or_else(|e| e.into_inner()) = pending_docs(&app.session);
        SheetApp::setup_context_for_language(ctx, app.ui.dark, app.ui.language);
        app.after_engine();
        if restored {
            app.message = Some(("GridCraft".into(), "Recovered unsaved changes".into()));
        }
        Shell {
            app,
            pending: Vec::new(),
            composing: String::new(),
            inbox,
            opening: Vec::new(),
            written: state.clone(),
            seen: state,
            changed_at: {
                #[allow(clippy::disallowed_methods)] // Android only: the clock is there
                Instant::now()
            },
        }
    }

    /// The dirty documents as (uid, revision).
    fn dirty_state(&self) -> Vec<(u64, u64)> {
        self.app.session.documents().iter().filter(|d| d.is_dirty()).map(|d| (d.uid, d.revision)).collect()
    }

    /// Notes each change to the dirty documents. The pending copies are updated at once, so the pause
    /// flush writes the latest state even when no frame follows the edit. The timed write is scheduled
    /// for when the edits pause.
    fn track_recovery(&mut self, ctx: &egui::Context) {
        #[allow(clippy::disallowed_methods)] // Android only: the clock is there
        let now = Instant::now();
        let state = self.dirty_state();
        if state != self.seen {
            *PENDING.lock().unwrap_or_else(|e| e.into_inner()) = pending_docs(&self.app.session);
            self.seen = state;
            self.changed_at = now;
        }
        if self.seen != self.written {
            ctx.request_repaint_after(recovery::IDLE.saturating_sub(now.saturating_duration_since(self.changed_at)));
        }
    }

    /// Writes the pending copies, and records the state they hold.
    fn sync_recovery(&mut self) {
        write_pending();
        self.written = self.seen.clone();
    }

    /// A document with a copy is now clean or gone, so its copy is out of date.
    fn copy_is_stale(&self) -> bool {
        let docs = self.app.session.documents();
        self.written.iter().any(|(uid, _)| docs.iter().find(|d| d.uid == *uid).is_none_or(|d| !d.is_dirty()))
    }

    /// The document at `loc` lives there now: plain Save writes back to it. Only `.xlsx` keeps a
    /// location (see `files::keeps_location`); the active document is the one that just changed.
    fn adopt(&mut self, loc: &str) {
        let Some((_, name)) = files::parse_location(loc) else { return };
        if !files::keeps_location(name) {
            return;
        }
        if let Some(d) = self.app.session.active_mut() {
            d.path = Some(loc.to_string());
        }
    }

    /// A save of revision `rev` to `loc` succeeded: if the active workbook is still that revision,
    /// it is clean now (the title drops "Edited", as after a desktop save) and lives at `loc`.
    /// Edits made since the save was asked for leave it dirty.
    /// `outcome` is None when the save was cancelled or failed; otherwise the document's new location
    /// (empty when it has none). The app is told whether the workbook was written (`save_finished`).
    fn finish_save(&mut self, ctx: &egui::Context, rev: u64, outcome: Option<String>) {
        // An export (no location) or a failure leaves the workbook as it was, unsaved.
        let loc = match outcome {
            Some(loc) if !loc.is_empty() => loc,
            _ => {
                self.app.save_finished(ctx, rev, false);
                return;
            }
        };
        let saved = match self.app.session.active_mut() {
            Some(d) if d.revision == rev => {
                d.saved = d.wb.clone();
                true
            }
            _ => false,
        };
        if saved {
            self.adopt(&loc);
        }
        self.app.save_finished(ctx, rev, saved);
    }

    /// Turns the keyboard's messages into egui events for the next frame.
    fn drain_ime(&mut self) {
        while let Some(m) = ime::pop() {
            match m {
                Msg::Compose(t) => self.replace_composing(&t),
                Msg::Commit(t) => {
                    self.replace_composing(&t);
                    self.composing.clear();
                }
                Msg::Finish => self.composing.clear(),
                Msg::Delete { before, after } => {
                    let before = before.min(MAX_ERASE);
                    for _ in 0..before {
                        self.press(Key::Backspace);
                    }
                    for _ in 0..after.min(MAX_ERASE) {
                        self.press(Key::Delete);
                    }
                    let keep = self.composing.chars().count().saturating_sub(before);
                    self.composing = self.composing.chars().take(keep).collect();
                }
                Msg::Key(code) => {
                    let key = match code {
                        KEYCODE_ENTER => Key::Enter,
                        KEYCODE_DEL => Key::Backspace,
                        KEYCODE_FORWARD_DEL => Key::Delete,
                        KEYCODE_ESCAPE => Key::Escape,
                        _ => continue,
                    };
                    self.composing.clear();
                    self.press(key);
                }
            }
        }
    }

    /// Replaces the keyboard's composing text with `text`. Only the part after the common prefix is
    /// erased and typed again: a keystroke that extends the word ("H" to "He") sends just the new
    /// letter, not a Backspace for every letter before it, which lost letters on the emulator.
    fn replace_composing(&mut self, text: &str) {
        let common = self.composing.chars().zip(text.chars()).take_while(|(a, b)| a == b).count();
        for _ in common..self.composing.chars().count() {
            self.press(Key::Backspace);
        }
        let added: String = text.chars().skip(common).collect();
        if !added.is_empty() {
            self.pending.push(Event::Text(added));
        }
        self.composing = text.to_string();
    }

    fn press(&mut self, key: Key) {
        for pressed in [true, false] {
            self.pending.push(Event::Key { key, physical_key: Some(key), pressed, repeat: false, modifiers: Default::default() });
        }
    }

    /// Files picked since the last frame go to the UI's inbox; notices go to its message box.
    fn drain_files(&mut self) {
        for (loc, bytes) in files::drain_opened() {
            let name = files::parse_location(&loc).map_or_else(|| loc.clone(), |(_, n)| n.to_string());
            self.inbox.lock().unwrap_or_else(|e| e.into_inner()).push((name, bytes));
            self.opening.push(loc);
        }
        for msg in files::drain_notices() {
            self.app.message = Some(("GridCraft".into(), msg));
        }
    }
}

impl eframe::App for Shell {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_ime();
        self.drain_files();
        if !self.pending.is_empty() {
            ctx.request_repaint();
        }
        // Saves queued during this frame's input save this revision.
        files::set_revision(self.app.session.active().map_or(0, |d| d.revision));
        // Inbox documents open inside `logic`: the one that became active keeps its location.
        let before = self.app.session.active().map(|d| d.uid);
        self.app.logic(ctx);
        let after = self.app.session.active().map(|d| d.uid);
        let opened = std::mem::take(&mut self.opening);
        if before != after
            && let Some(loc) = opened.last().cloned()
        {
            self.adopt(&loc);
        }
        for (rev, outcome) in files::drain_saved() {
            self.finish_save(ctx, rev, outcome);
        }
        self.track_recovery(ctx);
        // Recovery copies: written once edits pause, or when a copy went stale.
        #[allow(clippy::disallowed_methods)] // Android only: the clock is there
        let now = Instant::now();
        if recovery::write_due(&self.seen, &self.written, self.changed_at, now) || self.copy_is_stale() {
            self.sync_recovery();
        }
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        // Touch arrives in window pixels; the surface sits inside the system-bar padding.
        let ppp = ctx.pixels_per_point();
        let shift = egui::vec2(ime::INSET_LEFT.load(Ordering::Relaxed) as f32, ime::INSET_TOP.load(Ordering::Relaxed) as f32) / ppp;
        for e in &mut raw.events {
            match e {
                Event::PointerMoved(p) | Event::PointerButton { pos: p, .. } | Event::Touch { pos: p, .. } => *p -= shift,
                _ => {}
            }
        }
        raw.events.append(&mut self.pending);
        raw.system_theme = Some(if ime::NIGHT.load(Ordering::Relaxed) { egui::Theme::Dark } else { egui::Theme::Light });
        self.app.raw_input_hook(raw);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        ime::UI_STARTED.store(true, Ordering::Relaxed);
        let _ = ime::WAKE.set(Box::new(move || ctx.request_repaint()));
        self.app.ui(ui);
        // egui sets `output.ime` while a text position is focused: that is when the keyboard shows.
        let wants = ui.ctx().output(|o| o.ime.is_some());
        ime::WANTS_KEYBOARD.store(wants, Ordering::Relaxed);
        ime::CAN_GO_BACK.store(self.app.can_go_back(ui.ctx()), Ordering::Relaxed);
        // egui-winit has no Android clipboard; copies go to ClipboardManager through Kotlin.
        let copies: Vec<String> = ui
            .ctx()
            .output(|o| o.commands.iter().filter_map(|c| if let egui::OutputCommand::CopyText(t) = c { Some(t.clone()) } else { None }).collect());
        if let Some(t) = copies.into_iter().last() {
            clip::copied(t);
        }
        self.track_recovery(ui.ctx());
        // Last, after the recovery state: the host ends the app once the copies can no longer hold a closed workbook.
        ime::QUIT.store(self.app.quit_requested, Ordering::Relaxed);
    }
}
