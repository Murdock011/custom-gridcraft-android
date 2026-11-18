//! All tools on the phone: every engine command by ribbon tab, Search, pickers for one value from a
//! list, Back one level at a time, and the contextual tabs under Selection in More.

use egui::Event;
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use gridcraft_engine::Session;
use gridcraft_ui_egui::SheetApp;
use serde_json::json;

type Harness = egui_kittest::Harness<'static, SheetApp>;

fn harness(s: Session) -> Harness {
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

fn blank() -> Session {
    let mut s = Session::new();
    s.new_workbook();
    s
}

fn tap(h: &mut Harness, label: &str) {
    h.get_by_label(label).click();
    h.run_steps(4);
}

fn open_more(h: &mut Harness) {
    h.get_by_role_and_label(Role::Button, "More").click();
    h.run_steps(4);
}

/// Types `text` into the focused field, one character a frame.
fn type_text(h: &mut Harness, text: &str) {
    for c in text.chars() {
        h.input_mut().events.push(Event::Text(c.to_string()));
        h.run_steps(1);
    }
    h.run_steps(2);
}

#[test]
fn all_tools_lists_the_tabs_and_a_tab_lists_its_commands() {
    let mut h = harness(blank());
    open_more(&mut h);
    tap(&mut h, "All tools");
    assert!(h.query_by_label("Search all commands").is_some(), "the list of tabs starts with Search");
    h.get_by_label_contains("Data  (").click();
    h.run_steps(4);
    // The engine's first Data command is the first row: the list is built from the engine.
    let first = gridcraft_engine::cmd::command_specs().iter().find(|s| s.journal && s.menu.first() == Some(&"Data")).unwrap();
    assert!(h.query_by_label(first.label).is_some(), "the Data tab lists {}", first.label);
}

#[test]
fn search_finds_a_command_by_name_and_runs_it() {
    let mut h = harness(blank());
    open_more(&mut h);
    tap(&mut h, "Search");
    type_text(&mut h, "bold");
    tap(&mut h, "Bold");
    let d = h.state().session.active().unwrap();
    let sh = d.wb.active().unwrap();
    assert!(d.wb.styles.get(sh.style_id(d.selection.active)).font.bold, "Bold ran from Search");
    assert!(h.query_by_label("Search commands").is_none(), "the tools closed after running");
}

#[test]
fn search_runs_freeze_panes_at_the_selection() {
    let mut s = blank();
    s.run("selection.set", json!({"range": "C3"})).unwrap();
    let mut h = harness(s);
    open_more(&mut h);
    tap(&mut h, "Search");
    type_text(&mut h, "freeze pan");
    tap(&mut h, "Freeze Panes");
    assert!(h.state().session.active().unwrap().wb.active().unwrap().freeze.is_some(), "Freeze Panes ran from Search");
}

#[test]
fn a_tile_that_needs_one_value_opens_a_picker_whose_back_returns_to_the_sheet() {
    let mut h = harness(blank());
    gridcraft_ui_egui::open_phone_sheet(&h.ctx, "Page Layout");
    h.run_steps(4);
    tap(&mut h, "Orientation");
    assert!(h.query_by_label("Landscape").is_some(), "Orientation offers its values");
    tap(&mut h, "Back");
    assert!(h.query_by_label("Set Print Area").is_some(), "Back returns to the Page Layout tiles");
    tap(&mut h, "Orientation");
    tap(&mut h, "Landscape");
    assert!(h.state().message.is_none(), "no error: {:?}", h.state().message);
    assert!(h.query_by_label("Landscape").is_none(), "the picker closed after the choice");
}

#[test]
fn back_closes_one_level_at_a_time() {
    let mut h = harness(blank());
    open_more(&mut h);
    tap(&mut h, "All tools");
    h.get_by_label_contains("Data  (").click();
    h.run_steps(4);
    assert!(h.query_by_label("Sort A to Z").is_some(), "the Data tab lists its commands");
    h.key_press(egui::Key::Escape);
    h.run_steps(4);
    assert!(h.query_by_label("Search all commands").is_some(), "Back from a tab shows the list of tabs");
    h.key_press(egui::Key::Escape);
    h.run_steps(4);
    assert!(h.query_by_label("All tools").is_some(), "Back from the list of tabs shows More");
    h.key_press(egui::Key::Escape);
    h.run_steps(4);
    assert!(h.query_by_label("All tools").is_none(), "Back from More closes the sheet");
}

#[test]
fn a_table_adds_its_tab_under_selection_in_more() {
    let mut s = blank();
    s.run("cell.set", json!({"cell": "A1", "input": "Name"})).unwrap();
    s.run("cell.set", json!({"cell": "A2", "input": "Ada"})).unwrap();
    s.run("insert.table", json!({"range": "A1:A2", "header": true})).unwrap();
    s.run("selection.set", json!({"range": "A2"})).unwrap();
    let mut h = harness(s);
    open_more(&mut h);
    tap(&mut h, "Table Design");
    assert!(h.query_by_label("Back").is_some() && h.query_by_label("Search all commands").is_none(), "Table Design opens its tab in the tools");
}
