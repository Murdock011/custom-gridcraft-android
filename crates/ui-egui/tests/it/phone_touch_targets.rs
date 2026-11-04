//! The phone's touch targets: the formula bar's controls, the Sheets button and the colour palette
//! are at least 48 pt (Material's minimum), and the formula bar has no ✕ or ✓ (the edit bar has them).

use egui_kittest::kittest::Queryable;
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
