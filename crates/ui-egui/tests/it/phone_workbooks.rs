//! The phone's File drawer lists the open workbooks (two or more): a tap switches to one, and the
//! row's ✕ closes it. Android (`close_quits`) closes the active one and keeps the app running while
//! another stays open.

use egui_kittest::kittest::Queryable;
use gridcraft_engine::Session;
use gridcraft_ui_egui::{Services, SheetApp};
use serde_json::json;

/// Two workbooks: "Book1" with an unsaved edit in A1 (dirty), and "Book2" (clean, the active one).
fn harness(close_quits: bool) -> egui_kittest::Harness<'static, SheetApp> {
    let mut s = Session::new();
    s.new_workbook();
    s.run("cell.set", json!({"cell": "A1", "input": "Hello"})).unwrap();
    s.new_workbook();
    let services = Services { close_quits, ..Default::default() };
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(412.0, 915.0)).build_ui_state(
        |ui, app: &mut SheetApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        SheetApp::new(s, services),
    );
    h.run_steps(6);
    h
}

fn open_drawer(h: &mut egui_kittest::Harness<'static, SheetApp>) {
    h.get_by_label("Menu").click();
    h.run_steps(4);
}

#[test]
fn the_drawer_lists_both_workbooks_and_a_tap_switches() {
    let mut h = harness(false);
    open_drawer(&mut h);
    assert!(h.query_all_by_label("Book1 •").next().is_some(), "the dirty workbook is listed with its dot");
    assert!(h.query_all_by_label("Book2").next().is_some(), "the active workbook is listed");
    assert_eq!(h.state().session.active_index(), 1);
    h.query_all_by_label("Book1 •").next().unwrap().click();
    h.run_steps(4);
    assert_eq!(h.state().session.active_index(), 0, "the tap makes Book1 the active workbook");
    assert!(h.query_all_by_label("Book2").next().is_none(), "the drawer closed after the tap");
}

#[test]
fn a_single_workbook_shows_no_list() {
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
    open_drawer(&mut h);
    assert!(h.query_all_by_label("Close Book1").next().is_none(), "no workbook rows with one open");
}

#[test]
fn opening_another_workbook_commits_the_cell_being_edited() {
    let mut s = Session::new();
    s.new_workbook();
    let services = Services { open_async: Some(Box::new(|| {})), ..Default::default() };
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(412.0, 915.0)).build_ui_state(
        |ui, app: &mut SheetApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        SheetApp::new(s, services),
    );
    h.run_steps(6);
    h.state_mut().begin_edit(Some("Typed".into()), false);
    h.run_steps(2);
    h.state_mut().open_dialog("open", json!({}));
    h.run_steps(2);
    let wb = &h.state().session.active().unwrap().wb;
    assert_eq!(wb.sheet(0).unwrap().value(gridcraft_engine::core::CellRef::new(0, 0)).display(), "Typed", "the typed text is kept");
}
