//! The phone's touch selection: the action bar that a long press opens runs its commands, and its
//! dialog entries open the dialogs.

use egui::{Event, Modifiers, PointerButton, vec2};
use egui_kittest::kittest::Queryable;
use gridcraft_engine::Session;
use gridcraft_engine::core::CellRef;
use gridcraft_ui_egui::{SheetApp, phone_action_bar_open};
use serde_json::json;

fn harness(setup: impl FnOnce(&mut Session)) -> egui_kittest::Harness<'static, SheetApp> {
    harness_sized(setup, egui::vec2(412.0, 915.0))
}

/// The harness at `size`. A phone window (under 600 pt on its shorter side) gets the phone layout.
fn harness_sized(setup: impl FnOnce(&mut Session), size: egui::Vec2) -> egui_kittest::Harness<'static, SheetApp> {
    let mut s = Session::new();
    s.new_workbook();
    setup(&mut s);
    let mut h = egui_kittest::Harness::builder().with_size(size).build_ui_state(
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

/// A1:B2 selected, with A1 and B2 filled.
fn filled(s: &mut Session) {
    s.execute("cell.set", json!({"cell": "A1", "input": "one"})).expect("A1");
    s.execute("cell.set", json!({"cell": "B2", "input": "four"})).expect("B2");
    s.execute("selection.set", json!({"range": "A1:B2"})).expect("select");
}

/// Whether the cell at (row, col) of the first sheet holds a value.
fn filled_at(h: &egui_kittest::Harness<'static, SheetApp>, row: u32, col: u32) -> bool {
    h.state().session.active().and_then(|d| d.wb.sheet(0).and_then(|sh| sh.cell(CellRef::new(row, col)))).is_some_and(|c| !c.value.is_empty())
}

#[test]
fn clear_in_the_action_bar_clears_the_selection() {
    let mut h = harness(filled);
    gridcraft_ui_egui::open_phone_sheet(&h.ctx, "actionbar");
    h.run_steps(4);
    h.get_all_by_label("Clear").last().expect("action bar clear").click();
    h.run_steps(4);
    assert!(!filled_at(&h, 0, 0) && !filled_at(&h, 1, 1), "the selection's contents are cleared");
    assert!(!gridcraft_ui_egui::phone_action_bar_open(&h.ctx), "tapping an action closes the bar");
}

#[test]
fn delete_in_the_action_bar_opens_the_delete_cells_dialog() {
    // Delete is past the bar's visible slots: it sits in the More sheet.
    let mut h = harness(filled);
    gridcraft_ui_egui::open_phone_sheet(&h.ctx, "actionbar");
    h.run_steps(4);
    h.get_all_by_label("More").last().expect("action bar more").click();
    h.run_steps(4);
    h.get_all_by_label("Delete…").last().expect("more sheet delete").click();
    h.run_steps(6);
    assert!(filled_at(&h, 0, 0), "nothing is deleted until the dialog is confirmed");
    assert_eq!(h.state().dialog.as_ref().map(|d| d.name()), Some("deleteCells"));
}

#[test]
fn copy_in_the_action_bar_puts_the_selection_on_the_clipboard() {
    let mut h = harness(filled);
    gridcraft_ui_egui::open_phone_sheet(&h.ctx, "actionbar");
    h.run_steps(4);
    h.get_all_by_label("Copy").last().expect("action bar copy").click();
    h.run_steps(4);
    assert!(h.state().session.clipboard.is_some(), "Copy fills the clipboard");
    assert!(filled_at(&h, 0, 0), "Copy keeps the cells");
}

/// `cell`'s rectangle on the first sheet, in screen points.
fn cell_rect(h: &egui_kittest::Harness<'static, SheetApp>, cell: &str) -> egui::Rect {
    let app = h.state();
    let sh = app.session.active().unwrap().wb.active().unwrap();
    let geo = gridcraft_ui_egui::grid::Geo::new(sh, app.grid.rect.unwrap(), app.view().scroll);
    geo.cell_rect(sh, CellRef::parse(cell).unwrap())
}

fn cell_centre(h: &egui_kittest::Harness<'static, SheetApp>, cell: &str) -> egui::Pos2 {
    cell_rect(h, cell).center()
}

/// The centre of `cell`'s bottom-right handle dot: a handle radius (6 pt) diagonally past the corner.
fn bottom_right_dot(h: &egui_kittest::Harness<'static, SheetApp>, cell: &str) -> egui::Pos2 {
    cell_rect(h, cell).right_bottom() + vec2(6.0, 6.0)
}

/// A press on a selected cell that lands inside its top-left handle's 48 pt touch square (the handle's
/// dot is 16 pt away, so the press is on the cell, not on the dot).
fn press_in_handle_square(h: &egui_kittest::Harness<'static, SheetApp>, cell: &str) -> egui::Pos2 {
    cell_rect(h, cell).left_top() + vec2(16.0, 10.0)
}

/// A finger's tap at `at`: the pointer arrives, then presses and lifts on frames of their own.
fn tap(h: &mut egui_kittest::Harness<'static, SheetApp>, at: egui::Pos2) {
    h.input_mut().events.push(Event::PointerMoved(at));
    h.run_steps(2);
    for pressed in [true, false] {
        h.input_mut().events.push(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        h.run_steps(2);
    }
}

/// Presses at `at` and holds still until the long press opens the action bar (or gives up).
fn hold_until_bar_opens(h: &mut egui_kittest::Harness<'static, SheetApp>, at: egui::Pos2) {
    h.input_mut().events.push(Event::PointerMoved(at));
    h.run_steps(2);
    h.input_mut().events.push(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    for _ in 0..20 {
        h.run_steps(1);
        if phone_action_bar_open(&h.ctx) {
            break;
        }
    }
}

fn release(h: &mut egui_kittest::Harness<'static, SheetApp>, at: egui::Pos2) {
    h.input_mut().events.push(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(4);
}

#[test]
fn a_long_press_on_the_selected_cell_opens_the_action_bar() {
    let mut h = harness(|_| {});
    let b2 = cell_centre(&h, "B2");
    tap(&mut h, b2);
    assert_eq!(h.state().session.active().unwrap().selection.active.a1(), "B2", "a tap selects B2");

    // The press starts inside B2's top-left handle's touch square, so the handle sees it first.
    let at = press_in_handle_square(&h, "B2");
    hold_until_bar_opens(&mut h, at);
    assert!(phone_action_bar_open(&h.ctx), "a long press on the selected cell opens the action bar");
    h.run_steps(2);
    assert!(h.query_all_by_label("Cut").next().is_some(), "the action bar shows its Cut button");
    release(&mut h, at);
}

#[test]
fn a_handle_drag_still_extends_the_selection() {
    let mut h = harness(|_| {});
    let at = cell_centre(&h, "B2");
    tap(&mut h, at);
    // Press on B2's bottom-right handle dot and move well past the drag threshold, in one step, to D4.
    let corner = bottom_right_dot(&h, "B2");
    let to = cell_centre(&h, "D4");
    h.input_mut().events.push(Event::PointerMoved(corner));
    h.run_steps(2);
    h.input_mut().events.push(Event::PointerButton { pos: corner, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.run_steps(2);
    h.input_mut().events.push(Event::PointerMoved(to));
    h.run_steps(3);
    h.input_mut().events.push(Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(4);
    assert!(!phone_action_bar_open(&h.ctx), "a handle drag opens no action bar");
    assert_eq!(h.state().session.active().unwrap().selection.current().a1(), "B2:D4", "the drag extends the selection to D4");
}

#[test]
fn a_tap_beside_a_handle_selects_the_cell_there() {
    let mut h = harness(|_| {});
    let a1 = cell_centre(&h, "A1");
    tap(&mut h, a1);
    // 8 pt right of A1's bottom-right corner, a little above the row line: inside B1's left part, and
    // inside the handle's touch square, so the handle takes the tap.
    let corner = cell_rect(&h, "A1").right_bottom();
    tap(&mut h, corner + vec2(8.0, -5.0));
    assert_eq!(h.state().session.active().unwrap().selection.active.a1(), "B1", "the tap inside B1 selects B1");
    assert!(!phone_action_bar_open(&h.ctx), "a tap opens no action bar");
}

#[test]
fn the_bottom_right_handle_dot_is_outside_the_selection() {
    // The dot sits a handle radius past the selection's corner, so it never covers the cell's last digit.
    let mut h = harness(|_| {});
    let b2 = cell_centre(&h, "B2");
    tap(&mut h, b2);
    let selection = cell_rect(&h, "B2");
    let dot = bottom_right_dot(&h, "B2");
    assert!(dot.x > selection.right() && dot.y > selection.bottom(), "the dot {dot:?} is outside B2 {selection:?}");
}

#[test]
fn a_drag_from_a_handle_extends_the_selection_to_the_cell() {
    let mut h = harness(|_| {});
    let a1 = cell_centre(&h, "A1");
    tap(&mut h, a1);
    let corner = bottom_right_dot(&h, "A1");
    let to = cell_centre(&h, "C3");
    h.input_mut().events.push(Event::PointerMoved(corner));
    h.run_steps(2);
    h.input_mut().events.push(Event::PointerButton { pos: corner, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.run_steps(2);
    h.input_mut().events.push(Event::PointerMoved(to));
    h.run_steps(3);
    h.input_mut().events.push(Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(4);
    assert_eq!(h.state().session.active().unwrap().selection.current().a1(), "A1:C3", "the drag extends the selection to C3");
}

#[test]
fn the_comment_popup_is_not_drawn_under_the_action_bar() {
    let mut h = harness(|s| {
        s.execute("review.newComment", json!({"cell": "B2", "text": "Zebra note", "author": "Ada"})).expect("comment");
    });
    let b2 = cell_centre(&h, "B2");
    tap(&mut h, b2);
    h.run_steps(8);
    assert!(h.query_all_by_label_contains("Zebra note").next().is_some(), "the comment popup shows beside the selected cell");

    let at = press_in_handle_square(&h, "B2");
    hold_until_bar_opens(&mut h, at);
    assert!(phone_action_bar_open(&h.ctx), "the long press opens the action bar");
    h.run_steps(2);
    assert!(h.query_all_by_label_contains("Zebra note").next().is_none(), "the comment popup is not drawn while the action bar is open");
    release(&mut h, at);
}
