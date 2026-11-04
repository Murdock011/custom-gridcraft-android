//! The phone's touch targets: the formula bar's controls, the Sheets button and the colour palette
//! are at least 48 pt (Material's minimum), and the formula bar has no ✕ or ✓ (the edit bar has them).

use egui::accesskit::Role;
use egui::{Event, Modifiers, MouseWheelSource, MouseWheelUnit, TouchPhase, pos2, vec2};
use egui_kittest::kittest::{NodeT, Queryable};
use gridcraft_engine::Session;
use gridcraft_ui_egui::SheetApp;

/// A phone-sized window (412 x 915 pt): the phone layout.
fn harness() -> egui_kittest::Harness<'static, SheetApp> {
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

#[test]
fn phone_touch_targets_are_at_least_48_pt() {
    let mut h = harness();

    // At rest: the Name Box is 48 pt tall; Insert Function and Expand formula bar are 48 x 48.
    let name = h.get_by_label("Name Box").rect();
    assert!(name.height() >= 48.0, "Name Box is {} pt tall", name.height());
    for label in ["Insert Function", "Expand formula bar"] {
        let r = h.get_by_label(label).rect();
        assert!(r.width() >= 48.0 && r.height() >= 48.0, "{label} is {} x {} pt", r.width(), r.height());
    }
    let sheets = h.get_by_label_contains("Sheets:").rect();
    assert!(sheets.height() >= 48.0, "the Sheets button is {} pt tall", sheets.height());
    // The phone's formula bar has no ✕ or ✓ at rest (the edit bar carries them while editing).
    assert!(h.query_by_label("Cancel").is_none(), "no Cancel in the formula bar on the phone");
    assert!(h.query_by_label("Enter").is_none(), "no Enter in the formula bar on the phone");

    // The Fill Colour palette: No Fill and More Colors… are full-width 48 pt buttons.
    gridcraft_ui_egui::open_phone_sheet(&h.ctx, "Home");
    h.run_steps(4);
    h.get_by_label("Fill Colour").click();
    h.run_steps(4);
    let no_fill = h.get_by_label("No Fill").rect();
    assert!(no_fill.height() >= 48.0, "No Fill is {} pt tall", no_fill.height());
    let more = h.get_by_label("More Colors…").rect();
    assert!(more.height() >= 48.0, "More Colors… is {} pt tall", more.height());
}

#[test]
fn more_colors_is_a_focused_page_that_fits_at_48_pt() {
    let mut h = harness();
    gridcraft_ui_egui::open_phone_sheet(&h.ctx, "Home");
    h.run_steps(4);
    h.get_by_label("Fill Colour").click();
    h.run_steps(4);

    // More Colors… is at the foot of the palette, so scroll the sheet to its end as a finger would.
    let sheet = pos2(206.0, 700.0);
    h.input_mut().events.push(Event::PointerMoved(sheet));
    h.run_steps(2);
    for _ in 0..10 {
        h.input_mut().events.push(Event::MouseWheel {
            unit: MouseWheelUnit::Point,
            delta: vec2(0.0, -1000.0),
            phase: TouchPhase::Move,
            modifiers: Modifiers::NONE,
            source: MouseWheelSource::Unknown,
        });
        h.run_steps(2);
    }
    h.get_by_label("More Colors…").click();
    h.run_steps(4);

    // The picker is the palette's only page: no No Fill, no More Colors… button.
    assert!(h.query_by_label("No Fill").is_none(), "No Fill is still shown under More Colors…");
    assert!(h.query_by_label("More Colors…").is_none(), "More Colors… is still shown under More Colors…");

    // Every button and text field in the sheet is at least 48 pt tall. The sheet covers the bottom
    // 55% of the screen (see `phone::sheet_panel`), and the bar under it sits in that band too.
    let screen = h.ctx.content_rect();
    let sheet_top = screen.bottom() - screen.height() * 0.55;
    let mut checked = 0;
    for role in [Role::Button, Role::TextInput, Role::SpinButton] {
        for n in h.query_all_by_role(role) {
            let r = n.rect();
            if r.center().y < sheet_top {
                continue;
            }
            checked += 1;
            assert!(r.height() >= 48.0, "{role:?} {:?} is {} pt tall", n.accesskit_node().label(), r.height());
        }
    }
    assert!(checked >= 3, "the picker has its OK, Cancel and Hex controls ({checked} checked)");
}

#[test]
fn more_colors_opens_alone_at_the_top_and_resets_on_close() {
    let mut h = harness();
    gridcraft_ui_egui::open_phone_sheet(&h.ctx, "Home");
    h.run_steps(4);
    h.get_by_label("Fill Colour").click();
    h.run_steps(4);

    // Scroll the sheet until More Colors… is in its body (below the title row), as a finger would.
    let sheet = pos2(206.0, 700.0);
    h.input_mut().events.push(Event::PointerMoved(sheet));
    h.run_steps(2);
    for _ in 0..25 {
        let y = h.get_by_label("More Colors…").rect().center().y;
        if (520.0..=820.0).contains(&y) {
            break;
        }
        h.input_mut().events.push(Event::MouseWheel {
            unit: MouseWheelUnit::Point,
            delta: vec2(0.0, -100.0),
            phase: TouchPhase::Move,
            modifiers: Modifiers::NONE,
            source: MouseWheelSource::Unknown,
        });
        h.run_steps(2);
    }
    h.get_by_label("More Colors…").click();
    h.run_steps(4);

    // The sheet shows only the picker: no command rows, and the picker fits the screen without scrolling.
    assert!(h.query_by_label("Bold").is_none(), "the Home rows are still shown under More Colors…");
    let screen = h.ctx.content_rect();
    // The OK button's label is padded ("  OK  ") as the desktop picker's is.
    for label in ["  OK  ", "Cancel"] {
        let r = h.get_by_label(label).rect();
        assert!(
            r.top() >= screen.top() && r.bottom() <= screen.bottom() && r.left() >= screen.left() && r.right() <= screen.right(),
            "{label} is outside the {}x{} screen: {r:?}",
            screen.width(),
            screen.height()
        );
        assert!(r.height() >= 48.0, "{label} is {} pt tall", r.height());
    }

    // Close the sheet; reopening Fill Colour starts at the swatches again, not the picker.
    h.get_by_label("Close").click();
    h.run_steps(4);
    gridcraft_ui_egui::open_phone_sheet(&h.ctx, "Home");
    h.run_steps(4);
    // The list keeps its scroll, so scroll it back to the top before tapping Fill Colour.
    h.input_mut().events.push(Event::PointerMoved(pos2(206.0, 700.0)));
    h.run_steps(2);
    for _ in 0..25 {
        h.input_mut().events.push(Event::MouseWheel {
            unit: MouseWheelUnit::Point,
            delta: vec2(0.0, 100.0),
            phase: TouchPhase::Move,
            modifiers: Modifiers::NONE,
            source: MouseWheelSource::Unknown,
        });
        h.run_steps(2);
    }
    h.get_by_label("Fill Colour").click();
    h.run_steps(4);
    assert!(h.query_by_label("No Fill").is_some(), "reopening Fill Colour shows the swatches, not the picker");
}
