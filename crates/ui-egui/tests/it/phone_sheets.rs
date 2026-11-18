//! The phone's sheet button opens the Sheets sheet, and tapping a sheet's row switches to that sheet.
//! The sheet tab row is not drawn on the phone (its sheets live in the bar's sheet button).

use egui_kittest::kittest::{NodeT, Queryable};
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
    assert!(h.query_by_label("Bold").is_none() && h.query_by_label("Insert Chart").is_some(), "Insert's sheet replaced Home's");
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

/// The active cell's style in the harness's workbook.
fn active_style(h: &egui_kittest::Harness<'static, SheetApp>) -> gridcraft_engine::model::Style {
    let d = h.state().session.active().unwrap();
    let sh = d.wb.active().unwrap();
    d.wb.styles.get(sh.style_id(d.selection.active)).clone()
}

#[test]
fn a_format_tile_applies_shows_on_and_keeps_the_sheet_open() {
    let mut h = harness();
    tab(&h, "Home").click();
    h.run_steps(4);
    assert_ne!(h.get_by_label("Bold").accesskit_node().toggled(), Some(egui::accesskit::Toggled::True), "Bold is off in a plain cell");
    h.get_by_label("Bold").click();
    h.run_steps(4);
    assert!(active_style(&h).font.bold, "the Bold tile made the cell bold");
    assert_eq!(h.get_by_label("Bold").accesskit_node().toggled(), Some(egui::accesskit::Toggled::True), "Bold shows on, and the sheet stayed open");
}

#[test]
fn an_insert_tile_runs_and_closes_the_sheet() {
    let mut h = harness();
    tab(&h, "Insert").click();
    h.run_steps(4);
    h.get_by_label("Insert Chart").click();
    h.run_steps(4);
    assert!(h.query_by_label("Insert Chart").is_none(), "the sheet closed after inserting");
}

#[test]
fn a_sub_page_has_a_back_arrow_and_returns_to_the_sheet_after_a_choice() {
    let mut h = harness();
    tab(&h, "Home").click();
    h.run_steps(4);
    h.get_by_label("Borders…").click();
    h.run_steps(4);
    assert!(h.query_by_label("Bold").is_none(), "the Borders page replaces the tiles");
    h.get_by_label("Back").click();
    h.run_steps(4);
    assert!(h.query_by_label("Bold").is_some(), "Back returns to the Home tiles");
    h.get_by_label("Borders…").click();
    h.run_steps(4);
    h.get_by_label("All Borders").click();
    h.run_steps(4);
    let st = active_style(&h);
    assert!(st.borders != Default::default(), "All Borders set the cell's borders");
    assert!(h.query_by_label("Bold").is_some(), "after a choice the Home tiles are back, and the sheet is open");
}

#[test]
fn the_font_size_page_sets_the_size() {
    let mut h = harness();
    tab(&h, "Home").click();
    h.run_steps(4);
    h.get_by_label_contains("Font Size:").click();
    h.run_steps(4);
    h.get_by_label("14").click();
    h.run_steps(4);
    assert!((active_style(&h).font.size - 14.0).abs() < 0.01, "the size page set 14 pt");
}

#[test]
fn every_tile_is_a_touch_target_on_a_narrow_phone() {
    let mut h = harness();
    h.set_size(egui::vec2(360.0, 800.0));
    h.run_steps(6);
    tab(&h, "Home").click();
    h.run_steps(4);
    for label in ["Bold", "Italic", "Underline", "Strikethrough", "Borders…"] {
        let r = h.get_by_label(label).rect();
        assert!(r.width() >= 48.0 && r.height() >= 48.0, "{label} is {} x {} pt", r.width(), r.height());
    }
}

#[test]
fn dark_theme_darkens_the_grid_on_a_phone_but_not_on_the_desktop() {
    let mut h = harness();
    h.state_mut().run_or_alert("view.theme", serde_json::json!({"mode": "dark"}));
    h.run_steps(4);
    let t = gridcraft_ui_egui::theme::Tokens::get(&h.ctx);
    assert!(t.dark_grid && t.cell_bg != egui::Color32::WHITE, "a phone in dark theme has a dark grid");
    assert_eq!(t.grid_bg, egui::Color32::WHITE, "paper (print preview, page view) stays white");
    h.set_size(egui::vec2(1200.0, 800.0));
    h.run_steps(4);
    let t = gridcraft_ui_egui::theme::Tokens::get(&h.ctx);
    assert!(!t.dark_grid && t.cell_bg == egui::Color32::WHITE, "the desktop keeps its white grid");
}

#[test]
fn the_phone_options_page_offers_light_dark_and_system_with_light_first_chosen() {
    let mut h = harness();
    h.state_mut().open_dialog("backstage", json!({"page": "options"}));
    h.run_steps(4);
    h.get_by_label("General").click();
    h.run_steps(4);
    assert_eq!(h.state().ui.theme_mode(), "light", "Light is the default");
    assert_eq!(h.get_by_label("Light").accesskit_node().toggled(), Some(egui::accesskit::Toggled::True));
    h.get_by_label("System").click();
    h.run_steps(4);
    assert_eq!(h.state().ui.theme_mode(), "system", "System is chosen");
    h.get_by_label("Dark").click();
    h.run_steps(4);
    assert_eq!(h.state().ui.theme_mode(), "dark", "Dark is chosen");
}
