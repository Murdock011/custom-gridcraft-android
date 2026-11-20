//! The List style of the phone's toolbar sheets: no tab bar, a quick bar whose More opens the sheet
//! at the tab used last, a tab menu in the sheet's header (every tab, then Search and All tools),
//! and the same commands as the tiles, one a row.

use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use gridcraft_engine::Session;
use gridcraft_ui_egui::{SheetApp, UiState};
use serde_json::json;

type Harness = egui_kittest::Harness<'static, SheetApp>;

fn harness(list: bool) -> Harness {
    let mut s = Session::new();
    s.new_workbook();
    let mut app = SheetApp::new(s, Default::default());
    app.ui.phone_list_style = list;
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(412.0, 915.0)).build_ui_state(
        |ui, app: &mut SheetApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app,
    );
    h.run_steps(6);
    h
}

fn tap(h: &mut Harness, label: &str) {
    h.get_by_label(label).click();
    h.run_steps(4);
}

#[test]
fn the_list_style_has_a_quick_bar_and_no_tab_bar() {
    let mut h = harness(true);
    assert!(h.query_by_role_and_label(Role::Button, "Home").is_none(), "no Home tab in the List style");
    assert!(h.query_by_role_and_label(Role::Button, "More").is_some(), "the quick bar's More");
    tap(&mut h, "Bold");
    let d = h.state().session.active().unwrap();
    let sh = d.wb.active().unwrap();
    assert!(d.wb.styles.get(sh.style_id(d.selection.active)).font.bold, "the quick bar's Bold ran");
    tap(&mut h, "More");
    assert!(h.query_by_label_contains("Tabs: Home").is_some(), "More opens the sheet at Home, with the tab menu");
    assert!(h.query_by_label("Italic").is_some(), "the Home commands are listed");
}

#[test]
fn the_tab_menu_switches_tabs_and_more_reopens_the_last_one() {
    let mut h = harness(true);
    tap(&mut h, "More");
    h.get_by_label_contains("Tabs: Home").click();
    h.run_steps(4);
    assert!(h.query_by_label("Search").is_some() && h.query_by_label("All tools").is_some(), "the menu ends with Search and All tools");
    tap(&mut h, "Data");
    assert!(h.query_by_label_contains("Tabs: Data").is_some(), "the menu switched to Data");
    tap(&mut h, "Close");
    tap(&mut h, "More");
    assert!(h.query_by_label_contains("Tabs: Data").is_some(), "More reopens the tab used last");
}

#[test]
fn the_tab_menu_opens_all_tools() {
    let mut h = harness(true);
    tap(&mut h, "More");
    h.get_by_label_contains("Tabs: Home").click();
    h.run_steps(4);
    tap(&mut h, "All tools");
    assert!(h.query_by_label("Search all commands").is_some(), "All tools opened");
}

#[test]
fn both_styles_list_the_same_home_commands() {
    let labels = |list: bool| {
        let mut h = harness(list);
        gridcraft_ui_egui::open_phone_sheet(&h.ctx, "Home");
        h.run_steps(4);
        let mut v: Vec<String> = ["Bold", "Italic", "Borders…", "Fill Color…", "Wrap Text", "Merge & Center", "Number Format…", "Percent Style"]
            .iter()
            .filter(|l| h.query_by_label(l).is_some())
            .map(|l| l.to_string())
            .collect();
        v.sort();
        v
    };
    assert_eq!(labels(false), labels(true), "Tiles and List show the same commands");
    assert_eq!(labels(true).len(), 8, "every command is there");
}

#[test]
fn the_setting_switches_styles_and_is_saved() {
    let mut h = harness(false);
    h.state_mut().open_dialog("backstage", json!({"page": "options"}));
    h.run_steps(4);
    tap(&mut h, "General");
    tap(&mut h, "List");
    assert!(h.state().ui.phone_list_style, "List chosen");
    let saved = serde_json::to_string(&h.state().ui).unwrap();
    let back: UiState = serde_json::from_str(&saved).unwrap();
    assert!(back.phone_list_style, "the choice survives a save and load");
    let old: UiState = serde_json::from_str("{}").unwrap();
    assert!(!old.phone_list_style, "Tiles is the default, also for saved settings from before");
}
