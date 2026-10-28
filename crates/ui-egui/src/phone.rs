//! The phone layout, after WordCraft's (Material 3): a narrow window (a phone held upright) gets a
//! top app bar, a floating tab bar and bottom sheets in place of the ribbon and title bar. The
//! grid keeps its canvas. Every control runs a command id, so the phone chrome adds no behaviour
//! of its own.

use egui::{Align2, Context, CornerRadius, Frame, Layout, Margin, Panel, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, vec2};
use gridcraft_engine::cmd;
use serde_json::json;

use crate::SheetApp;
use crate::backstage::Page;
use crate::icons::{self, Icon};
use crate::ribbon;
use crate::theme::{self, Tokens};
use crate::widgets::color_palette;

/// Windows narrower than this (points) get the phone layout. A phone held upright is about 390 pt.
pub const MAX_WIDTH: f32 = 600.0;
/// Height of the top app bar (Material's small app bar).
const APP_BAR_H: f32 = 64.0;
/// A Material icon button: 48 pt, the platform's touch target.
const ICON_BTN: f32 = 48.0;
/// A sheet row: the 48 pt touch target.
const ROW_H: f32 = 48.0;
/// The shortest bar under the grid (formula bar, sheet tabs) on the phone: a 44 pt touch target.
pub const BAR_MIN_H: f32 = 44.0;
/// The floating tab bar: its height, the gap around it, and its inset from the screen's sides.
const PILL_H: f32 = 56.0;
const FLOAT_GAP: f32 = 8.0;
const PILL_SIDE: f32 = 16.0;

/// Whether the window is a phone: its shorter side is under the breakpoint, so a phone turned
/// sideways keeps the phone layout.
pub fn active(ctx: &Context) -> bool {
    let size = ctx.content_rect().size();
    size.x.min(size.y) < MAX_WIDTH
}

/// A bar under the grid's top or bottom edge: `desktop` high on desktop, at least a touch target
/// on the phone.
pub fn bar_height(ctx: &Context, desktop: f32) -> f32 {
    if active(ctx) { desktop.max(BAR_MIN_H) } else { desktop }
}

/// The tabs of the floating bar, with their icons. Each opens a bottom sheet of its rows.
const TABS: [(&str, Icon); 5] = [("Home", Icon::Home), ("Insert", Icon::Insert), ("Formulas", Icon::Fx), ("Data", Icon::SortFilter), ("View", Icon::Gridlines)];

/// Which colour palette a sheet row opens inline.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Palette {
    Fill,
    Font,
}

/// What a sheet row does.
#[derive(Clone, Copy)]
enum Act {
    /// A command with no parameters.
    Run(&'static str),
    /// A command that copies to the clipboard as the ribbon's Cut and Copy do.
    Copy(&'static str),
    /// A dialog, with its page named by the optional tab.
    Dialog(&'static str, Option<&'static str>),
    /// The colour palette, shown inline in the sheet until a colour is picked.
    Colour(Palette),
    /// Zoom in or out by this many percent.
    Zoom(i32),
}

/// The rows of a tab: label, icon, action.
fn rows(tab: usize) -> &'static [(&'static str, Icon, Act)] {
    match tab {
        0 => &[
            ("Bold", Icon::Bold, Act::Run("home.bold")),
            ("Italic", Icon::Italic, Act::Run("home.italic")),
            ("Underline", Icon::Underline, Act::Run("home.underline")),
            ("Fill Colour", Icon::Fill, Act::Colour(Palette::Fill)),
            ("Font Colour", Icon::FontColor, Act::Colour(Palette::Font)),
            ("Align Left", Icon::AlignLeft, Act::Run("home.alignLeft")),
            ("Center", Icon::AlignCenter, Act::Run("home.alignCenter")),
            ("Align Right", Icon::AlignRight, Act::Run("home.alignRight")),
            ("Number Format", Icon::Currency, Act::Dialog("formatCells", Some("Number"))),
            ("Cut", Icon::Cut, Act::Copy("edit.cut")),
            ("Copy", Icon::Copy, Act::Copy("edit.copy")),
            ("Paste", Icon::Paste, Act::Run("edit.paste")),
            ("Insert Row", Icon::Insert, Act::Run("home.insertRows")),
            ("Insert Column", Icon::Insert, Act::Run("home.insertColumns")),
            ("Delete Row", Icon::Delete, Act::Run("home.deleteRows")),
            ("Delete Column", Icon::Delete, Act::Run("home.deleteColumns")),
        ],
        1 => &[
            ("Chart", Icon::Chart, Act::Run("insert.chart")),
            ("Picture", Icon::Picture, Act::Run("insert.picture")),
            ("Function", Icon::Fx, Act::Dialog("insertFunction", None)),
            ("Comment", Icon::Comment, Act::Run("review.newComment")),
        ],
        2 => &[("AutoSum", Icon::Sum, Act::Run("formulas.autoSum")), ("Insert Function", Icon::Fx, Act::Dialog("insertFunction", None))],
        3 => &[
            ("Sort A to Z", Icon::SortAsc, Act::Run("data.sortAscending")),
            ("Sort Z to A", Icon::SortDesc, Act::Run("data.sortDescending")),
            ("Filter", Icon::Filter, Act::Run("data.filter")),
        ],
        4 => &[
            ("Freeze Panes", Icon::Freeze, Act::Run("view.freezePanes")),
            ("Gridlines", Icon::Gridlines, Act::Run("view.gridlines")),
            ("Zoom In", Icon::Plus, Act::Zoom(10)),
            ("Zoom Out", Icon::Minus, Act::Zoom(-10)),
        ],
        _ => &[],
    }
}

/// The open sheet's tab, or `None` while no sheet is open. Kept in egui's memory, so it lasts the
/// session without a field on `SheetApp`.
fn sheet_id() -> egui::Id {
    egui::Id::new("phone_sheet_tab")
}

fn menu_tab(ctx: &Context) -> Option<usize> {
    ctx.data(|d| d.get_temp::<Option<usize>>(sheet_id())).flatten()
}

fn set_menu(ctx: &Context, tab: Option<usize>) {
    ctx.data_mut(|d| d.insert_temp(sheet_id(), tab));
}

/// The colour palette open in the sheet, if any.
fn palette_id() -> egui::Id {
    egui::Id::new("phone_sheet_palette")
}

fn open_palette(ctx: &Context) -> Option<Palette> {
    ctx.data(|d| d.get_temp::<Option<Palette>>(palette_id())).flatten()
}

fn set_palette(ctx: &Context, p: Option<Palette>) {
    ctx.data_mut(|d| d.insert_temp(palette_id(), p));
}

/// Where the open sheet was drawn last frame. A press outside it closes the sheet.
fn sheet_rect_id() -> egui::Id {
    egui::Id::new("phone_sheet_rect")
}

/// The top app bar: File (the Backstage), the workbook's name, and Undo, Redo and Save.
pub fn top_bar(app: &mut SheetApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let t = Tokens::get(&ctx);
    let lang = app.ui.language;
    let (title, dirty) = app.session.active().map(|d| (d.display_title(), d.is_dirty())).unwrap_or_default();
    let can_undo = cmd::can_undo(&app.session).is_ok();
    let can_redo = cmd::can_redo(&app.session).is_ok();
    Panel::top("phone_top_bar").exact_size(APP_BAR_H).frame(Frame::NONE.fill(t.window).inner_margin(Margin::symmetric(4, 0))).show(ui, |ui| {
        ui.horizontal_centered(|ui| {
            if icon_button(ui, t, Icon::Folder, lang.tr("File"), true).clicked() {
                app.open_backstage(Page::Home);
            }
            // The name gets the width the three icons on the right leave, and is cut short.
            let name = if dirty { format!("{title} •") } else { title };
            let name_w = (ui.available_width() - 3.0 * ICON_BTN - 8.0).max(0.0);
            ui.add_sized(
                vec2(name_w, ICON_BTN),
                egui::Label::new(egui::RichText::new(name).font(theme::ui_bold(18.0)).color(t.text)).truncate(),
            );
            // Right to left: Save is added first so it sits at the far end.
            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                if icon_button(ui, t, Icon::Save, lang.tr("Save"), true).clicked() && app.commit_edit(0, 0, false, false) {
                    app.run_or_alert("file.save", json!({}));
                }
                if icon_button(ui, t, Icon::Redo, lang.tr("Redo"), can_redo).clicked() && can_redo {
                    app.run_or_alert("edit.redo", json!({}));
                }
                if icon_button(ui, t, Icon::Undo, lang.tr("Undo"), can_undo).clicked() && can_undo {
                    app.run_or_alert("edit.undo", json!({}));
                }
            });
        });
    });
}

/// The floating tab bar at the bottom, and the bottom sheet that a tab opens in its place. The bar
/// is a bottom panel, so the grid stops above it and nothing is drawn under it.
pub fn bottom_bar(app: &mut SheetApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let t = Tokens::get(&ctx);
    let lang = app.ui.language;
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        set_menu(&ctx, None);
    }
    // A press outside the open sheet closes it. The press still reaches the grid, as a cell tap.
    if menu_tab(&ctx).is_some() && ctx.input(|i| i.pointer.primary_pressed()) {
        let pos = ctx.input(|i| i.pointer.latest_pos());
        let rect = ctx.data(|d| d.get_temp::<Rect>(sheet_rect_id()));
        if matches!((pos, rect), (Some(p), Some(r)) if !r.contains(p)) {
            set_menu(&ctx, None);
            set_palette(&ctx, None);
        }
    }
    if let Some(tab) = menu_tab(&ctx) {
        return sheet(app, ui, tab);
    }
    Panel::bottom("phone_tab_bar").exact_size(PILL_H + 2.0 * FLOAT_GAP).frame(Frame::NONE).show(ui, |ui| {
        let outer = ui.max_rect();
        let pill = Rect::from_min_max(
            Pos2::new(outer.left() + PILL_SIDE, outer.top() + FLOAT_GAP),
            Pos2::new(outer.right() - PILL_SIDE, outer.bottom() - FLOAT_GAP),
        );
        ui.painter().rect(pill, CornerRadius::same((PILL_H / 2.0) as u8), t.ribbon, Stroke::new(1.0, t.ribbon_border), StrokeKind::Inside);
        let w = pill.width() / TABS.len() as f32;
        for (i, (label, icon)) in TABS.iter().enumerate() {
            let cell = Rect::from_min_size(Pos2::new(pill.left() + i as f32 * w, pill.top()), vec2(w, pill.height()));
            let resp = ui.interact(cell, ui.id().with(("phone_tab", i)), Sense::click());
            // The indicator is the 56 x 28 pill behind the icon, as Material draws an active tab.
            let indicator = Rect::from_center_size(Pos2::new(cell.center().x, pill.top() + 18.0), vec2(56.0, 28.0));
            let painter = ui.painter();
            if resp.is_pointer_button_down_on() {
                painter.rect_filled(indicator, 14.0, t.pressed);
            } else if resp.hovered() {
                painter.rect_filled(indicator, 14.0, t.hover);
            }
            painter.text(
                Pos2::new(cell.center().x, pill.top() + 44.0),
                Align2::CENTER_CENTER,
                lang.tr(label),
                theme::ui_font(11.0),
                t.text_dim,
            );
            icons::paint(painter, Rect::from_center_size(indicator.center(), vec2(22.0, 22.0)), *icon, t.text);
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, lang.tr(label)));
            if resp.clicked() {
                set_menu(&ctx, Some(i));
            }
        }
    });
}

/// The bottom sheet of a tab: a title row with Close, the colour palette when a colour row is open,
/// then the tab's rows as 48 pt touch targets.
fn sheet(app: &mut SheetApp, ui: &mut Ui, tab: usize) {
    let ctx = ui.ctx().clone();
    let t = Tokens::get(&ctx);
    let lang = app.ui.language;
    let screen_h = ctx.content_rect().height();
    let height = (screen_h * 0.55).clamp(260.0, 520.0).min(screen_h * 0.85);
    let name = TABS.get(tab).map_or("", |tb| tb.0);
    let panel = Panel::bottom("phone_sheet")
        .exact_size(height)
        .frame(
            Frame::NONE
                .fill(t.menu_bg)
                .corner_radius(CornerRadius { nw: 16, ne: 16, sw: 0, se: 0 })
                .inner_margin(Margin::symmetric(16, 0)),
        )
        .show(ui, |ui| {
            ui.add_space(8.0);
            let (handle, _) = ui.allocate_exact_size(vec2(32.0, 4.0), Sense::hover());
            ui.painter().rect_filled(handle, CornerRadius::same(2), t.separator);
            ui.horizontal(|ui| {
                ui.add_sized(
                    vec2((ui.available_width() - ICON_BTN).max(0.0), ICON_BTN),
                    egui::Label::new(egui::RichText::new(lang.tr(name)).font(theme::ui_bold(18.0)).color(t.text)),
                );
                if icon_button(ui, t, Icon::Close, lang.tr("Close"), true).clicked() {
                    set_menu(&ctx, None);
                    set_palette(&ctx, None);
                }
            });
            egui::ScrollArea::vertical().show(ui, |ui| {
                if let Some(p) = open_palette(&ctx) {
                    let colors = ribbon::theme_colors(app);
                    let (none, cmd_id) = match p {
                        Palette::Fill => (lang.tr("No Fill"), "home.fillColor"),
                        Palette::Font => (lang.tr("Automatic"), "home.fontColor"),
                    };
                    if let Some(c) = color_palette(ui, &colors, none) {
                        // The font colour's "no colour" is Automatic, as the ribbon's palette sends it.
                        let color = if p == Palette::Font && c == "none" { "auto".to_string() } else { c };
                        app.run_or_alert(cmd_id, json!({"color": color}));
                        set_menu(&ctx, None);
                        set_palette(&ctx, None);
                    }
                    ui.add_space(4.0);
                }
                for (label, icon, act) in rows(tab) {
                    if list_row(ui, t, lang.tr(label), *icon).clicked() {
                        match *act {
                            Act::Colour(p) => set_palette(&ctx, if open_palette(&ctx) == Some(p) { None } else { Some(p) }),
                            other => {
                                run_act(app, &ctx, other);
                                set_menu(&ctx, None);
                                set_palette(&ctx, None);
                            }
                        }
                    }
                }
            });
        });
    ctx.data_mut(|d| d.insert_temp(sheet_rect_id(), panel.response.rect));
}

/// Runs a sheet row's action. Colour rows are handled by the sheet itself.
fn run_act(app: &mut SheetApp, ctx: &Context, act: Act) {
    match act {
        Act::Run(id) => app.run_or_alert(id, json!({})),
        Act::Copy(id) => app.copy_to_clipboard(ctx, id),
        Act::Dialog(name, tab) => app.open_dialog(name, tab.map_or_else(|| json!({}), |page| json!({"tab": page}))),
        Act::Zoom(step) => {
            let zoom = app.session.active().and_then(|d| d.wb.active().map(|s| s.zoom)).unwrap_or(100);
            let percent = (i32::from(zoom) + step).clamp(10, 400);
            app.run_or_alert("view.zoom", json!({"percent": percent}));
        }
        Act::Colour(_) => {}
    }
}

/// An icon button of the app bar and the sheets: 48 pt, the icon in the middle, its name as the
/// tooltip and accessible name. A disabled one is drawn grey and does nothing.
fn icon_button(ui: &mut Ui, t: Tokens, icon: Icon, tip: &str, enabled: bool) -> Response {
    let (r, resp) = ui.allocate_exact_size(vec2(ICON_BTN, ICON_BTN), Sense::click());
    if enabled && resp.is_pointer_button_down_on() {
        ui.painter().rect_filled(r, ICON_BTN / 2.0, t.pressed);
    } else if enabled && resp.hovered() {
        ui.painter().rect_filled(r, ICON_BTN / 2.0, t.hover);
    }
    let color = if enabled { t.text } else { t.text_disabled };
    icons::paint(ui.painter(), Rect::from_center_size(r.center(), vec2(24.0, 24.0)), icon, color);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, tip));
    resp.on_hover_text(tip)
}

/// A sheet row: the icon, then the label, in a 48 pt touch target across the sheet.
fn list_row(ui: &mut Ui, t: Tokens, label: &str, icon: Icon) -> Response {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    if resp.is_pointer_button_down_on() {
        ui.painter().rect_filled(r, 0.0, t.pressed);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, 0.0, t.hover);
    }
    let painter = ui.painter();
    icons::paint(painter, Rect::from_center_size(Pos2::new(r.left() + 24.0, r.center().y), vec2(24.0, 24.0)), icon, t.text);
    painter.text(Pos2::new(r.left() + 60.0, r.center().y), Align2::LEFT_CENTER, label, theme::ui_font(15.0), t.text);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    resp
}
