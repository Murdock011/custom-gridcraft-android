//! The phone's sheet button opens the Sheets sheet, and tapping a sheet's row switches to that sheet.
//! The sheet tab row is not drawn on the phone (its sheets live in the bar's sheet button).

use egui_kittest::kittest::Queryable;
use gridcraft_engine::Session;
use gridcraft_ui_egui::SheetApp;
use serde_json::json;

fn harness() -> egui_kittest::Harness<'static, SheetApp> {
    let mut s = Session::new();
    s.new_workbook();
    s.run("home.insertSheet", json!({"name": "Budget"})).unwrap();
    s.run("home.insertSheet", json!({"name": "Notes"})).unwrap();
    s.run("sheet.activate", json!({"sheet": 0})).unwrap();
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
fn tapping_a_sheet_in_the_sheets_sheet_makes_it_active() {
    let mut h = harness();
    assert_eq!(h.state().session.active().unwrap().wb.active_sheet, 0);
    h.get_by_label("Sheets: Sheet1").click();
    h.run_steps(4);
    h.get_by_label("Budget").click();
    h.run_steps(4);
    assert_eq!(h.state().session.active().unwrap().wb.active_sheet, 1, "the tapped sheet becomes active");
}
