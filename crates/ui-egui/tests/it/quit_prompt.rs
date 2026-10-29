//! Android (`close_quits`): File › Close and Back with unsaved work ask the Save changes question
//! first. Don't Save closes the workbook and ends the app. Without unsaved work Back leaves the app
//! as before (the host's business: `can_go_back` is false).

use egui_kittest::kittest::Queryable;
use gridcraft_engine::Session;
use gridcraft_ui_egui::{Services, SheetApp};
use serde_json::json;

fn harness(dirty: bool, close_quits: bool) -> egui_kittest::Harness<'static, SheetApp> {
    let mut s = Session::new();
    s.new_workbook();
    if dirty {
        s.run("cell.set", json!({"cell": "A1", "input": "Hello"})).unwrap();
    }
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

fn dialog(h: &egui_kittest::Harness<'static, SheetApp>) -> Option<String> {
    h.state().dialog.as_ref().map(|d| d.name().to_string())
}

#[test]
fn close_with_unsaved_changes_then_dont_save_ends_the_app() {
    let mut h = harness(true, true);
    h.get_by_label("Menu").click();
    h.run_steps(4);
    h.get_by_label("Close").click();
    h.run_steps(4);
    assert_eq!(dialog(&h).as_deref(), Some("saveChanges"));
    assert!(!h.state().quit_requested, "the app waits for the answer");
    h.get_by_label("Don't Save").click();
    h.run_steps(4);
    assert!(h.state().quit_requested, "Don't Save ends the app");
    assert!(!h.state().session.documents().iter().any(|d| d.is_dirty()), "the discarded workbook is gone");
}

#[test]
fn back_with_unsaved_changes_asks_first_and_cancel_stays() {
    let mut h = harness(true, true);
    assert!(h.state().can_go_back(&h.ctx), "unsaved work makes Back a question");
    h.key_press(egui::Key::Escape);
    h.run_steps(4);
    assert_eq!(dialog(&h).as_deref(), Some("saveChanges"));
    h.get_by_label("Cancel").click();
    h.run_steps(4);
    assert!(!h.state().quit_requested, "Cancel keeps the app open");
    assert!(h.state().session.active().is_some_and(|d| d.is_dirty()), "the workbook is still open");
}

#[test]
fn back_without_unsaved_changes_does_not_intercept() {
    let mut h = harness(false, true);
    assert!(!h.state().can_go_back(&h.ctx), "nothing open and nothing unsaved: Back leaves as before");
    h.key_press(egui::Key::Escape);
    h.run_steps(4);
    assert!(dialog(&h).is_none() && !h.state().quit_requested);
}

#[test]
fn desktop_close_is_unchanged() {
    let mut h = harness(true, false);
    h.get_by_label("Menu").click();
    h.run_steps(4);
    h.get_by_label("Close").click();
    h.run_steps(4);
    assert_eq!(dialog(&h).as_deref(), Some("saveChanges"), "the prompt is the same");
    h.get_by_label("Don't Save").click();
    h.run_steps(4);
    assert!(!h.state().quit_requested, "without close_quits the window stays and a blank book is made");
}
