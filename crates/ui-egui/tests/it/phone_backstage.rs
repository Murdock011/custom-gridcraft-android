//! The phone's Backstage pages: Open shows a Browse row, and Options goes list → category → back.

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

/// Opens a File drawer row from the phone's menu button.
fn file_row(h: &mut egui_kittest::Harness<'static, SheetApp>, row: &str) {
    h.get_by_label("Menu").click();
    h.run_steps(4);
    h.get_by_label(row).click();
    h.run_steps(6);
}

#[test]
fn open_page_shows_a_browse_row() {
    let mut h = harness();
    file_row(&mut h, "Open");
    assert!(h.state().backstage.is_some(), "Open is a Backstage page");
    assert!(h.query_all_by_label("Browse…").next().is_some(), "the Open page has a Browse row");
    assert!(h.query_all_by_label("Back").next().is_some(), "the page has a Back button in its top bar");
}

#[test]
fn options_list_category_and_back_return_step_by_step() {
    let mut h = harness();
    file_row(&mut h, "Options");
    for cat in ["General", "Editing", "Save", "Formulas"] {
        assert!(h.query_all_by_label(cat).next().is_some(), "the list shows {cat}");
    }
    assert_eq!(h.state().backstage.as_ref().and_then(|b| b.options_category), None, "the list shows first");

    h.get_by_label("Editing").click();
    h.run_steps(6);
    assert_eq!(h.state().backstage.as_ref().and_then(|b| b.options_category), Some(1), "Editing is open");
    assert!(h.query_all_by_label("Enable AutoComplete for cell values").next().is_some(), "the Editing page shows its switch");

    // Back from the category returns to the list, and stays in Options.
    h.get_by_label("Back").click();
    h.run_steps(6);
    assert_eq!(h.state().backstage.as_ref().and_then(|b| b.options_category), None, "Back returns to the list");
    assert!(h.state().backstage.is_some(), "Options stays open");

    // Back from the list returns to the sheet.
    h.get_by_label("Back").click();
    h.run_steps(6);
    assert!(h.state().backstage.is_none(), "Back from the list returns to the sheet");
}
