//! Type-to-edit: typing on a selected cell starts an edit, and no typed letter is lost, whether the
//! letters arrive one per frame or several in one frame, before or after Enter moves the selection.
use egui::{Event, Key, Modifiers};
use gridcraft_engine::Session;
use gridcraft_engine::core::CellRef;
use gridcraft_ui_egui::SheetApp;
use serde_json::json;

type Harness = egui_kittest::Harness<'static, SheetApp>;

fn harness() -> Harness {
    let mut s = Session::new();
    s.new_workbook();
    let mut h = Harness::builder().with_size(egui::vec2(412.0, 915.0)).build_ui_state(
        |ui, app: &mut SheetApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        SheetApp::new(s, Default::default()),
    );
    h.run_steps(4);
    h
}

fn select(h: &mut Harness, cell: &str) {
    h.state_mut().session.run("selection.set", json!({"cell": cell})).unwrap();
    h.run_steps(2);
}

fn enter(h: &mut Harness) {
    h.input_mut().events.push(Event::Key { key: Key::Enter, physical_key: Some(Key::Enter), pressed: true, repeat: false, modifiers: Modifiers::NONE });
    h.input_mut().events.push(Event::Key { key: Key::Enter, physical_key: Some(Key::Enter), pressed: false, repeat: false, modifiers: Modifiers::NONE });
    h.run_steps(3);
}

fn value(h: &Harness, a1: &str) -> String {
    let cell = CellRef::parse(a1).unwrap();
    let wb = &h.state().session.active().unwrap().wb;
    wb.active().unwrap().cell(cell).map(|c| c.input_text()).unwrap_or_default()
}

#[test]
fn letters_one_per_frame_are_all_kept() {
    let mut h = harness();
    select(&mut h, "A3");
    for ch in "Wednesday".chars() {
        h.input_mut().events.push(Event::Text(ch.to_string()));
        h.run_steps(1);
    }
    h.run_steps(2);
    enter(&mut h);
    assert_eq!(value(&h, "A3"), "Wednesday");
    for ch in "Friday".chars() {
        h.input_mut().events.push(Event::Text(ch.to_string()));
        h.run_steps(1);
    }
    h.run_steps(2);
    enter(&mut h);
    assert_eq!(value(&h, "A4"), "Friday");
}

#[test]
fn letters_several_in_one_frame_are_all_kept() {
    let mut h = harness();
    select(&mut h, "B2");
    h.input_mut().events.extend("Wednesday".chars().map(|c| Event::Text(c.to_string())));
    h.run_steps(2);
    enter(&mut h);
    assert_eq!(value(&h, "B2"), "Wednesday");
    h.input_mut().events.extend("Pending".chars().map(|c| Event::Text(c.to_string())));
    h.run_steps(2);
    enter(&mut h);
    assert_eq!(value(&h, "B3"), "Pending");
}

#[test]
fn word_in_one_text_event_then_next_cell_is_kept() {
    let mut h = harness();
    select(&mut h, "C1");
    h.input_mut().events.push(Event::Text("Thursday".into()));
    h.run_steps(2);
    enter(&mut h);
    h.input_mut().events.push(Event::Text("Wednesday".into()));
    h.run_steps(2);
    enter(&mut h);
    assert_eq!(value(&h, "C1"), "Thursday");
    assert_eq!(value(&h, "C2"), "Wednesday");
}

#[test]
fn letters_before_the_edit_has_focus_are_kept() {
    let mut h = harness();
    select(&mut h, "D1");
    // The first letter starts the edit; the rest arrive before any frame has focused the field.
    h.state_mut().begin_edit(Some("W".into()), false);
    h.input_mut().events.push(Event::Text("e".into()));
    h.run_steps(1);
    h.input_mut().events.push(Event::Text("dnesday".into()));
    h.run_steps(2);
    enter(&mut h);
    assert_eq!(value(&h, "D1"), "Wednesday");
}

#[test]
fn a_formula_typed_in_one_frame_keeps_its_letters() {
    let mut h = harness();
    select(&mut h, "E1");
    h.input_mut().events.extend("=SUM(1,2)".chars().map(|c| Event::Text(c.to_string())));
    h.run_steps(2);
    enter(&mut h);
    assert_eq!(value(&h, "E1"), "=SUM(1,2)");
}

#[test]
fn letters_kept_when_focus_is_on_the_editor_before_its_first_frame() {
    let mut h = harness();
    select(&mut h, "F1");
    h.state_mut().begin_edit(Some("W".into()), false);
    // The editor's id holds the focus, but no frame has shown the field yet.
    h.ctx.memory_mut(|m| m.request_focus(egui::Id::new("gridcraft.cell_editor")));
    h.input_mut().events.push(Event::Text("ednesday".into()));
    h.run_steps(2);
    enter(&mut h);
    assert_eq!(value(&h, "F1"), "Wednesday");
}

#[test]
fn letters_kept_when_another_widget_holds_the_focus_during_a_start() {
    let mut h = harness();
    select(&mut h, "G1");
    h.state_mut().begin_edit(Some("W".into()), false);
    h.ctx.memory_mut(|m| m.request_focus(egui::Id::new("some_other_widget")));
    h.input_mut().events.push(Event::Text("ednesday".into()));
    h.run_steps(2);
    enter(&mut h);
    assert_eq!(value(&h, "G1"), "Wednesday");
}
