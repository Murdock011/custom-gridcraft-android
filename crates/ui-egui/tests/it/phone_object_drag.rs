//! On the phone, a one-finger drag that starts on the selected chart's handle resizes it and a drag
//! from its body moves it; the grid does not pan for either (a press elsewhere still pans).

use egui::{Event, Modifiers, PointerButton, Pos2, TouchDeviceId, TouchId, TouchPhase, vec2};
use gridcraft_engine::Session;
use gridcraft_engine::model::Anchor;
use gridcraft_ui_egui::{SheetApp, grid::Geo};
use serde_json::json;

type Harness = egui_kittest::Harness<'static, SheetApp>;

/// A phone-sized window (412 x 915 pt) with a chart at B2 that is selected.
fn fixture() -> (Harness, u32) {
    let mut session = Session::new();
    session.new_workbook();
    let chart = session.execute("insert.chart", json!({"type": "column", "at": "B2", "width": 160, "height": 120})).unwrap();
    let id = chart["chart"].as_u64().unwrap() as u32;
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(412.0, 915.0)).build_ui_state(
        |ui, app: &mut SheetApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        SheetApp::new(session, Default::default()),
    );
    h.run_steps(6);
    h.state_mut().select_object_id(Some(id));
    h.run_steps(3);
    (h, id)
}

fn anchor(h: &Harness, id: u32) -> Anchor {
    h.state().session.doc().unwrap().wb.active().unwrap().charts.iter().find(|c| c.id == id).unwrap().anchor
}

fn scroll(h: &Harness) -> egui::Vec2 {
    h.state().view().scroll
}

/// Bottom-right corner of the chart, on screen.
fn corner(h: &Harness, id: u32) -> Pos2 {
    let app = h.state();
    let sh = app.session.doc().unwrap().wb.active().unwrap();
    let geo = Geo::new(sh, app.grid.rect.unwrap(), app.view().scroll);
    let a = anchor(h, id);
    let min = geo.cell_rect(sh, a.cell).min + vec2(a.dx, a.dy) * geo.z;
    min + vec2(a.width, a.height) * geo.z
}

/// One finger, sent the way egui-winit does: a touch event plus the emulated pointer events.
fn touch(h: &mut Harness, phase: TouchPhase, pos: Pos2) {
    let ev = &mut h.input_mut().events;
    ev.push(Event::Touch { device_id: TouchDeviceId(1), id: TouchId(0), phase, pos, force: None });
    match phase {
        TouchPhase::Start => {
            ev.push(Event::PointerMoved(pos));
            ev.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
        }
        TouchPhase::Move => ev.push(Event::PointerMoved(pos)),
        TouchPhase::End | TouchPhase::Cancel => {
            ev.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
            ev.push(Event::PointerGone);
        }
    }
}

fn drag(h: &mut Harness, from: Pos2, by: egui::Vec2) {
    touch(h, TouchPhase::Start, from);
    h.run_steps(2);
    for i in 1..=10 {
        touch(h, TouchPhase::Move, from + by * (i as f32 / 10.0));
        h.run_steps(1);
    }
    touch(h, TouchPhase::End, from + by);
    h.run_steps(3);
}

#[test]
fn a_touch_drag_from_the_chart_handle_resizes_it_without_panning() {
    let (mut h, id) = fixture();
    let before = anchor(&h, id);
    let scroll0 = scroll(&h);
    let start = corner(&h, id);
    drag(&mut h, start, vec2(-60.0, 80.0));
    let after = anchor(&h, id);
    assert!(after.width < before.width, "width shrank: {} -> {}", before.width, after.width);
    assert!(after.height > before.height, "height grew: {} -> {}", before.height, after.height);
    assert_eq!(scroll(&h), scroll0, "the grid did not pan");
    assert_eq!(after.cell, before.cell, "a handle drag keeps the anchor cell");
}

#[test]
fn a_touch_drag_from_the_chart_body_moves_it_without_panning() {
    let (mut h, id) = fixture();
    let before = anchor(&h, id);
    let scroll0 = scroll(&h);
    let a = anchor(&h, id);
    let app = h.state();
    let sh = app.session.doc().unwrap().wb.active().unwrap();
    let geo = Geo::new(sh, app.grid.rect.unwrap(), app.view().scroll);
    let min = geo.cell_rect(sh, a.cell).min + vec2(a.dx, a.dy) * geo.z;
    let centre = min + vec2(a.width, a.height) * geo.z * 0.5;
    drag(&mut h, centre, vec2(40.0, 30.0));
    let after = anchor(&h, id);
    assert!(after.cell != before.cell || after.dx != before.dx || after.dy != before.dy, "the chart moved: {before:?} -> {after:?}");
    assert_eq!((after.width, after.height), (before.width, before.height), "a move keeps the size");
    assert_eq!(scroll(&h), scroll0, "the grid did not pan");
}
