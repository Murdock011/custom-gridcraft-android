use super::{SaveAnswer, SheetApp, json};

impl SheetApp {
    /// Holds back a window close (the close button, Alt+F4, the OS quitting the app) while a
    /// workbook has unsaved changes, and asks about them first.
    pub(super) fn guard_window_close(&mut self, ctx: &egui::Context) {
        if self.native_windows || self.allow_close || !ctx.input(|i| i.viewport().close_requested()) {
            return;
        }
        // A cell still being edited is part of the workbook, as for File › Close.
        if !self.commit_edit(0, 0, false, false) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            return;
        }
        // The macro step editor asks about its own unsaved steps and debug run first; once it has
        // closed, the window close goes on.
        if self.dialog.as_ref().is_some_and(|d| d.name == "macroEditor") {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            crate::macro_editor::request_close(self, true);
            return;
        }
        if !self.session.documents().iter().any(|d| d.is_dirty()) {
            return;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        match self.dialog.as_mut() {
            // Already asking about a workbook (File › Close): answering it now goes on to quit.
            Some(d) if d.name() == "saveChanges" => {
                d.values.insert("quit".into(), json!(true));
            }
            _ => self.continue_quit(ctx),
        }
    }

    /// Asks about the next unsaved workbook before quitting, in its window, or closes the app
    /// once none is left.
    pub(crate) fn continue_quit(&mut self, ctx: &egui::Context) {
        let dirty = self.session.documents().iter().find(|d| d.is_dirty()).map(|d| d.uid);
        match dirty.and_then(|uid| self.session.doc_window(uid)) {
            Some(window) => {
                // Its window comes forward (the native window too: see `doc_windows`).
                self.enter_window(window);
                let title = self.session.active().map(|d| d.display_title()).unwrap_or_default();
                self.open_dialog("saveChanges", json!({"title": title, "quit": true}));
                self.bring_forward = true;
            }
            None => {
                self.allow_close = true;
                self.quit_requested = true;
                ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close);
            }
        }
    }

    /// Answers the Save Changes question about the active workbook (its buttons). While quitting,
    /// the next unsaved workbook is asked about once this one is answered; Cancel stops the quit.
    /// Returns whether the question is settled: Save keeps it open when the cell being edited
    /// can't be entered.
    pub fn answer_save_changes(&mut self, answer: SaveAnswer, quitting: bool, ctx: &egui::Context) -> bool {
        match answer {
            SaveAnswer::Cancel => {}
            SaveAnswer::DontSave => {
                self.run_or_alert("file.close", json!({"force": true, "document": true}));
                self.after_close(ctx, quitting);
            }
            SaveAnswer::Save => {
                // File operations include the cell still being edited, as File › Save does.
                if !self.commit_edit(0, 0, false, false) {
                    return false;
                }
                self.run_or_alert("file.save", json!({}));
                if self.session.active().is_some_and(|x| !x.is_dirty()) {
                    self.run_or_alert("file.close", json!({"force": true, "document": true}));
                    self.after_close(ctx, quitting);
                } else if self.services.close_quits {
                    // Android: the save is the system's dialog and answers later (`save_finished`).
                    self.close_after_save = self.session.active().map(|x| (x.revision, quitting));
                }
            }
        }
        true
    }

    /// Quit (the macOS app menu, ⌘Q): commits every window's cell draft as Enter would, lets the
    /// macro step editor ask about its unsaved steps, then asks about each unsaved workbook in its
    /// window; Cancel on any of them stops the quit. A draft its
    /// workbook refuses stops the quit too, with its window in front and the reason shown.
    pub fn request_quit(&mut self, ctx: &egui::Context) {
        while let Some(window) = self.parked_window_with_draft() {
            self.enter_window(window);
            if self.settle_drafts().is_err() {
                return;
            }
        }
        if self.settle_drafts().is_err() {
            return;
        }
        // The macro step editor asks about its own unsaved steps and debug run first, as for the
        // window's close button, in the window it is shown in; once it has closed, the quit goes
        // on, and Cancel stops it.
        if let Some(window) = self.window_with_dialog("macroEditor") {
            if self.ui_window() != Some(window) {
                self.enter_window(window);
            }
            crate::macro_editor::request_close(self, true);
            return;
        }
        if let Some(d) = self.dialog.as_mut().filter(|d| d.name() == "saveChanges") {
            // Already asking about a workbook (File › Close): answering it goes on to quit.
            d.values.insert("quit".into(), json!(true));
            return;
        }
        self.continue_quit(ctx);
    }

    /// Whether Android Back has something open to close first: a phone sheet or its palette, a dialog
    /// or message (both bottom sheets on the phone), the File drawer, the Backstage, or a grid popup (the
    /// filter, cell, list or header menu, which Escape closes). The host calls
    /// this each frame and closes the innermost part on Back. With unsaved work (Android) Back also
    /// counts: with nothing else open it asks about the work (see `ui`).
    pub fn can_go_back(&self, ctx: &egui::Context) -> bool {
        self.something_open(ctx) || self.leaves_with_prompt()
    }

    pub(super) fn something_open(&self, ctx: &egui::Context) -> bool {
        self.backstage.is_some()
            || self.editor.is_some()
            || self.dialog.is_some()
            || self.message.is_some()
            || self.grid.filter_menu.is_some()
            || self.grid.context_menu.is_some()
            || self.grid.list_picker.is_some()
            || self.grid.header_menu.is_some()
            || self.grid.fill_menu.is_some()
            || crate::phone::can_go_back(ctx)
    }

    /// Android: leaving the app would lose unsaved work, so Back asks first.
    pub(super) fn leaves_with_prompt(&self) -> bool {
        self.services.close_quits && self.session.documents().iter().any(|d| d.is_dirty())
    }

    /// File › Close: commits the cell being edited, then closes the active workbook, asking first when it
    /// has unsaved changes. On Android (`close_quits`) the last workbook's close ends the app (see
    /// `continue_quit`); with others open the app stays and another workbook shows (`after_close`).
    /// False when the cell being edited could not be committed.
    pub(crate) fn close_file(&mut self, ctx: &egui::Context) -> bool {
        if !self.commit_edit(0, 0, false, false) {
            return false;
        }
        if !self.services.close_quits {
            self.run_or_alert("file.close", json!({}));
        } else if self.session.documents().len() <= 1 {
            self.continue_quit(ctx);
        } else if let Some(d) = self.session.active() {
            if d.is_dirty() {
                let title = d.display_title();
                self.open_dialog("saveChanges", json!({"title": title, "quit": false}));
            } else {
                self.run_or_alert("file.close", json!({}));
            }
        }
        true
    }

    /// Android: the active workbook has closed. While quitting (`quitting`) the next unsaved workbook is
    /// asked about; with none left the app ends. Otherwise the app stays on another open workbook.
    pub(crate) fn after_close(&mut self, ctx: &egui::Context, quitting: bool) {
        if quitting || (self.services.close_quits && self.session.documents().is_empty()) {
            self.continue_quit(ctx);
        }
    }

    /// Android: a save of revision `rev` ended. `saved` says it wrote the workbook, which is clean now.
    /// If the Save changes prompt or File › Close asked for that save, the workbook closes and the app
    /// goes on to quit (or asks about the next unsaved workbook). A cancelled save leaves it open.
    pub fn save_finished(&mut self, ctx: &egui::Context, rev: u64, saved: bool) {
        let Some((pending, quitting)) = self.close_after_save else {
            return;
        };
        if pending != rev {
            return;
        }
        self.close_after_save = None;
        if saved {
            self.run_or_alert("file.close", json!({"force": true}));
            self.after_close(ctx, quitting);
        }
    }
}
