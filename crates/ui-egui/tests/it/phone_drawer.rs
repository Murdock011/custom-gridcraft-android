//! The phone's File button opens the File drawer: its rows are the File actions in WordCraft's
//! order, and Close with unsaved changes asks about them (the Save changes dialog) first.

use egui_kittest::kittest::Queryable;
use gridcraft_engine::Session;
use gridcraft_ui_egui::SheetApp;
use serde_json::json;

fn harness(dirty: bool) -> egui_kittest::Harness<'static, SheetApp> {
    let mut s = Session::new();
    s.new_workbook();
    if dirty {
        s.run("cell.set", json!({"cell": "A1", "input": "Hello"})).unwrap();
    }
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
fn the_file_button_opens_the_file_drawer_with_its_rows() {
    let mut h = harness(false);
    h.get_by_label("File").click();
    h.run_steps(4);
    // "Home" is also the bottom bar's tab, so the rows are looked up as a set.
    for row in ["Home", "New", "Open", "Info", "Save", "Save As", "Export", "Print", "Options", "About", "Close"] {
        assert!(h.query_all_by_label(row).next().is_some(), "the drawer lists {row}");
    }
}

#[test]
fn close_with_unsaved_changes_asks_the_save_changes_question() {
    let mut h = harness(true);
    h.get_by_label("File").click();
    h.run_steps(4);
    h.get_by_label("Close").click();
    h.run_steps(4);
    assert_eq!(h.state().dialog.as_ref().map(|d| d.name()), Some("saveChanges"), "the drawer's Close asks first");
    assert!(h.state().session.active().is_some(), "nothing closed yet");
}

#[test]
fn back_walks_from_a_file_page_to_the_file_menu_then_to_the_workbook() {
    let mut h = harness(false);
    h.get_by_label("File").click();
    h.run_steps(4);
    h.get_by_label("Info").click();
    h.run_steps(4);
    assert!(h.state().backstage.is_some(), "Info opened its page");
    h.key_press(egui::Key::Escape);
    h.run_steps(4);
    assert!(h.state().backstage.is_none(), "Back left the page");
    assert!(h.query_by_label("Save As").is_some(), "Back from a page shows the File menu again");
    h.key_press(egui::Key::Escape);
    h.run_steps(4);
    assert!(h.query_by_label("Save As").is_none(), "Back from the File menu returns to the workbook");
}

#[test]
fn the_file_menu_fills_the_screen_and_about_opens_its_dialog() {
    let mut h = harness(false);
    h.get_by_label("File").click();
    h.run_steps(4);
    let screen = h.ctx.content_rect();
    let close = h.get_by_label("Close").rect();
    assert!(close.left() < screen.width() * 0.25 && close.width() > screen.width() * 0.9, "rows run the full width: {close:?}");
    h.get_by_label("About").click();
    h.run_steps(4);
    assert_eq!(h.state().dialog.as_ref().map(|d| d.name()), Some("about"), "About opens the About dialog");
}
