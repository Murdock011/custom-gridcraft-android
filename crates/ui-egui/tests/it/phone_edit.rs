//! The phone's edit bar: while a cell is being edited it replaces the floating bar, ✗ cancels the
//! edit and ✓ commits it and moves down, as Enter does.

use egui_kittest::kittest::Queryable;
use gridcraft_engine::Session;
use gridcraft_ui_egui::SheetApp;

fn harness() -> egui_kittest::Harness<'static, SheetApp> {
    let mut s = Session::new();
    s.new_workbook();
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(412.0, 915.0)).build_ui_state(
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

#[test]
fn cancel_button_ends_the_edit_and_keeps_the_cell() {
    let mut h = harness();
    h.state_mut().begin_edit(Some("42".into()), false);
    h.run_steps(4);
    assert!(h.state().editor.is_some());
    h.get_all_by_label("Cancel").last().expect("edit bar cancel").click();
    h.run_steps(4);
    assert!(h.state().editor.is_none(), "the edit is cancelled");
    let wb = &h.state().session.active().unwrap().wb;
    assert!(wb.sheet(0).unwrap().cell(gridcraft_engine::core::CellRef::new(0, 0)).is_none(), "nothing was committed");
}

#[test]
fn confirm_button_commits_and_moves_down() {
    let mut h = harness();
    h.state_mut().begin_edit(Some("42".into()), false);
    h.run_steps(4);
    h.get_all_by_label("Enter").last().expect("edit bar confirm").click();
    h.run_steps(4);
    assert!(h.state().editor.is_none(), "the edit is committed");
    let active = h.state().session.active().unwrap().selection.active;
    assert_eq!((active.row, active.col), (1, 0), "the selection moved down to A2");
}
