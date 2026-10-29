//! The egui side: one `SheetApp` in an eframe window, fed by the soft keyboard and the document
//! picker. Keeps the Android specifics (touch insets, night mode, clipboard) out of the UI crate.

use std::sync::atomic::Ordering;

use egui::{Event, Key};
use gridcraft_engine::Session;
use gridcraft_ui_egui::{Inbox, Services, SheetApp};

use crate::clip;
use crate::files;
use crate::ime::{self, Msg};

/// Android KeyEvent codes the keyboard sends as key presses.
const KEYCODE_ESCAPE: i32 = 111;
const KEYCODE_DEL: i32 = 67;
const KEYCODE_FORWARD_DEL: i32 = 112;
const KEYCODE_ENTER: i32 = 66;

/// Most keys one `Delete` message may erase, so a hostile count can't flood the queue.
const MAX_ERASE: usize = 256;

pub struct Shell {
    app: SheetApp,
    /// Events from the keyboard, handed to egui with the next frame's input.
    pending: Vec<Event>,
    /// The keyboard's composing text, which is replaced as the user types (see `replace_composing`).
    composing: String,
    inbox: Inbox,
    /// Locations of the documents queued for the inbox in this frame (see `logic`).
    opening: Vec<String>,
}

impl Shell {
    pub fn new(ctx: &egui::Context) -> Shell {
        let mut session = Session::new();
        session.new_workbook();
        let inbox = Inbox::default();
        let services = Services {
            open_async: Some(Box::new(files::request_open)),
            inbox: Some(inbox.clone()),
            download: Some(Box::new(|name: &str, bytes: &[u8]| files::save_as(name, bytes))),
            save_in_place: Some(Box::new(|loc: &str, bytes: &[u8]| files::save_in_place(loc, bytes))),
            paste_text: Some(Box::new(clip::text)),
            ..Default::default()
        };
        let mut app = SheetApp::new(session, services);
        SheetApp::setup_context_for_language(ctx, app.ui.dark, app.ui.language);
        app.after_engine();
        Shell { app, pending: Vec::new(), composing: String::new(), inbox, opening: Vec::new() }
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
    fn finish_save(&mut self, rev: u64, loc: &str) {
        let saved = match self.app.session.active_mut() {
            Some(d) if d.revision == rev => {
                d.saved = d.wb.clone();
                true
            }
            _ => false,
        };
        if saved {
            self.adopt(loc);
        }
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

    /// Replaces the keyboard's composing text with `text`: erase what was composed, type the new.
    fn replace_composing(&mut self, text: &str) {
        for _ in 0..self.composing.chars().count() {
            self.press(Key::Backspace);
        }
        if !text.is_empty() {
            self.pending.push(Event::Text(text.to_string()));
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
        for (rev, loc) in files::drain_saved() {
            self.finish_save(rev, &loc);
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
    }
}
