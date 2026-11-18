//! The phone's tab sheets as tiles (the shared shell, phone plan part 1.6): each tab is a list of
//! sections, each section a grid of tiles. A tile is an icon above a label of up to two lines. The
//! label and whether the tile is enabled come from the engine's command (`cmd::find_command`), so a
//! tile says what the ribbon says. Formatting tiles keep the sheet open; tiles that insert something
//! or open a dialog close it. The model here is the only list of the sheets' commands: the List
//! style draws the same model.

use egui::{Align2, Context, CornerRadius, Pos2, Rect, Response, Sense, Ui, vec2};
use gridcraft_engine::cmd;
use gridcraft_engine::model::{HAlign, Style, Underline, VAlign};
use serde_json::json;

use super::{Act, GUTTER, SubPage, TOUCH, close_all, run_act, set_palette};
use crate::SheetApp;
use crate::icons::{self, Icon};
use crate::theme::{self, Tokens};

/// The narrowest a tile may be: the sheet divides its width into whole tiles of at least this.
pub(super) const TILE_MIN_W: f32 = 76.0;
/// A tile: the icon, then a label of up to two lines.
pub(super) const TILE_H: f32 = 76.0;
const _: () = assert!(TILE_MIN_W >= TOUCH && TILE_H >= TOUCH);

/// One command in a sheet.
#[derive(Clone, Copy)]
pub(super) struct Item {
    /// The engine command whose label and enabled state the tile shows. Empty for a sub-page that
    /// stands for several commands (its label is then `label`).
    pub cmd: &'static str,
    pub icon: Icon,
    pub act: Act,
    /// Whether the sheet stays open after a tap (formatting); otherwise it closes.
    pub keep: bool,
    /// The label when the engine's would be wrong here (a sub-page, a zoom step).
    pub label: Option<&'static str>,
}

/// A command that closes the sheet after it runs.
const fn run(cmd: &'static str, icon: Icon) -> Item {
    Item { cmd, icon, act: Act::Run(cmd), keep: false, label: None }
}

/// A formatting command: the sheet stays open, so another format can follow.
const fn fmt(cmd: &'static str, icon: Icon) -> Item {
    Item { cmd, icon, act: Act::Run(cmd), keep: true, label: None }
}

/// A command that opens a dialog, by its dialog name.
const fn dialog(cmd: &'static str, name: &'static str, icon: Icon) -> Item {
    Item { cmd, icon, act: Act::Dialog(name, None), keep: false, label: None }
}

/// A sub-page of the sheet (a colour grid or a list of choices).
const fn page(cmd: &'static str, page: SubPage, icon: Icon, label: Option<&'static str>) -> Item {
    Item { cmd, icon, act: Act::Page(page), keep: true, label }
}

/// A section of a sheet: a heading and its tiles.
pub(super) struct Section {
    pub heading: &'static str,
    pub items: &'static [Item],
}

const HOME: &[Section] = &[
    Section {
        heading: "Font",
        items: &[
            fmt("home.bold", Icon::Bold),
            fmt("home.italic", Icon::Italic),
            fmt("home.underline", Icon::Underline),
            fmt("home.strikethrough", Icon::Strike),
            page("home.borders", SubPage::Borders, Icon::Borders, Some("Borders…")),
            page("home.fillColor", SubPage::Fill, Icon::Fill, Some("Fill Color…")),
            page("home.fontColor", SubPage::Font, Icon::FontColor, Some("Font Color…")),
        ],
    },
    Section {
        heading: "Alignment",
        items: &[
            fmt("home.alignLeft", Icon::AlignLeft),
            fmt("home.alignCenter", Icon::AlignCenter),
            fmt("home.alignRight", Icon::AlignRight),
            fmt("home.alignTop", Icon::AlignTop),
            fmt("home.alignMiddle", Icon::AlignMiddle),
            fmt("home.alignBottom", Icon::AlignBottom),
            fmt("home.wrapText", Icon::Wrap),
            fmt("home.mergeCenter", Icon::Merge),
        ],
    },
    Section {
        heading: "Number",
        items: &[
            page("home.numberFormat", SubPage::NumberFormat, Icon::Currency, Some("Number Format…")),
            fmt("home.percent", Icon::Percent),
            fmt("home.increaseDecimal", Icon::DecInc),
            fmt("home.decreaseDecimal", Icon::DecDec),
        ],
    },
    Section {
        heading: "Clipboard",
        items: &[
            Item { cmd: "edit.cut", icon: Icon::Cut, act: Act::Copy("edit.cut"), keep: false, label: None },
            Item { cmd: "edit.copy", icon: Icon::Copy, act: Act::Copy("edit.copy"), keep: false, label: None },
            run("edit.paste", Icon::Paste),
            run("edit.pasteSpecial", Icon::Paste),
        ],
    },
    Section {
        heading: "Cells",
        items: &[
            run("home.insertRows", Icon::Insert),
            run("home.insertColumns", Icon::Insert),
            run("home.deleteRows", Icon::Delete),
            run("home.deleteColumns", Icon::Delete),
            run("home.hideRows", Icon::Collapse),
            run("home.hideColumns", Icon::Collapse),
            run("home.unhideRows", Icon::Expand),
            run("home.unhideColumns", Icon::Expand),
            run("home.rowHeight", Icon::Format),
            run("home.columnWidth", Icon::Format),
            run("home.autofitColumnWidth", Icon::Format),
            dialog("home.formatCells", "formatCells", Icon::Format),
        ],
    },
    Section {
        heading: "Styles",
        items: &[
            run("home.conditionalFormat", Icon::CondFormat),
            run("home.manageRules", Icon::CondFormat),
            run("home.formatAsTable", Icon::FormatTable),
        ],
    },
    Section {
        heading: "Editing",
        items: &[
            run("edit.fillDown", Icon::FillDown),
            run("edit.fillRight", Icon::FillDown),
            run("edit.clearContents", Icon::Eraser),
            run("edit.clearFormats", Icon::Eraser),
            run("edit.find", Icon::Find),
            run("edit.replace", Icon::Find),
        ],
    },
];

const INSERT: &[Section] = &[
    Section { heading: "Tables", items: &[run("insert.table", Icon::Table), run("insert.pivotTable", Icon::PivotTable)] },
    Section {
        heading: "Charts",
        items: &[run("insert.chart", Icon::Chart), run("insert.recommendedCharts", Icon::ChartBar), run("insert.sparkline", Icon::Sparkline)],
    },
    Section {
        heading: "Objects",
        items: &[
            run("insert.picture", Icon::Picture),
            run("insert.link", Icon::Link),
            Item { cmd: "review.newComment", icon: Icon::Comment, act: Act::Comment, keep: false, label: None },
            run("insert.equation", Icon::Equation),
        ],
    },
];

const FORMULAS: &[Section] = &[
    Section { heading: "Functions", items: &[run("formulas.autoSum", Icon::Sum), dialog("formulas.insertFunction", "insertFunction", Icon::Fx)] },
    Section { heading: "Names", items: &[run("formulas.nameManager", Icon::Name)] },
    Section {
        heading: "Auditing",
        items: &[
            fmt("formulas.showFormulas", Icon::Function),
            run("formulas.errorChecking", Icon::Trace),
            run("formulas.evaluateFormula", Icon::Calc),
            fmt("formulas.calculateNow", Icon::Calc),
        ],
    },
];

const DATA: &[Section] = &[
    Section {
        heading: "Sort and Filter",
        items: &[
            fmt("data.sortAscending", Icon::SortAsc),
            fmt("data.sortDescending", Icon::SortDesc),
            run("data.sort", Icon::SortFilter),
            fmt("data.filter", Icon::Filter),
            fmt("data.clearFilter", Icon::Eraser),
        ],
    },
    Section {
        heading: "Data Tools",
        items: &[
            run("data.removeDuplicates", Icon::Duplicates),
            run("data.textToColumns", Icon::TextColumns),
            run("data.validation", Icon::Validation),
        ],
    },
    Section {
        heading: "Outline",
        items: &[fmt("data.group", Icon::Group), fmt("data.ungroup", Icon::Ungroup), run("data.subtotal", Icon::Subtotal)],
    },
];

const VIEW: &[Section] = &[
    Section {
        heading: "Show",
        items: &[fmt("view.gridlines", Icon::Gridlines), fmt("view.headings", Icon::Header), fmt("view.formulaBar", Icon::Fx)],
    },
    Section {
        heading: "Zoom",
        items: &[
            Item { cmd: "view.zoom", icon: Icon::Plus, act: Act::Zoom(10), keep: true, label: Some("Zoom In") },
            Item { cmd: "view.zoom", icon: Icon::Minus, act: Act::Zoom(-10), keep: true, label: Some("Zoom Out") },
            fmt("view.zoom100", Icon::Zoom),
        ],
    },
    Section { heading: "Window", items: &[fmt("view.freezePanes", Icon::Freeze)] },
];

const PAGE_LAYOUT: &[Section] = &[Section {
    heading: "Page Setup",
    items: &[
        run("pageLayout.pageSetup", Icon::Margins),
        run("pageLayout.orientation", Icon::Orientation),
        run("pageLayout.printArea", Icon::PrintArea),
    ],
}];

const REVIEW: &[Section] = &[
    Section { heading: "Proofing", items: &[run("review.spelling", Icon::Spell)] },
    Section { heading: "Comments", items: &[fmt("review.showComments", Icon::Comment)] },
    Section { heading: "Protect", items: &[run("review.protectSheet", Icon::Lock), run("review.protectWorkbook", Icon::Lock)] },
];

/// The sections of command tab `tab` (an index into `super::TABS`).
pub(super) fn sections(tab: usize) -> &'static [Section] {
    match tab {
        0 => HOME,
        1 => INSERT,
        2 => FORMULAS,
        3 => DATA,
        4 => VIEW,
        5 => PAGE_LAYOUT,
        6 => REVIEW,
        _ => &[],
    }
}

/// The label a tile shows, before translation: its own label, else the engine command's.
pub(super) fn label(item: &Item) -> &'static str {
    item.label.or_else(|| cmd::find_command(item.cmd).map(|s| s.label)).unwrap_or(item.cmd)
}

/// Whether the tile's command can run now (the engine's `enabled`). A sub-page with no command of
/// its own is always enabled.
pub(super) fn enabled(app: &SheetApp, item: &Item) -> bool {
    cmd::find_command(item.cmd).is_none_or(|s| (s.enabled)(&app.session).is_ok())
}

/// The active cell's style.
pub(super) fn active_style(app: &SheetApp) -> Style {
    app.session
        .active()
        .and_then(|d| {
            let sh = d.wb.active()?;
            Some(d.wb.styles.get(sh.style_id(d.selection.active)).clone())
        })
        .unwrap_or_default()
}

/// Whether a toggle tile is on: the active cell's format, or the sheet's view setting.
pub(super) fn is_on(app: &SheetApp, st: &Style, cmd: &str) -> bool {
    let sheet = |f: fn(&gridcraft_engine::model::Sheet) -> bool| app.session.active().and_then(|d| d.wb.active().map(f)).unwrap_or(false);
    match cmd {
        "home.bold" => st.font.bold,
        "home.italic" => st.font.italic,
        "home.underline" => st.font.underline != Underline::None,
        "home.strikethrough" => st.font.strike,
        "home.alignLeft" => st.align.h == HAlign::Left,
        "home.alignCenter" => st.align.h == HAlign::Center,
        "home.alignRight" => st.align.h == HAlign::Right,
        "home.alignTop" => st.align.v == VAlign::Top,
        "home.alignMiddle" => st.align.v == VAlign::Center,
        "home.alignBottom" => st.align.v == VAlign::Bottom,
        "home.wrapText" => st.align.wrap,
        "view.gridlines" => sheet(|s| s.show_gridlines),
        "view.headings" => sheet(|s| s.show_headings),
        "formulas.showFormulas" => sheet(|s| s.show_formulas),
        "view.formulaBar" => app.session.prefs.formula_bar,
        _ => false,
    }
}

/// The body of a tab's sheet: Home's font row, then each section's heading and tiles.
pub(super) fn body(app: &mut SheetApp, ui: &mut Ui, tab: usize) {
    let ctx = ui.ctx().clone();
    let t = Tokens::get(&ctx);
    let st = active_style(app);
    if tab == 0 {
        font_row(app, ui, &st);
    }
    for section in sections(tab) {
        heading(ui, t, app.ui.language.tr(section.heading));
        let width = ui.available_width();
        let cols = ((width / TILE_MIN_W).floor() as usize).max(1);
        let w = width / cols as f32;
        for row in section.items.chunks(cols) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for item in row {
                    let on = is_on(app, &st, item.cmd);
                    let enabled = enabled(app, item);
                    let text = app.ui.language.tr(label(item)).to_string();
                    if tile(ui, t, w, item.icon, &text, on, enabled).clicked() {
                        tap(app, &ctx, item);
                    }
                }
            });
        }
    }
    ui.add_space(GUTTER);
}

/// Runs a tile: opens its sub-page, or runs its command and closes the sheet unless it keeps it.
pub(super) fn tap(app: &mut SheetApp, ctx: &Context, item: &Item) {
    match item.act {
        Act::Page(p) => set_palette(ctx, Some(p)),
        // A command that needs values: a picker, its dialog, or a form, as All tools does; Back
        // returns to this tab.
        Act::Run(id) if cmd::find_command(id).is_some_and(|s| !super::tools::required(s.params).is_empty()) => {
            if let Some(spec) = cmd::find_command(id) {
                let tab = super::menu_tab(ctx);
                super::tools::start(app, ctx, spec, tab);
            }
        }
        other => {
            run_act(app, ctx, other);
            if !item.keep {
                close_all(ctx);
            }
        }
    }
}

/// A section heading: small, in the accent colour.
pub(super) fn heading(ui: &mut Ui, t: Tokens, text: &str) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::hover());
    ui.painter().text(Pos2::new(r.left(), r.bottom() - 8.0), Align2::LEFT_BOTTOM, text, theme::ui_bold(12.0), t.accent);
}

/// Home's font row: the font name (opens the Font list), then − size + (the size opens the Size list).
fn font_row(app: &mut SheetApp, ui: &mut Ui, st: &Style) {
    let ctx = ui.ctx().clone();
    let t = Tokens::get(&ctx);
    let lang = app.ui.language;
    let size = st.font.size;
    let size_text = if size.fract() == 0.0 { format!("{}", size as i32) } else { format!("{size}") };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let name_w = (ui.available_width() - 3.0 * TOUCH - 3.0 * 4.0).max(TOUCH);
        let (r, resp) = ui.allocate_exact_size(vec2(name_w, TOUCH), Sense::click());
        let fill = if resp.is_pointer_button_down_on() { t.pressed } else { t.input_bg };
        ui.painter().rect(r.shrink2(vec2(0.0, 2.0)), CornerRadius::same(8), fill, egui::Stroke::new(1.0, t.input_border), egui::StrokeKind::Inside);
        super::paint_truncated(ui.painter(), r.left() + 12.0, r.center().y, (r.width() - 44.0).max(0.0), &st.font.name, theme::ui_font(15.0), t.text);
        icons::paint(ui.painter(), Rect::from_center_size(Pos2::new(r.right() - 20.0, r.center().y), vec2(16.0, 16.0)), Icon::Chevron, t.text_dim);
        let font_label = format!("{}: {}", lang.tr("Font"), st.font.name);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, font_label.clone()));
        if resp.clicked() {
            set_palette(&ctx, Some(SubPage::FontName));
        }
        if text_button(ui, t, "−", lang.tr("Decrease Font Size")).clicked() {
            app.run_or_alert("home.decreaseFontSize", json!({}));
        }
        let size_label = format!("{}: {size_text}", lang.tr("Font Size"));
        if text_button(ui, t, &size_text, &size_label).clicked() {
            set_palette(&ctx, Some(SubPage::FontSize));
        }
        if text_button(ui, t, "+", lang.tr("Increase Font Size")).clicked() {
            app.run_or_alert("home.increaseFontSize", json!({}));
        }
    });
}

/// A 48 pt button showing `text` (the size stepper's − and +, and the size), named `name`.
fn text_button(ui: &mut Ui, t: Tokens, text: &str, name: &str) -> Response {
    let (r, resp) = ui.allocate_exact_size(vec2(TOUCH, TOUCH), Sense::click());
    if resp.is_pointer_button_down_on() {
        ui.painter().rect_filled(r, TOUCH / 2.0, t.pressed);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, TOUCH / 2.0, t.hover);
    }
    let big = text == "−" || text == "+";
    let (font, color) = if big { (theme::ui_font(24.0), t.accent) } else { (theme::ui_font(16.0), t.text) };
    ui.painter().text(r.center(), Align2::CENTER_CENTER, text, font, color);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
    resp
}

/// One tile, `w` wide: the icon, then the label on up to two lines, broken only at spaces. A single
/// word too long for one line is drawn at 11 pt, or smaller down to 9 pt when it must be; text past
/// the second line ends in an ellipsis. An
/// "on" tile has a tinted background and accent ink; a disabled one is greyed and inert.
pub(super) fn tile(ui: &mut Ui, t: Tokens, w: f32, icon: Icon, text: &str, on: bool, enabled: bool) -> Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (r, resp) = ui.allocate_exact_size(vec2(w, TILE_H), sense);
    let painter = ui.painter();
    let body = r.shrink(2.0);
    if on {
        painter.rect_filled(body, CornerRadius::same(12), t.accent_soft);
    }
    if enabled && resp.is_pointer_button_down_on() {
        painter.rect_filled(body, CornerRadius::same(12), t.pressed);
    } else if enabled && resp.hovered() {
        painter.rect_filled(body, CornerRadius::same(12), t.hover);
    }
    let ink = if !enabled {
        t.text_disabled
    } else if on {
        t.accent
    } else {
        t.text
    };
    icons::paint(painter, Rect::from_center_size(Pos2::new(r.center().x, r.top() + 22.0), vec2(24.0, 24.0)), icon, ink);
    let max_w = (w - 4.0).max(1.0);
    // The widest word at 12 pt: a word wider than the tile is drawn smaller (11 pt, or less down to
    // 9 pt when it must), so lines break only at spaces.
    let widest = text.split(' ').map(|word| painter.layout_no_wrap(word.to_owned(), theme::ui_font(12.0), ink).size().x).fold(0.0, f32::max);
    let size = if widest <= max_w { 12.0 } else { (12.0 * max_w / widest).floor().clamp(9.0, 11.0) };
    let mut job = egui::text::LayoutJob::simple(text.to_owned(), theme::ui_font(size), ink, max_w);
    job.wrap.max_rows = 2;
    job.wrap.break_anywhere = false;
    job.wrap.overflow_character = Some('…');
    job.halign = egui::Align::Center;
    let galley = painter.layout_job(job);
    painter.galley(Pos2::new(r.center().x, r.top() + 40.0), galley, ink);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, enabled, on, text));
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tile_names_a_known_command() {
        for tab in 0..super::super::TABS.len() {
            assert!(!sections(tab).is_empty(), "tab {tab} has sections");
            for section in sections(tab) {
                assert!(!section.items.is_empty(), "{} has tiles", section.heading);
                for item in section.items {
                    assert!(cmd::find_command(item.cmd).is_some(), "{} names no engine command", item.cmd);
                    if std::env::var_os("PRINT_PHONE_LABELS").is_some() {
                        println!("{tab}\t{}\t{}\t{}", section.heading, item.cmd, label(item));
                    }
                }
            }
        }
    }

    /// Every tile whose command needs values opens its dialog, or a picker or form in the tools, and
    /// never ends in an error message.
    #[test]
    fn every_tile_that_needs_values_opens_a_dialog_or_a_picker() {
        use egui_kittest::kittest::Queryable;
        for (tab, (tab_name, _)) in super::super::TABS.iter().enumerate() {
            for section in sections(tab) {
                for item in section.items {
                    let Act::Run(id) = item.act else { continue };
                    let Some(spec) = cmd::find_command(id) else { continue };
                    if super::super::tools::required(spec.params).is_empty() {
                        continue;
                    }
                    let mut s = gridcraft_engine::Session::new();
                    s.new_workbook();
                    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(412.0, 915.0)).build_ui_state(
                        |ui, app: &mut SheetApp| {
                            let ctx = ui.ctx().clone();
                            app.logic(&ctx);
                            app.ui(ui);
                        },
                        SheetApp::new(s, Default::default()),
                    );
                    h.run_steps(4);
                    super::super::open_by_name(&h.ctx, tab_name);
                    h.run_steps(4);
                    if !enabled(h.state(), item) {
                        continue;
                    }
                    // A tile low in the sheet is scrolled into view first, as a finger would.
                    let tab_bar_top = h.ctx.content_rect().bottom() - super::super::TAB_BAR_H;
                    for _ in 0..40 {
                        if h.get_by_label(label(item)).rect().bottom() <= tab_bar_top {
                            break;
                        }
                        h.input_mut().events.push(egui::Event::PointerMoved(egui::pos2(206.0, tab_bar_top - 40.0)));
                        h.input_mut().events.push(egui::Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: egui::vec2(0.0, -60.0),
                            phase: egui::TouchPhase::Move,
                            modifiers: egui::Modifiers::NONE,
                            source: egui::MouseWheelSource::Unknown,
                        });
                        h.run_steps(2);
                    }
                    h.get_by_label(label(item)).click();
                    h.run_steps(4);
                    let app = h.state();
                    assert!(app.message.is_none(), "{id} ended in an error: {:?}", app.message);
                    let in_tools = super::super::menu_tab(&h.ctx) == Some(super::super::TOOLS);
                    assert!(app.dialog.is_some() || in_tools, "{id} opened neither a dialog nor the tools");
                }
            }
        }
    }

    #[test]
    fn labels_in_a_section_are_distinct() {
        for tab in 0..super::super::TABS.len() {
            for section in sections(tab) {
                let mut seen = std::collections::HashSet::new();
                for item in section.items {
                    assert!(seen.insert(label(item)), "{} repeats {}", section.heading, label(item));
                }
            }
        }
    }
}
