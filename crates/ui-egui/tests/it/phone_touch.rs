//! The phone's touch selection: the action bar that a long press opens runs its commands, and its
//! dialog entries open the dialogs.

use egui_kittest::kittest::Queryable;
use gridcraft_engine::Session;
use gridcraft_engine::core::CellRef;
use gridcraft_ui_egui::SheetApp;
use serde_json::json;

fn harness(setup: impl FnOnce(&mut Session)) -> egui_kittest::Harness<'static, SheetApp> {
    harness_sized(setup, egui::vec2(412.0, 915.0))
}

/// The harness at `size`. A phone window (under 600 pt on its shorter side) gets the phone layout.
fn harness_sized(setup: impl FnOnce(&mut Session), size: egui::Vec2) -> egui_kittest::Harness<'static, SheetApp> {
    let mut s = Session::new();
    s.new_workbook();
    setup(&mut s);
    let mut h = egui_kittest::Harness::builder().with_size(size).build_ui_state(
        |ui, app: &mut SheetApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        SheetApp::new(s, Default::default()),
    );
    h.run_steps(6);
    h
}

/// A1:B2 selected, with A1 and B2 filled.
fn filled(s: &mut Session) {
    s.execute("cell.set", json!({"cell": "A1", "input": "one"})).expect("A1");
    s.execute("cell.set", json!({"cell": "B2", "input": "four"})).expect("B2");
    s.execute("selection.set", json!({"range": "A1:B2"})).expect("select");
}

/// Whether the cell at (row, col) of the first sheet holds a value.
fn filled_at(h: &egui_kittest::Harness<'static, SheetApp>, row: u32, col: u32) -> bool {
    h.state().session.active().and_then(|d| d.wb.sheet(0).and_then(|sh| sh.cell(CellRef::new(row, col)))).is_some_and(|c| !c.value.is_empty())
}

#[test]
fn clear_in_the_action_bar_clears_the_selection() {
    let mut h = harness(filled);
    gridcraft_ui_egui::open_phone_sheet(&h.ctx, "actionbar");
    h.run_steps(4);
    h.get_all_by_label("Clear").last().expect("action bar clear").click();
    h.run_steps(4);
    assert!(!filled_at(&h, 0, 0) && !filled_at(&h, 1, 1), "the selection's contents are cleared");
    assert!(!gridcraft_ui_egui::phone_action_bar_open(&h.ctx), "tapping an action closes the bar");
}

#[test]
fn delete_in_the_action_bar_opens_the_delete_cells_dialog() {
    // Delete is past the bar's visible slots: it sits in the More sheet.
    let mut h = harness(filled);
    gridcraft_ui_egui::open_phone_sheet(&h.ctx, "actionbar");
    h.run_steps(4);
    h.get_all_by_label("More").last().expect("action bar more").click();
    h.run_steps(4);
    h.get_all_by_label("Delete…").last().expect("more sheet delete").click();
    h.run_steps(6);
    assert!(filled_at(&h, 0, 0), "nothing is deleted until the dialog is confirmed");
    assert_eq!(h.state().dialog.as_ref().map(|d| d.name()), Some("deleteCells"));
}

#[test]
fn copy_in_the_action_bar_puts_the_selection_on_the_clipboard() {
    let mut h = harness(filled);
    gridcraft_ui_egui::open_phone_sheet(&h.ctx, "actionbar");
    h.run_steps(4);
    h.get_all_by_label("Copy").last().expect("action bar copy").click();
    h.run_steps(4);
    assert!(h.state().session.clipboard.is_some(), "Copy fills the clipboard");
    assert!(filled_at(&h, 0, 0), "Copy keeps the cells");
}
