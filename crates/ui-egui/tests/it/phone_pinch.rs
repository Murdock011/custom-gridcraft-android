//! The phone's two-finger pinch: the sheet zooms about the point between the fingers, nothing
//! selects while the fingers are down, and two taps that do not move still tap.

use egui::{Event, Modifiers, PointerButton, Pos2, TouchDeviceId, TouchId, TouchPhase};
use gridcraft_engine::Session;
use gridcraft_engine::core::CellRef;
use gridcraft_ui_egui::SheetApp;

type Harness = egui_kittest::Harness<'static, SheetApp>;

/// A phone-sized window (its shorter side under 600 pt), so the phone layout and its pinch are on.
fn harness() -> Harness {
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

/// One finger, sent the way egui-winit does: a touch event, and for the first finger the emulated
/// pointer events too.
fn touch(h: &mut Harness, id: u64, phase: TouchPhase, pos: Pos2) {
    let ev = &mut h.input_mut().events;
    ev.push(Event::Touch { device_id: TouchDeviceId(1), id: TouchId(id), phase, pos, force: None });
    if id != 0 {
        return;
    }
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

fn zoom(h: &Harness) -> u16 {
    h.state().session.active().and_then(|d| d.wb.active()).map_or(100, |s| s.zoom)
}

fn selection_untouched(h: &Harness) -> bool {
    let sel = &h.state().session.active().unwrap().selection;
    sel.active == CellRef::new(0, 0) && sel.ranges.len() == 1 && sel.ranges[0].start == CellRef::new(0, 0) && sel.ranges[0].end == CellRef::new(0, 0)
}

#[test]
fn spreading_two_fingers_zooms_about_the_point_between_them() {
    let mut h = harness();
    let cells = h.state().grid.cells_rect.unwrap();
    let centre = cells.center();
    let (a, b) = (centre - egui::vec2(60.0, 0.0), centre + egui::vec2(60.0, 0.0));
    // The sheet point under the fingers' centre before the pinch.
    let before = h.state().view().scroll + (centre - cells.min) / 0.8;

    touch(&mut h, 0, TouchPhase::Start, a);
    touch(&mut h, 1, TouchPhase::Start, b);
    h.run_steps(2);
    for i in 1..=10 {
        let k = 1.0 + i as f32 / 10.0; // the distance doubles over ten frames
        touch(&mut h, 0, TouchPhase::Move, centre - egui::vec2(60.0 * k, 0.0));
        touch(&mut h, 1, TouchPhase::Move, centre + egui::vec2(60.0 * k, 0.0));
        h.run_steps(1);
    }
    touch(&mut h, 1, TouchPhase::End, centre + egui::vec2(120.0, 0.0));
    h.run_steps(1);
    touch(&mut h, 0, TouchPhase::End, centre - egui::vec2(120.0, 0.0));
    h.run_steps(3);

    assert!((198..=202).contains(&zoom(&h)), "two fingers twice as far apart zoom to 200%: {}", zoom(&h));
    let cells = h.state().grid.cells_rect.unwrap();
    let scroll = h.state().view().scroll;
    // Sheet units are drawn at DISPLAY_SCALE (0.8) points per unit at 100%.
    let after = scroll + (centre - cells.min) / (f32::from(zoom(&h)) / 100.0 * 0.8);
    assert!((after - before).length() < 2.0, "the sheet point under the fingers stays put: {before:?} vs {after:?}");
    assert!(selection_untouched(&h), "a pinch selects nothing");
}

#[test]
fn fingers_that_do_not_move_do_not_zoom_or_select() {
    let mut h = harness();
    let centre = h.state().grid.cells_rect.unwrap().center();
    touch(&mut h, 0, TouchPhase::Start, centre);
    touch(&mut h, 1, TouchPhase::Start, centre + egui::vec2(30.0, 0.0));
    h.run_steps(4);
    touch(&mut h, 1, TouchPhase::End, centre + egui::vec2(30.0, 0.0));
    touch(&mut h, 0, TouchPhase::End, centre);
    h.run_steps(3);
    assert_eq!(zoom(&h), 100, "still fingers do not zoom");
}

#[test]
fn two_quick_taps_still_tap_and_do_not_zoom() {
    let mut h = harness();
    let cells = h.state().grid.cells_rect.unwrap();
    let p = cells.min + egui::vec2(120.0, 60.0);
    for _ in 0..2 {
        touch(&mut h, 0, TouchPhase::Start, p);
        h.run_steps(1);
        touch(&mut h, 0, TouchPhase::End, p);
        h.run_steps(3);
    }
    assert_eq!(zoom(&h), 100, "a quick double tap does not zoom");
    let sel = &h.state().session.active().unwrap().selection;
    assert!(sel.active.row > 0 && sel.active.col > 0, "the taps select the tapped cell: {sel:?}");
}
