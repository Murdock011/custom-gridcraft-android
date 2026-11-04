//! The phone's Insert > Comment opens the comment dialog (a focused text box, nothing created until
//! OK), and Data > Filter's arrows take 48 pt presses and open a bottom sheet with 48 pt rows that
//! does not focus its search box. Android Back (Escape) closes the filter menu first.

use egui::{Event, Modifiers, PointerButton, vec2};
use egui_kittest::kittest::Queryable;
use gridcraft_engine::Session;
use gridcraft_engine::core::CellRef;
use gridcraft_ui_egui::{SheetApp, grid, open_phone_sheet};
use serde_json::json;

type Harness = egui_kittest::Harness<'static, SheetApp>;

/// A phone-sized window (412 x 915 pt): the phone layout.
fn harness(session: Session) -> Harness {
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(412.0, 915.0)).build_ui_state(
        |ui, app: &mut SheetApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        SheetApp::new(session, Default::default()),
    );
    h.run_steps(6);
    h
}

/// A finger tap: the pointer arrives, then presses and lifts on frames of their own.
fn tap(h: &mut Harness, at: egui::Pos2) {
    h.input_mut().events.push(Event::PointerMoved(at));
    h.run_steps(2);
    for pressed in [true, false] {
        h.input_mut().events.push(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        h.run_steps(2);
    }
}

fn fruit_session() -> Session {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("range.setValues", json!({"range": "A1", "values": [["Fruit"], ["Apple"], ["Banana"], ["Cherry"]]})).unwrap();
    s.execute("data.filter", json!({"range": "A1:A4"})).unwrap();
    s.execute("selection.set", json!({"cell": "A1"})).unwrap();
    s
}

/// The centre of the filter arrow on A1, in screen points.
fn a1_arrow(h: &Harness) -> egui::Pos2 {
    let app = h.state();
    let sh = app.session.active().unwrap().wb.active().unwrap();
    let geo = grid::Geo::new(sh, app.grid.rect.unwrap(), app.view().scroll);
    grid::filter_button_rect(&geo, sh, CellRef::parse("A1").unwrap()).center()
}

#[test]
fn insert_comment_opens_a_typing_dialog_and_creates_a_comment_on_ok() {
    let mut h = harness(Session::new());
    open_phone_sheet(&h.ctx, "Insert");
    h.run_steps(4);
    h.get_by_label("Comment").click();
    h.run_steps(6);

    assert_eq!(h.state().dialog.as_ref().map(|d| d.name.as_str()), Some("comment"), "Insert > Comment opens the comment dialog");
    let sheet = h.state().session.active().unwrap().wb.active().unwrap();
    assert!(sheet.comments.is_empty(), "nothing is created until OK");

    // The text box has the focus, so typing goes into it and not into the cell.
    h.input_mut().events.push(Event::Text("Hello".into()));
    h.run_steps(4);
    assert_eq!(h.state().dialog.as_ref().and_then(|d| d.values.get("text")).and_then(|v| v.as_str()), Some("Hello"), "typing goes to the comment");

    h.get_by_label("OK").click();
    h.run_steps(6);
    assert!(h.state().dialog.is_none(), "OK closes the dialog");
    let doc = h.state().session.active().unwrap();
    let sheet = doc.wb.active().unwrap();
    let c = sheet.comments.get(&doc.selection.active).expect("OK creates the comment on the active cell");
    assert_eq!(c.text, "Hello");
    assert!(c.threaded, "Insert > Comment makes a threaded comment, as the desktop does");
    assert_eq!(gridcraft_engine::display::cell_text(&doc.wb, sheet, doc.selection.active), "", "the cell value is unchanged");
}

#[test]
fn filter_arrow_opens_a_touch_sheet_that_does_not_take_focus() {
    let mut h = harness(fruit_session());
    let at = a1_arrow(&h);
    tap(&mut h, at);
    assert!(h.state().grid.filter_menu.is_some(), "the arrow opens the filter menu");

    for label in ["Sort Ascending", "Sort Descending", "(Select All)", "Apple", "Banana", "Clear Filter", "Apply"] {
        let r = h.get_by_label(label).rect();
        assert!(r.height() >= 48.0, "{label} is {} pt tall", r.height());
    }
    let close = h.get_by_label("Close").rect();
    assert!(close.width() >= 48.0 && close.height() >= 48.0, "Close is {} x {} pt", close.width(), close.height());
    assert!(h.ctx.memory(|m| m.focused()).is_none(), "the search box is not focused on the phone");

    // Back (Escape) closes the menu first.
    assert!(h.state().can_go_back(&h.ctx), "an open filter menu is what Back closes first");
    h.key_press(egui::Key::Escape);
    h.run_steps(4);
    assert!(h.state().grid.filter_menu.is_none(), "Back closes the filter menu");
}

#[test]
fn a_tap_beside_the_arrow_still_opens_the_menu() {
    let mut h = harness(fruit_session());
    // 16 pt right of the arrow: outside the arrow, inside its 48 pt touch square (in column B).
    let at = a1_arrow(&h) + vec2(16.0, 0.0);
    tap(&mut h, at);
    assert!(h.state().grid.filter_menu.is_some(), "the 48 pt square takes the press");
}
