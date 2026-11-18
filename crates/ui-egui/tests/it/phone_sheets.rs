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
    assert_eq!(h.state().session.active().unwrap().wb.active_sheet.get(), 0);
    h.get_by_label("Sheets: Sheet1").click();
    h.run_steps(4);
    h.get_by_label("Budget").click();
    h.run_steps(4);
    assert_eq!(h.state().session.active().unwrap().wb.active_sheet.get(), 1, "the tapped sheet becomes active");
}

/// The tab bar's tab named `label` (a button; the sheet's title is a plain label of the same name).
fn tab<'a>(h: &'a egui_kittest::Harness<'static, SheetApp>, label: &'a str) -> egui_kittest::Node<'a> {
    h.get_by_role_and_label(egui::accesskit::Role::Button, label)
}

#[test]
fn a_tab_opens_its_sheet_above_the_tab_bar_and_a_second_tap_closes_it() {
    let mut h = harness();
    assert!(h.query_by_label("Bold").is_none(), "no sheet at the start");
    tab(&h, "Home").click();
    h.run_steps(4);
    let bold = h.get_by_label("Bold").rect();
    let home = tab(&h, "Home").rect();
    assert!(bold.bottom() <= home.top(), "the sheet sits above the tab bar, which stays visible");
    tab(&h, "Home").click();
    h.run_steps(4);
    assert!(h.query_by_label("Bold").is_none(), "tapping the open tab closes its sheet");
}

#[test]
fn another_tab_switches_the_sheet_and_view_is_in_more() {
    let mut h = harness();
    tab(&h, "Home").click();
    h.run_steps(4);
    tab(&h, "Insert").click();
    h.run_steps(4);
    assert!(h.query_by_label("Bold").is_none() && h.query_by_label("Chart").is_some(), "Insert's sheet replaced Home's");
    assert!(h.query_by_role_and_label(egui::accesskit::Role::Button, "View").is_none(), "View has no slot in the tab bar");
    tab(&h, "More").click();
    h.run_steps(4);
    h.get_by_label("View").click();
    h.run_steps(4);
    assert!(h.query_by_label("Freeze Panes").is_some(), "More › View opens the View sheet");
}

#[test]
fn every_tab_bar_slot_is_a_touch_target_on_a_narrow_phone() {
    let mut h = harness();
    h.set_size(egui::vec2(360.0, 800.0));
    h.run_steps(6);
    for label in ["Home", "Insert", "Formulas", "Data", "More"] {
        let r = tab(&h, label).rect();
        assert!(r.width() >= 48.0 && r.height() >= 48.0, "{label} is {} x {} pt", r.width(), r.height());
    }
    let sheets = h.get_by_label_contains("Sheets:").rect();
    assert!(sheets.width() >= 48.0 && sheets.height() >= 48.0, "the sheet button is {} x {} pt", sheets.width(), sheets.height());
}
