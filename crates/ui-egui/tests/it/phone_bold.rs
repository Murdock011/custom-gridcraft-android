//! The phone sheet's Bold runs `home.bold` with no params, as the ribbon does. A1 must end up bold
//! and stay regular in the cell below it.

use gridcraft_engine::Session;
use gridcraft_engine::core::CellRef;
use gridcraft_ui_egui::SheetApp;
use serde_json::json;

fn app() -> SheetApp {
    let mut session = Session::new();
    session.new_workbook();
    SheetApp::new(session, Default::default())
}

fn bold(app: &SheetApp, cell: &str) -> bool {
    let d = app.session.active().unwrap();
    let sh = d.wb.active().unwrap();
    d.wb.styles.get(sh.style_id(CellRef::parse(cell).unwrap())).font.bold
}

#[test]
fn phone_bold_sets_the_selected_cell_bold() {
    let mut app = app();
    app.run("cell.set", json!({"cell": "A1", "input": "Hello"})).unwrap();
    app.run("cell.set", json!({"cell": "A2", "input": "Hello"})).unwrap();
    app.run("selection.set", json!({"cell": "A1"})).unwrap();
    app.run("home.bold", json!({})).unwrap();
    assert!(bold(&app, "A1"), "A1 should be bold after the phone Bold");
    assert!(!bold(&app, "A2"), "A2 must stay regular");
}
