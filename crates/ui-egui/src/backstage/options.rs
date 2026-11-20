use egui::{Align2, Key, Modifiers, Rect, Sense, Stroke, pos2, vec2};
use serde_json::json;

use super::{Backstage, Icon, Language, SheetApp, Tokens, Ui, dim, i18n, icons, section, theme};

/// Excel's File › Options page: General (display theme, interface language and Show formula bar),
/// Editing (AutoComplete) and Save (the autosave checkbox toggling `app.ui.autosave`), plus Formulas
/// (calculation mode and iterative calculation) and the display options for this workbook (sheet
/// tabs and scroll bars) when a document is active. Theme, language, formula
/// bar, AutoComplete and calculation changes run their commands through `run_or_alert`; calculation
/// settings are saved with the workbook.
///
/// On a phone (`phone::active`) the page is a list of the four categories, and each category opens
/// as its own page (`bs.options_category`). The switches there are 48 pt rows (`toggle_row`).
pub(super) fn options(app: &mut SheetApp, ui: &mut Ui, bs: &mut Backstage) {
    let lang = app.ui.language;
    if !crate::phone::active(ui.ctx()) {
        section(ui, lang, "Options|General");
        general_options(app, ui, lang);
        ui.add_space(6.0);
        formula_bar_option(app, ui, lang);
        ui.add_space(18.0);
        editing_option(app, ui, lang);
        ui.add_space(18.0);
        save_option(app, ui, lang);
        formulas_options(app, ui, lang);
        return;
    }
    let ctx = ui.ctx().clone();
    // Escape (Back) from a category returns to the list. From the list Escape is left alone, so `show`
    // closes the Backstage with it (the same as the top bar's Back).
    if bs.options_category.is_some() && !egui::Popup::is_any_open(&ctx) && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
        bs.options_category = None;
    }
    let Some(index) = bs.options_category else {
        let t = Tokens::get(&ctx);
        for (i, (label, icon)) in super::OPTION_CATEGORIES.iter().enumerate() {
            if category_row(ui, t, lang.tr(label), *icon).clicked() {
                bs.options_category = Some(i);
            }
        }
        return;
    };
    match index {
        0 => {
            section(ui, lang, "Options|General");
            general_options(app, ui, lang);
            ui.add_space(6.0);
            formula_bar_option(app, ui, lang);
        }
        1 => {
            section(ui, lang, "Options|Editing");
            editing_option(app, ui, lang);
        }
        2 => {
            section(ui, lang, "Options|Save");
            save_option(app, ui, lang);
        }
        _ => formulas_options(app, ui, lang),
    }
}

/// Show formula bar (General).
fn formula_bar_option(app: &mut SheetApp, ui: &mut Ui, lang: Language) {
    let mut formula_bar = app.session.prefs.formula_bar;
    if option_check(ui, &mut formula_bar, lang.tr("Show formula bar")) {
        app.run_or_alert("view.formulaBar", json!({"on": formula_bar}));
    }
    let mut highlight_row = app.ui.highlight_active_row;
    if option_check(ui, &mut highlight_row, lang.tr("Highlight the active cell's row")) {
        app.run_or_alert("options.highlightActiveRow", json!({"on": highlight_row}));
    }
}

/// AutoComplete for cell values (Editing).
fn editing_option(app: &mut SheetApp, ui: &mut Ui, lang: Language) {
    let mut autocomplete = app.ui.autocomplete;
    if option_check(ui, &mut autocomplete, lang.tr("Enable AutoComplete for cell values")) {
        app.run_or_alert("options.autoComplete", json!({"on": autocomplete}));
    }
}

/// AutoSave for .xlsx workbooks (Save).
fn save_option(app: &mut SheetApp, ui: &mut Ui, lang: Language) {
    let mut autosave = app.ui.autosave;
    if option_check(ui, &mut autosave, lang.tr("Save .xlsx workbooks automatically after every change")) {
        app.toggle_autosave();
    }
}

/// Formulas: the calculation mode and iterative calculation. Needs an open workbook; without one
/// the section is left out.
fn formulas_options(app: &mut SheetApp, ui: &mut Ui, lang: Language) {
    let Some((calc, view)) = app.session.active().map(|d| (d.wb.calc.clone(), d.wb.view)) else { return };
    ui.add_space(18.0);
    // Excel's Advanced › Display options for this workbook: saved with the workbook (#3101).
    section(ui, lang, "Display options for this workbook");
    for (on, label, id) in [
        (view.show_horizontal_scroll, "Show horizontal scroll bar", "view.showHorizontalScrollBar"),
        (view.show_vertical_scroll, "Show vertical scroll bar", "view.showVerticalScrollBar"),
        (view.show_sheet_tabs, "Show sheet tabs", "view.showSheetTabs"),
    ] {
        let mut on = on;
        if option_check(ui, &mut on, lang.tr(label)) {
            app.run_or_alert(id, json!({"on": on}));
        }
    }
    ui.add_space(18.0);
    section(ui, lang, "Formulas");
    ui.label(lang.tr("Workbook calculation"));
    calc_options(app, ui, lang, &calc);
    ui.add_space(6.0);
    dim(ui, lang.tr("Calculation settings are saved with the workbook."));
}

/// A checkbox on the desktop, a 48 pt switch row on a phone. True when the value changed.
fn option_check(ui: &mut Ui, value: &mut bool, text: &str) -> bool {
    if crate::phone::active(ui.ctx()) {
        crate::phone::toggle_row(ui, Tokens::get(ui.ctx()), value, text).changed()
    } else {
        ui.checkbox(value, text).changed()
    }
}

/// A 48 pt category row with its icon and a chevron (the phone's Options list).
fn category_row(ui: &mut Ui, t: Tokens, label: &str, icon: Icon) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 48.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
    let p = ui.painter();
    if resp.is_pointer_button_down_on() {
        p.rect_filled(r, 0.0, t.pressed);
    } else if resp.hovered() {
        p.rect_filled(r, 0.0, t.hover);
    }
    icons::paint(p, Rect::from_center_size(pos2(r.left() + 24.0, r.center().y), vec2(24.0, 24.0)), icon, t.text);
    p.text(pos2(r.left() + 60.0, r.center().y), Align2::LEFT_CENTER, label, theme::ui_font(16.0), t.text);
    let c = pos2(r.right() - 28.0, r.center().y);
    let stroke = Stroke::new(2.0, t.text_dim);
    p.line_segment([c + vec2(-3.0, -6.0), c + vec2(3.0, 0.0)], stroke);
    p.line_segment([c + vec2(3.0, 0.0), c + vec2(-3.0, 6.0)], stroke);
    resp
}

/// One choice of a phone setting: a 56 pt row with a radio mark, the title and a line under it.
/// True when tapped.
fn choice_row(ui: &mut Ui, t: Tokens, title: &str, line: &str, on: bool) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 56.0), Sense::click());
    if resp.is_pointer_button_down_on() {
        ui.painter().rect_filled(r, 0.0, t.pressed);
    }
    let c = pos2(r.left() + 14.0, r.center().y);
    ui.painter().circle_stroke(c, 10.0, Stroke::new(2.0, if on { t.accent } else { t.text_dim }));
    if on {
        ui.painter().circle_filled(c, 5.5, t.accent);
    }
    ui.painter().text(pos2(r.left() + 40.0, r.center().y - 9.0), Align2::LEFT_CENTER, title, theme::ui_font(16.0), t.text);
    ui.painter().text(pos2(r.left() + 40.0, r.center().y + 12.0), Align2::LEFT_CENTER, line, theme::ui_font(12.0), t.text_dim);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::RadioButton, true, on, title));
    resp.clicked()
}

/// A phone setting's heading, in the accent colour.
fn setting_heading(ui: &mut Ui, t: Tokens, text: &str) {
    ui.label(egui::RichText::new(text).font(theme::ui_bold(13.0)).color(t.accent));
    ui.add_space(4.0);
}

/// The phone's Theme setting (the shared shell): Light (the default), Dark (the whole app, the grid
/// included) and System (follows the phone); then Phone layout: Tiles (the default) or List.
fn theme_rows(app: &mut SheetApp, ui: &mut Ui, lang: Language) {
    let t = Tokens::get(ui.ctx());
    setting_heading(ui, t, lang.tr("Theme"));
    let mode = app.ui.theme_mode();
    for (value, title, line) in [
        ("light", "Light", "The default"),
        ("dark", "Dark", "The whole app, the grid included"),
        ("system", "System", "Follows the phone's dark theme setting"),
    ] {
        if choice_row(ui, t, lang.tr(title), lang.tr(line), mode == value) && mode != value {
            app.run_or_alert("view.theme", json!({"mode": value}));
        }
    }
    ui.add_space(12.0);
    setting_heading(ui, t, lang.tr("Phone layout"));
    ui.label(egui::RichText::new(lang.tr("Toolbar sheets")).color(t.text_dim));
    let list = app.ui.phone_list_style;
    for (style, title, line, on) in
        [("tiles", "Tiles", "Icons in a grid, grouped by section", !list), ("list", "List", "One command per row, with a tab menu at the top", list)]
    {
        if choice_row(ui, t, lang.tr(title), lang.tr(line), on) && !on {
            app.run_or_alert("ui.phoneSheetStyle", json!({"style": style}));
        }
    }
    ui.add_space(12.0);
}

/// The display theme and interface language controls of the General section.
fn general_options(app: &mut SheetApp, ui: &mut Ui, lang: Language) {
    // A phone stacks each label above its full-width combo box (one column), as before the merge.
    let phone = crate::phone::active(ui.ctx());
    let columns = if phone { 1 } else { 2 };
    let width = if phone { ui.available_width() } else { 160.0 };
    if phone {
        theme_rows(app, ui, lang);
    }
    egui::Grid::new("backstage_options_general").num_columns(columns).spacing(vec2(18.0, 10.0)).show(ui, |ui| {
        if !phone {
            let label = ui.label(lang.tr("Display theme"));
            let mode = app.ui.theme_mode();
            let current = match mode {
                "system" => "System",
                "dark" => "Dark",
                _ => "Light",
            };
            craft_ui::ComboBox::from_id_salt("backstage_theme")
                .width(width)
                .selected_text(lang.tr(current))
                .show_ui(ui, |ui| {
                    for (value, label) in [("system", "System"), ("light", "Light"), ("dark", "Dark")] {
                        if ui.selectable_label(mode == value, lang.tr(label)).clicked() {
                            app.run_or_alert("view.theme", json!({"mode": value}));
                        }
                    }
                })
                .response
                .labelled_by(label.id);
            ui.end_row();
        }
        let label = ui.label(lang.tr("Interface language"));
        if phone {
            ui.end_row();
        }
        craft_ui::ComboBox::from_id_salt("backstage_language")
            .width(width)
            .selected_text(lang.name())
            .show_ui(ui, |ui| {
                for l in i18n::Language::for_picker() {
                    if ui.selectable_label(l == lang, l.name()).clicked() {
                        app.run_or_alert("app.language.set", json!({"language": l.code()}));
                    }
                }
            })
            .response
            .labelled_by(label.id);
        ui.end_row();
    });
}

/// The calculation mode, and iterative calculation, of the active workbook.
fn calc_options(app: &mut SheetApp, ui: &mut Ui, lang: Language, calc: &gridcraft_engine::model::CalcSettings) {
    use gridcraft_engine::model::CalcMode;
    for (mode, id, label) in [
        (CalcMode::Automatic, "automatic", "Automatic"),
        (CalcMode::AutomaticExceptTables, "automaticExceptTables", "Automatic Except for Data Tables"),
        (CalcMode::Manual, "manual", "Manual"),
    ] {
        if ui.radio(calc.mode == mode, lang.tr(label)).clicked() && calc.mode != mode {
            app.run_or_alert("formulas.calculationOptions", json!({"mode": id}));
        }
    }
    ui.add_space(4.0);
    let mut iterative = calc.iterative;
    if option_check(ui, &mut iterative, lang.tr("Enable iterative calculation")) {
        app.run_or_alert("formulas.calculationOptions", json!({"iterative": iterative}));
    }
    // Maximum Iterations and Maximum Change apply only while iteration is on, as in Excel.
    iteration_options(app, ui, lang, calc, iterative);
    // TODO: add "Set precision as displayed" here once `formulas.calculationOptions` takes
    // `precisionAsDisplayed` (PR #1366); the model already stores it.
}

/// Maximum Iterations and Maximum Change, live only while iterative calculation is on.
fn iteration_options(app: &mut SheetApp, ui: &mut Ui, lang: Language, calc: &gridcraft_engine::model::CalcSettings, iterative: bool) {
    ui.add_enabled_ui(iterative, |ui| {
        ui.horizontal(|ui| {
            ui.add_space(24.0);
            egui::Grid::new("backstage_options_iteration").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
                let label = ui.label(lang.tr("Maximum Iterations:"));
                let id = ui.make_persistent_id("backstage_max_iterations");
                // Text left in the field when iteration is switched off (it loses focus) is dropped.
                let committed = crate::widgets::committed_text(ui, id, &calc.max_iterations.to_string(), Some(80.0));
                if let Some(response) = ui.ctx().read_response(id) {
                    response.labelled_by(label.id);
                }
                if let Some(text) = committed.filter(|_| iterative) {
                    match parse_max_iterations(&text) {
                        Some(n) if n != calc.max_iterations => {
                            app.run_or_alert("formulas.calculationOptions", json!({"maxIterations": n}));
                        }
                        Some(_) => {}
                        None => refuse(app, ui, "Maximum Iterations must be a whole number from 1 to 32767."),
                    }
                }
                ui.end_row();
                let label = ui.label(lang.tr("Maximum Change:"));
                let id = ui.make_persistent_id("backstage_max_change");
                let committed = crate::widgets::committed_text(ui, id, &calc.max_change.to_string(), Some(80.0));
                if let Some(response) = ui.ctx().read_response(id) {
                    response.labelled_by(label.id);
                }
                if let Some(text) = committed.filter(|_| iterative) {
                    match parse_max_change(&text, lang.formula_locale()) {
                        Some(d) if d != calc.max_change => {
                            app.run_or_alert("formulas.calculationOptions", json!({"maxChange": d}));
                        }
                        Some(_) => {}
                        None => refuse(app, ui, "Maximum Change must be a number of 0 or more."),
                    }
                }
                ui.end_row();
            });
        });
    });
}

/// Refuses a typed setting with a message, leaving the workbook as it was. The Enter that
/// committed the field is consumed so it doesn't also dismiss the message box in this frame.
fn refuse(app: &mut SheetApp, ui: &mut Ui, text: &str) {
    ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
    app.message = Some(("GridCraft".into(), app.ui.language.tr(text).into()));
}

/// Maximum Iterations as typed: a whole number from 1 to 32767, or `None` (Excel refuses others).
fn parse_max_iterations(text: &str) -> Option<u32> {
    let n = text.trim().parse::<u32>().ok()?;
    (1..=gridcraft_engine::model::CalcSettings::MAX_ITERATIONS).contains(&n).then_some(n)
}

/// Maximum Change as typed: a finite number of 0 or more, or `None`. The interface's decimal
/// separator is accepted as well as a dot.
fn parse_max_change(text: &str, locale: gridcraft_engine::formula::FormulaLocale) -> Option<f64> {
    let d = text.trim().replace(locale.decimal_separator(), ".").parse::<f64>().ok()?;
    (d.is_finite() && d >= 0.0).then_some(d)
}

/// The platform the app was built for, e.g. "macOS (aarch64)"; the web build reports "Web".
pub(super) fn platform() -> String {
    let os = match std::env::consts::OS {
        "macos" => "macOS",
        "windows" => "Windows",
        "linux" => "Linux",
        "freebsd" => "FreeBSD",
        "openbsd" => "OpenBSD",
        "netbsd" => "NetBSD",
        _ if cfg!(target_arch = "wasm32") => return "Web".into(),
        other => other,
    };
    format!("{os} ({})", std::env::consts::ARCH)
}
