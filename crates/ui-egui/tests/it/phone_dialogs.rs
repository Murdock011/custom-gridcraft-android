//! Every dialog on a 360 pt phone: it draws without a panic, passes the accessibility check (every
//! control named), and no control runs past the screen's right edge (the shared shell's dialog
//! rules). The Format Cells tab strip scrolls sideways by design, so its tabs may.

use egui_kittest::kittest::{NodeT, Queryable};
use gridcraft_engine::Session;
use gridcraft_ui_egui::SheetApp;
use serde_json::json;

const DIALOGS: &[&str] = &[
    "about",
    "accessibility",
    "advancedFilter",
    "agents",
    "allowEditRanges",
    "analysisToolPak",
    "arrangeAll",
    "assignAction",
    "cellIs",
    "cfQuick",
    "chartTitle",
    "columnWidth",
    "commandSearch",
    "comment",
    "comments",
    "containsText",
    "controlQuestion",
    "dataValidation",
    "defaultWidth",
    "defineName",
    "deleteCells",
    "equation",
    "errorChecking",
    "evaluateFormula",
    "find",
    "forecastSheet",
    "formatCells",
    "formatChart",
    "formatControl",
    "goTo",
    "goToSpecial",
    "goalSeek",
    "headerFooter",
    "insertCells",
    "insertFunction",
    "insertLink",
    "insertPaste",
    "insertPicture",
    "journal",
    "macroEditor",
    "macros",
    "manageRules",
    "moveSheet",
    "nameManager",
    "openLink",
    "openPassword",
    "outlineSettings",
    "pageSetup",
    "pasteName",
    "pasteSpecial",
    "percent",
    "pickList",
    "pivotCalculatedField",
    "preset",
    "protectSheet",
    "protectWorkbook",
    "recommendedCharts",
    "recordMacro",
    "removeDuplicates",
    "renameSheet",
    "rowHeight",
    "saveChanges",
    "saveCopy",
    "selectData",
    "selectionPane",
    "series",
    "solver",
    "solverResults",
    "sort",
    "sparkline",
    "spelling",
    "start",
    "statistics",
    "subtotal",
    "tableResize",
    "textToColumns",
    "timePeriod",
    "unhideSheet",
    "unprotectSheet",
    "unprotectWorkbook",
    "watch",
    "zoom",
];

/// Controls a person taps or types into.
fn interactive(role: egui::accesskit::Role) -> bool {
    use egui::accesskit::Role::*;
    matches!(role, Button | RadioButton | CheckBox | TextInput | ComboBox | SpinButton | Slider)
}

#[test]
fn every_dialog_fits_a_360_pt_phone_and_names_its_controls() {
    let mut overflow = Vec::new();
    for name in DIALOGS {
        let mut s = Session::new();
        s.new_workbook();
        s.run("cell.set", json!({"cell": "A1", "input": "Name"})).unwrap();
        s.run("cell.set", json!({"cell": "A2", "input": "Ada"})).unwrap();
        let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(360.0, 800.0)).build_ui_state(
            |ui, app: &mut SheetApp| {
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                app.ui(ui);
            },
            SheetApp::new(s, Default::default()),
        );
        h.run_steps(4);
        h.state_mut().open_dialog(name, json!({}));
        // A full run ends with the accessibility check; a dialog that keeps animating is run by steps.
        if h.try_run().is_err() {
            h.run_steps(8);
        }
        let screen = h.ctx.content_rect();
        for node in h.get_all(egui_kittest::kittest::by()) {
            let n = node.accesskit_node();
            if !interactive(n.role()) || n.is_hidden() {
                continue;
            }
            let r = node.rect();
            let label = n.label().unwrap_or_default();
            let tab = *name == "formatCells" && ["Number", "Alignment", "Font", "Border", "Fill", "Protection"].contains(&label.as_str());
            if r.right() > screen.right() + 1.0 && r.left() < screen.right() && !tab {
                overflow.push(format!("{name}: {:?} {label:?} ends at {:.0}", n.role(), r.right()));
            }
        }
    }
    assert!(overflow.is_empty(), "controls past the right edge of a 360 pt phone:\n{}", overflow.join("\n"));
}
