//! The phone layout, after WordCraft's (Material 3): a narrow window (a phone held upright) gets a
//! top app bar, a floating bar at the bottom and bottom sheets in place of the ribbon, title bar and
//! sheet tabs. The bar's first item is the active sheet: tapping it opens the Sheets sheet, to switch
//! sheets, add one, or rename, duplicate or delete one. The grid keeps its canvas. Every control runs
//! a command id, so the phone chrome adds no behaviour of its own.

use egui::{Align2, Color32, Context, CornerRadius, FontId, Frame, Layout, Margin, Painter, Panel, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, vec2};
use gridcraft_engine::cmd;
use gridcraft_engine::core::{CellRef, RangeRef};
use gridcraft_engine::model::{Sheet, Visibility};
use serde_json::json;

use crate::SheetApp;
use crate::backstage::Page;
use crate::grid::{self, Geo};
use crate::icons::{self, Icon};
use crate::ribbon;
use crate::tabs;
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
/// The shortest bar under the grid (formula bar) on the phone: a 44 pt touch target.
pub const BAR_MIN_H: f32 = 44.0;
/// The floating bar: its height, the gap around it, and its inset from the screen's sides.
const PILL_H: f32 = 56.0;
const FLOAT_GAP: f32 = 8.0;
const PILL_SIDE: f32 = 16.0;
/// The sheet button at the bar's left (48 pt tall, this wide), and the room the bar takes beside the
/// command tabs: the inset, the button, and the gaps either side of the divider.
const SHEET_BTN_W: f32 = 96.0;
const PILL_SPARE: f32 = 4.0 + SHEET_BTN_W + 4.0 + 4.0 + 8.0;
/// The least width a command tab gets. When the five do not fit, the last one moves into More.
const MIN_TAB_W: f32 = 52.0;

/// The bar's command tabs, with their icons. Each opens a bottom sheet of its rows.
const TABS: [(&str, Icon); 5] = [("Home", Icon::Home), ("Insert", Icon::Insert), ("Formulas", Icon::Fx), ("Data", Icon::SortFilter), ("View", Icon::Gridlines)];
/// On a bar too narrow for every tab, the tabs from this index on are listed in More.
const NARROW_TABS: usize = TABS.len() - 1;
/// Menu states past the command tabs: the More list and the Sheets sheet.
const MORE: usize = TABS.len();
const SHEETS: usize = TABS.len() + 1;

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

/// The open sheet (a tab, More or Sheets), or `None` while no sheet is open. Kept in egui's memory,
/// so it lasts the session without a field on `SheetApp`.
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

/// The sheet (by index) whose row actions are open in the Sheets sheet.
fn actions_id() -> egui::Id {
    egui::Id::new("phone_sheet_row_actions")
}

fn row_actions(ctx: &Context) -> Option<usize> {
    ctx.data(|d| d.get_temp::<Option<usize>>(actions_id())).flatten()
}

fn set_row_actions(ctx: &Context, sheet: Option<usize>) {
    ctx.data_mut(|d| d.insert_temp(actions_id(), sheet));
}

/// Closes the open sheet, with its palette and row actions.
fn close_all(ctx: &Context) {
    set_menu(ctx, None);
    set_palette(ctx, None);
    set_row_actions(ctx, None);
    set_bar_open(ctx, false);
}

/// Where the open sheet was drawn last frame. A press outside it closes the sheet.
fn sheet_rect_id() -> egui::Id {
    egui::Id::new("phone_sheet_rect")
}

/// The gesture the grid must ignore: a press that began outside the open sheet. Set while the
/// button is held, so the press closes the sheet and reaches neither the grid nor the cells.
fn gesture_id() -> egui::Id {
    egui::Id::new("phone_gesture_taken")
}

/// Whether the grid ignores the pointer this frame (see [`take_pointer`]).
pub fn pointer_taken(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(gesture_id())).unwrap_or(false)
}

/// Runs once a frame, before the grid. A press outside the open sheet closes it and is taken: the
/// grid gets none of the gesture until the button is released. A press inside the sheet is the
/// sheet's own.
pub fn take_pointer(ctx: &Context) {
    let (pressed, pos) = ctx.input(|i| (i.pointer.primary_pressed(), i.pointer.latest_pos()));
    // Set on the press and kept through the release: egui emits the click on the release frame.
    // The next press decides again.
    if !pressed {
        return;
    }
    let outside_sheet = menu_tab(ctx).is_some() && {
        let rect = ctx.data(|d| d.get_temp::<Rect>(sheet_rect_id()));
        matches!((pos, rect), (Some(p), Some(r)) if !r.contains(p))
    };
    let outside_bar = bar_open(ctx) && {
        let rect = ctx.data(|d| d.get_temp::<Rect>(bar_rect_id()));
        matches!((pos, rect), (Some(p), Some(r)) if !r.contains(p))
    };
    let outside = outside_sheet || outside_bar;
    if outside {
        close_all(ctx);
    }
    ctx.data_mut(|d| d.insert_temp(gesture_id(), outside));
}

/// Whether Android Back has an open sheet (or its palette or row actions) to close first.
pub fn can_go_back(ctx: &Context) -> bool {
    menu_tab(ctx).is_some() || bar_open(ctx)
}

/// Escape or Back: closes the innermost open part first (the action bar, row actions, then the
/// palette), then the sheet.
pub fn back_step(ctx: &Context) {
    if bar_open(ctx) {
        set_bar_open(ctx, false);
    } else if row_actions(ctx).is_some() {
        set_row_actions(ctx, None);
    } else if open_palette(ctx).is_some() {
        set_palette(ctx, None);
    } else {
        set_menu(ctx, None);
    }
}

/// Opens a phone sheet by name, for the snapshot tool: `sheets`, `more`, a tab's name (`Home`,
/// `Insert`, ...) or `actions:N` for the row actions of sheet N in the Sheets sheet.
pub fn open_by_name(ctx: &Context, which: &str) {
    if which == "sheets" {
        set_menu(ctx, Some(SHEETS));
    } else if which == "more" {
        set_menu(ctx, Some(MORE));
    } else if let Some(n) = which.strip_prefix("actions:").and_then(|n| n.parse().ok()) {
        set_menu(ctx, Some(SHEETS));
        set_row_actions(ctx, Some(n));
    } else if which == "actionbar" {
        set_bar_open(ctx, true);
    } else if let Some(i) = TABS.iter().position(|(label, _)| *label == which) {
        set_menu(ctx, Some(i));
    }
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

/// Whether all the command tabs fit beside the sheet button. If not, the last one moves into More.
fn all_tabs_fit(ctx: &Context) -> bool {
    let pill_w = ctx.content_rect().width() - 2.0 * PILL_SIDE;
    pill_w - PILL_SPARE >= TABS.len() as f32 * MIN_TAB_W
}

/// The bar's slots in order: a tab index, or [`MORE`].
fn pill_slots(ctx: &Context) -> Vec<usize> {
    if all_tabs_fit(ctx) { (0..TABS.len()).collect() } else { (0..NARROW_TABS).chain([MORE]).collect() }
}

/// The floating bar at the bottom, and the bottom sheet that opens in its place. The bar is a
/// bottom panel, so the grid stops above it and nothing is drawn under it.
pub fn bottom_bar(app: &mut SheetApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let t = Tokens::get(&ctx);
    let lang = app.ui.language;
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        back_step(&ctx);
    }
    take_pointer(&ctx);
    // Editing a cell: the edit bar replaces the floating bar and sits directly above the keyboard.
    track_edit(app, &ctx);
    if app.editor.is_some() {
        set_menu(&ctx, None);
        return edit_bar(app, ui);
    }
    match menu_tab(&ctx) {
        Some(SHEETS) => return sheets_sheet(app, ui),
        Some(MORE) => return more_sheet(app, ui),
        Some(tab) => return tab_sheet(app, ui, tab),
        None => {}
    }
    Panel::bottom("phone_tab_bar").exact_size(PILL_H + 2.0 * FLOAT_GAP).frame(Frame::NONE.fill(t.grid_bg)).show(ui, |ui| {
        let outer = ui.max_rect();
        let pill = Rect::from_min_max(
            Pos2::new(outer.left() + PILL_SIDE, outer.top() + FLOAT_GAP),
            Pos2::new(outer.right() - PILL_SIDE, outer.bottom() - FLOAT_GAP),
        );
        ui.painter().rect(pill, CornerRadius::same((PILL_H / 2.0) as u8), t.ribbon, Stroke::new(1.0, t.ribbon_border), StrokeKind::Inside);
        sheet_button(app, ui, pill);
        let div_x = pill.left() + 4.0 + SHEET_BTN_W + 4.0;
        ui.painter().line_segment([Pos2::new(div_x, pill.top() + 14.0), Pos2::new(div_x, pill.bottom() - 14.0)], Stroke::new(1.0, t.ribbon_border));
        let tabs_rect = Rect::from_min_max(Pos2::new(div_x + 4.0, pill.top()), Pos2::new(pill.right() - 8.0, pill.bottom()));
        let slots = pill_slots(&ctx);
        let w = tabs_rect.width() / slots.len() as f32;
        for (i, &slot) in slots.iter().enumerate() {
            let cell = Rect::from_min_size(Pos2::new(tabs_rect.left() + i as f32 * w, pill.top()), vec2(w, pill.height()));
            let (label, icon) = if slot == MORE { ("More", Icon::More) } else { TABS[slot] };
            let resp = ui.interact(cell, ui.id().with(("phone_tab", slot)), Sense::click());
            // The indicator is the 48 x 28 pill behind the icon, as Material draws an active tab.
            let indicator = Rect::from_center_size(Pos2::new(cell.center().x, pill.top() + 18.0), vec2(48.0, 28.0));
            let painter = ui.painter();
            if resp.is_pointer_button_down_on() {
                painter.rect_filled(indicator, 14.0, t.pressed);
            } else if resp.hovered() {
                painter.rect_filled(indicator, 14.0, t.hover);
            }
            painter.text(Pos2::new(cell.center().x, pill.top() + 44.0), Align2::CENTER_CENTER, lang.tr(label), theme::ui_font(11.0), t.text_dim);
            icons::paint(painter, Rect::from_center_size(indicator.center(), vec2(22.0, 22.0)), icon, t.text);
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, lang.tr(label)));
            if resp.clicked() {
                set_menu(&ctx, Some(slot));
            }
        }
    });
}

/// The edit bar's height, and its 48 pt buttons: a red cancel (✗) at the left, a green confirm (✓) at
/// the right.
const EDIT_BAR_H: f32 = 56.0;
const CANCEL_RED: Color32 = Color32::from_rgb(0xC6, 0x28, 0x28);
const CONFIRM_GREEN: Color32 = Color32::from_rgb(0x1E, 0x7E, 0x34);

/// Keeps the edited cell in view. The grid scrolls to it when an edit starts and again whenever the
/// window height changes while editing (the keyboard coming up or going down).
fn track_edit(app: &mut SheetApp, ctx: &Context) {
    let id = egui::Id::new("phone_edit_height");
    if app.editor.is_none() {
        ctx.data_mut(|d| d.remove::<f32>(id));
        return;
    }
    let h = ctx.content_rect().height();
    let prev = ctx.data_mut(|d| d.get_temp::<f32>(id));
    ctx.data_mut(|d| d.insert_temp(id, h));
    if prev.is_none_or(|p| (p - h).abs() > 1.0) {
        app.grid.ensure_visible = true;
    }
}

/// The edit bar, shown while a cell is being edited. ✗ cancels the edit (the keyboard closes with
/// it); ✓ commits and moves down, as Enter does.
fn edit_bar(app: &mut SheetApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let lang = app.ui.language;
    let t = Tokens::get(&ctx);
    Panel::bottom("phone_edit_bar").exact_size(EDIT_BAR_H).frame(Frame::NONE.fill(t.ribbon)).show(ui, |ui| {
        let bar = ui.max_rect();
        let top = bar.top() + (EDIT_BAR_H - ICON_BTN) / 2.0;
        let cancel = Rect::from_min_size(Pos2::new(bar.left() + 4.0, top), vec2(ICON_BTN, ICON_BTN));
        let confirm = Rect::from_min_size(Pos2::new(bar.right() - 4.0 - ICON_BTN, top), vec2(ICON_BTN, ICON_BTN));
        if edit_button(ui, t, cancel, Icon::Close, CANCEL_RED, lang.tr("Cancel")) {
            app.cancel_edit();
        }
        if edit_button(ui, t, confirm, Icon::Check, CONFIRM_GREEN, lang.tr("Enter")) {
            app.commit_edit(1, 0, false, false);
        }
    });
}

/// One edit-bar button: a large coloured mark in a 48 pt touch target. Returns whether it was tapped.
fn edit_button(ui: &mut Ui, t: Tokens, r: Rect, icon: Icon, color: Color32, label: &str) -> bool {
    let resp = ui.interact(r, ui.id().with(("phone_edit", label)), Sense::click());
    let painter = ui.painter();
    if resp.is_pointer_button_down_on() {
        painter.circle_filled(r.center(), ICON_BTN / 2.0, t.pressed);
    } else if resp.hovered() {
        painter.circle_filled(r.center(), ICON_BTN / 2.0, t.hover);
    }
    icons::paint(painter, Rect::from_center_size(r.center(), vec2(32.0, 32.0)), icon, color);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    resp.clicked()
}

/// The sheet button at the bar's left: a sheet icon and the active sheet's name, cut short with an
/// ellipsis. Tapping it opens the Sheets sheet.
fn sheet_button(app: &mut SheetApp, ui: &mut Ui, pill: Rect) {
    let ctx = ui.ctx().clone();
    let t = Tokens::get(&ctx);
    let lang = app.ui.language;
    let name = app.session.active().and_then(|d| d.wb.active().map(|s| s.name.clone())).unwrap_or_default();
    let btn = Rect::from_min_size(pill.left_top() + vec2(4.0, 4.0), vec2(SHEET_BTN_W, pill.height() - 8.0));
    let resp = ui.interact(btn, ui.id().with("phone_sheet_button"), Sense::click());
    let painter = ui.painter();
    let base = if resp.is_pointer_button_down_on() {
        t.pressed
    } else if resp.hovered() {
        t.hover
    } else {
        t.accent_soft
    };
    painter.rect_filled(btn, CornerRadius::same((btn.height() / 2.0) as u8), base);
    icons::paint(painter, Rect::from_center_size(Pos2::new(btn.left() + 22.0, btn.center().y), vec2(20.0, 20.0)), Icon::Sheet, t.accent);
    let text_left = btn.left() + 38.0;
    paint_truncated(painter, text_left, btn.center().y, (btn.right() - 8.0 - text_left).max(0.0), &name, theme::ui_bold(14.0), t.text);
    let label = format!("{}: {name}", lang.tr("Sheets"));
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.clone()));
    if resp.clicked() {
        set_menu(&ctx, Some(SHEETS));
    }
}

/// Paints one line of `text` from `left`, vertically centred on `y`, cut with an ellipsis to `max_w`.
fn paint_truncated(painter: &Painter, left: f32, y: f32, max_w: f32, text: &str, font: FontId, color: Color32) {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap.max_width = max_w;
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('…');
    let galley = painter.layout_job(job);
    let pos = Pos2::new(left, y - galley.size().y / 2.0);
    painter.galley(pos, galley, color);
}

/// The bottom sheet's frame: a handle, a title row with Close, then the body in a scroll area.
fn sheet_panel(app: &mut SheetApp, ui: &mut Ui, title: &str, body: impl FnOnce(&mut SheetApp, &mut Ui)) {
    let ctx = ui.ctx().clone();
    let t = Tokens::get(&ctx);
    let lang = app.ui.language;
    let screen_h = ctx.content_rect().height();
    let height = (screen_h * 0.55).clamp(260.0, 520.0).min(screen_h * 0.85);
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
                    egui::Label::new(egui::RichText::new(title).font(theme::ui_bold(18.0)).color(t.text)).truncate(),
                );
                if icon_button(ui, t, Icon::Close, lang.tr("Close"), true).clicked() {
                    close_all(&ctx);
                }
            });
            egui::ScrollArea::vertical().show(ui, |ui| body(app, ui));
        });
    ctx.data_mut(|d| d.insert_temp(sheet_rect_id(), panel.response.rect));
}

/// The sheet of a command tab: its colour palette when a colour row is open, then its rows.
fn tab_sheet(app: &mut SheetApp, ui: &mut Ui, tab: usize) {
    let title = app.ui.language.tr(TABS.get(tab).map_or("", |tb| tb.0));
    sheet_panel(app, ui, title, |app, ui| {
        let ctx = ui.ctx().clone();
        let t = Tokens::get(&ctx);
        let lang = app.ui.language;
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
                close_all(&ctx);
            }
            ui.add_space(4.0);
        }
        for (label, icon, act) in rows(tab) {
            if list_row(ui, t, lang.tr(label), *icon, true).clicked() {
                match *act {
                    Act::Colour(p) => set_palette(&ctx, if open_palette(&ctx) == Some(p) { None } else { Some(p) }),
                    other => {
                        run_act(app, &ctx, other);
                        close_all(&ctx);
                    }
                }
            }
        }
    });
}

/// The More sheet: the command tabs that do not fit on the bar.
fn more_sheet(app: &mut SheetApp, ui: &mut Ui) {
    let title = app.ui.language.tr("More");
    sheet_panel(app, ui, title, |app, ui| {
        let ctx = ui.ctx().clone();
        let t = Tokens::get(&ctx);
        let lang = app.ui.language;
        for (i, (label, icon)) in TABS.iter().enumerate().skip(NARROW_TABS) {
            if list_row(ui, t, lang.tr(label), *icon, true).clicked() {
                set_menu(&ctx, Some(i));
            }
        }
    });
}

/// The Sheets sheet: one row per visible sheet (tap to switch, ⋯ for its actions), then New sheet.
/// With a sheet's row actions open, the sheet shows Rename, Duplicate and Delete for it instead.
fn sheets_sheet(app: &mut SheetApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    if let Some(i) = row_actions(&ctx) {
        let name = app.session.active().and_then(|d| d.wb.sheets.get(i).map(|s| s.name.clone())).unwrap_or_default();
        return sheet_panel(app, ui, &name, |app, ui| actions_body(app, ui, i));
    }
    let title = app.ui.language.tr("Sheets");
    sheet_panel(app, ui, title, sheet_list);
}

/// The Sheets sheet's rows.
fn sheet_list(app: &mut SheetApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let t = Tokens::get(&ctx);
    let lang = app.ui.language;
    let (sheets, active) = app
        .session
        .active()
        .map(|d| {
            let visible: Vec<(usize, String)> =
                d.wb.sheets.iter().enumerate().filter(|(_, s)| s.visibility == Visibility::Visible).map(|(i, s)| (i, s.name.clone())).collect();
            (visible, d.wb.active_sheet)
        })
        .unwrap_or_default();
    for (i, name) in sheets {
        let (main, more) = sheet_row(ui, t, &name, i, i == active);
        if main.clicked() {
            tabs::activate_sheet(app, i);
            close_all(&ctx);
        } else if more.clicked() {
            set_row_actions(&ctx, Some(i));
        }
    }
    if list_row(ui, t, lang.tr("New sheet"), Icon::Plus, true).clicked() {
        app.run_or_alert("home.insertSheet", json!({}));
        close_all(&ctx);
    }
}

/// The row actions of sheet `i`. Delete waits until the workbook has another sheet to keep.
fn actions_body(app: &mut SheetApp, ui: &mut Ui, i: usize) {
    let ctx = ui.ctx().clone();
    let t = Tokens::get(&ctx);
    let lang = app.ui.language;
    let count = app.session.active().map_or(0, |d| d.wb.sheets.len());
    if list_row(ui, t, lang.tr("Rename"), Icon::Pen, true).clicked() {
        close_all(&ctx);
        app.open_dialog("renameSheet", json!({"sheet": i}));
    }
    if list_row(ui, t, lang.tr("Duplicate"), Icon::Copy, true).clicked() {
        close_all(&ctx);
        app.run_or_alert("sheet.move", json!({"sheet": i, "to": i + 1, "copy": true}));
    }
    if list_row(ui, t, lang.tr("Delete"), Icon::Delete, count > 1).clicked() {
        close_all(&ctx);
        app.run_or_alert("home.deleteSheet", json!({"sheet": i}));
    }
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

/// A sheet row: the icon, then the label, in a 48 pt touch target across the sheet. A disabled row
/// is greyed and never clicked.
fn list_row(ui: &mut Ui, t: Tokens, label: &str, icon: Icon, enabled: bool) -> Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), sense);
    if enabled && resp.is_pointer_button_down_on() {
        ui.painter().rect_filled(r, 0.0, t.pressed);
    } else if enabled && resp.hovered() {
        ui.painter().rect_filled(r, 0.0, t.hover);
    }
    let ink = if enabled { t.text } else { t.text_disabled };
    let painter = ui.painter();
    icons::paint(painter, Rect::from_center_size(Pos2::new(r.left() + 24.0, r.center().y), vec2(24.0, 24.0)), icon, ink);
    painter.text(Pos2::new(r.left() + 60.0, r.center().y), Align2::LEFT_CENTER, label, theme::ui_font(15.0), ink);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    resp
}

/// A sheet's row in the Sheets sheet: the sheet icon and name (the active sheet in the accent, with
/// a check), and a 48 pt ⋯ button at the right. Returns the row's main part and the ⋯ button.
fn sheet_row(ui: &mut Ui, t: Tokens, name: &str, index: usize, active: bool) -> (Response, Response) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
    let main_r = Rect::from_min_max(r.min, Pos2::new(r.right() - ICON_BTN, r.bottom()));
    let more_r = Rect::from_min_max(Pos2::new(r.right() - ICON_BTN, r.top()), r.max);
    let main = ui.interact(main_r, ui.id().with(("phone_sheet_row", index)), Sense::click());
    let more = ui.interact(more_r, ui.id().with(("phone_sheet_more", index)), Sense::click());
    let painter = ui.painter();
    if active {
        painter.rect_filled(main_r, 0.0, t.accent_soft);
    }
    if main.is_pointer_button_down_on() {
        painter.rect_filled(main_r, 0.0, t.pressed);
    } else if main.hovered() {
        painter.rect_filled(main_r, 0.0, t.hover);
    }
    let ink = if active { t.accent } else { t.text };
    icons::paint(painter, Rect::from_center_size(Pos2::new(main_r.left() + 24.0, r.center().y), vec2(24.0, 24.0)), Icon::Sheet, ink);
    let text_left = main_r.left() + 60.0;
    paint_truncated(painter, text_left, r.center().y, (main_r.right() - 48.0 - text_left).max(0.0), name, theme::ui_font(15.0), ink);
    if active {
        icons::paint(painter, Rect::from_center_size(Pos2::new(main_r.right() - 24.0, r.center().y), vec2(20.0, 20.0)), Icon::Check, t.accent);
    }
    if more.is_pointer_button_down_on() {
        painter.circle_filled(more_r.center(), ICON_BTN / 2.0, t.pressed);
    } else if more.hovered() {
        painter.circle_filled(more_r.center(), ICON_BTN / 2.0, t.hover);
    }
    icons::paint(painter, Rect::from_center_size(more_r.center(), vec2(24.0, 24.0)), Icon::More, t.text_dim);
    main.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
    more.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("More actions: {name}")));
    (main, more)
}

/// Touch selection on the grid: a handle at each corner of the selection, and a long press that
/// selects a cell and opens the action bar above (or below) the selection.
/// A handle's dot is 12 pt; its touch target is 44 pt.
const HANDLE_HIT: f32 = 44.0;
const HANDLE_R: f32 = 6.0;
/// A press held this long without moving opens the action bar.
const LONG_PRESS: f64 = 0.5;
/// An action button in the bar: 72 pt wide, the bar 64 pt tall (the bar scrolls across when it overflows).
const BAR_BTN_W: f32 = 72.0;
const BAR_H: f32 = 64.0;
const BAR_GAP: f32 = 8.0;
/// Within this many points of a cell edge, a handle drag scrolls the grid.
const EDGE: f32 = 24.0;

/// The action bar's buttons: the clipboard, Clear and Fill Down run their commands, the two insert and
/// delete entries open their dialogs (as the ribbon's ellipsis entries do).
const BAR_ITEMS: [(&str, Icon, Act); 7] = [
    ("Cut", Icon::Cut, Act::Copy("edit.cut")),
    ("Copy", Icon::Copy, Act::Copy("edit.copy")),
    ("Paste", Icon::Paste, Act::Run("edit.paste")),
    ("Clear", Icon::Eraser, Act::Run("edit.clearContents")),
    ("Fill Down", Icon::FillDown, Act::Run("edit.fillDown")),
    ("Insert…", Icon::Insert, Act::Dialog("insertCells", None)),
    ("Delete…", Icon::Delete, Act::Dialog("deleteCells", None)),
];

fn bar_id() -> egui::Id {
    egui::Id::new("phone_action_bar_open")
}

/// Whether the action bar is open.
pub fn action_bar_open(ctx: &Context) -> bool {
    bar_open(ctx)
}

fn bar_open(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(bar_id())).unwrap_or(false)
}

fn set_bar_open(ctx: &Context, open: bool) {
    ctx.data_mut(|d| d.insert_temp(bar_id(), open));
}

/// Where the action bar was drawn last frame. A press outside it closes the bar.
fn bar_rect_id() -> egui::Id {
    egui::Id::new("phone_action_bar_rect")
}

/// The press start time of the long press that last opened the bar, so one press opens it once.
fn long_press_id() -> egui::Id {
    egui::Id::new("phone_long_press_fired")
}

/// The selection area after a handle drag: the corner opposite the dragged handle stays where it is,
/// and the dragged corner goes to `cell`. `moving_start` is the top-left handle.
pub(crate) fn handle_range(current: RangeRef, moving_start: bool, cell: CellRef) -> RangeRef {
    let fixed = if moving_start { current.end } else { current.start };
    RangeRef::new(fixed, cell)
}

/// The grid's auto-scroll while a handle is dragged near the cells' edges, in logical points.
fn edge_scroll(cells: Rect, p: Pos2) -> egui::Vec2 {
    let mut d = vec2(0.0, 0.0);
    if p.y > cells.bottom() - EDGE {
        d.y = 20.0;
    } else if p.y < cells.top() + EDGE {
        d.y = -20.0;
    }
    if p.x > cells.right() - EDGE {
        d.x = 30.0;
    } else if p.x < cells.left() + EDGE {
        d.x = -30.0;
    }
    d
}

/// Runs in the grid after it is painted, before its own input. Draws the handles at the selection's
/// top-left and bottom-right corners and takes their drags, opens the action bar on a long press, and
/// draws the bar. Returns whether a handle or a long press took this frame, so the grid skips its own
/// input for the frame.
pub(crate) fn touch_gestures(app: &mut SheetApp, ui: &mut Ui, geo: &Geo, sh: &Sheet) -> bool {
    let ctx = ui.ctx().clone();
    if !active(&ctx) || app.editor.is_some() || app.text_box_editor.is_some() || app.dialog.is_some() {
        return false;
    }
    let t = Tokens::get(&ctx);
    let sel = app.session.active().map(|d| d.selection.clone()).unwrap_or_default();
    let cur = sel.current();
    let logical = geo.range_rect(sh, cur);
    let mut took = false;
    let mut handles: Vec<(bool, Pos2)> = Vec::new();
    for (i, (moving_start, corner)) in [(true, logical.left_top()), (false, logical.right_bottom())].into_iter().enumerate() {
        let at = geo.mirror_pos(corner);
        if !geo.cells.contains(at) {
            continue;
        }
        handles.push((moving_start, at));
        let resp = ui.interact(Rect::from_center_size(at, vec2(HANDLE_HIT, HANDLE_HIT)), ui.id().with(("phone_handle", i)), Sense::drag());
        ui.painter().circle(at, HANDLE_R, t.accent, Stroke::new(2.0, t.window));
        if !resp.dragged() {
            continue;
        }
        took = true;
        if let Some(p) = resp.interact_pointer_pos().map(|p| geo.mirror_pos(p)) {
            let p = Pos2::new(p.x.clamp(geo.cells.left() + 1.0, geo.cells.right() - 1.0), p.y.clamp(geo.cells.top() + 1.0, geo.cells.bottom() - 1.0));
            let range = handle_range(cur, moving_start, geo.cell_at(sh, p));
            let fixed = if moving_start { cur.end } else { cur.start };
            // The active cell stays put while it is in the area; otherwise it moves to the area's start.
            let active = if range.contains(sel.active) { sel.active } else { range.start };
            let mut ranges: Vec<String> = sel.ranges.iter().map(|r| r.a1()).collect();
            ranges.pop();
            ranges.push(range.a1());
            let _ = app.session.run("selection.set", json!({"range": ranges.join(","), "active": active.a1()}));
            if let Some(d) = app.session.active_mut() {
                d.selection.anchor = fixed;
            }
            grid::scroll_by(app, edge_scroll(geo.cells, p));
            ctx.request_repaint();
        }
    }

    // A long press: held without moving, on a cell but not on a handle. It runs once per press.
    let (down, origin, started, decided, multi, now) = ctx.input(|i| {
        (
            i.pointer.primary_down(),
            i.pointer.press_origin(),
            i.pointer.press_start_time(),
            i.pointer.is_decidedly_dragging(),
            i.multi_touch().is_some(),
            i.time,
        )
    });
    if let (true, Some(origin), Some(t0)) = (down && !decided && !multi && !took && !pointer_taken(&ctx), origin, started) {
        let p = geo.mirror_pos(origin);
        let on_handle = handles.iter().any(|(_, h)| Rect::from_center_size(*h, vec2(HANDLE_HIT, HANDLE_HIT)).contains(origin));
        let fired = ctx.data(|d| d.get_temp::<f64>(long_press_id()));
        if geo.in_cells(p) && !on_handle && fired != Some(t0) {
            if now - t0 >= LONG_PRESS {
                ctx.data_mut(|d| d.insert_temp(long_press_id(), t0));
                let cell = geo.cell_at(sh, p);
                if !sel.contains(cell) {
                    let _ = app.session.run("selection.set", json!({"cell": cell.a1()}));
                }
                set_bar_open(&ctx, true);
                // The grid ignores the rest of this press, so its release is no click.
                ctx.data_mut(|d| d.insert_temp(gesture_id(), true));
                took = true;
            } else {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(LONG_PRESS - (now - t0)));
            }
        }
    }

    if bar_open(&ctx) {
        action_bar(app, &ctx, geo.cells, geo.mirror_rect(logical));
    }
    took
}

/// The action bar: a row of 48 pt-plus buttons in a card above the selection (below it when there is
/// no room above), 16 pt in from the screen's sides, scrolling across when the buttons overflow.
fn action_bar(app: &mut SheetApp, ctx: &Context, cells: Rect, selection: Rect) {
    let t = Tokens::get(ctx);
    let lang = app.ui.language;
    let width = (ctx.content_rect().width() - 2.0 * PILL_SIDE).max(0.0);
    let above = selection.top() - BAR_GAP - BAR_H;
    let below = selection.bottom() + BAR_GAP;
    let y = if above >= cells.top() {
        above
    } else if below + BAR_H <= cells.bottom() {
        below
    } else {
        cells.top() + BAR_GAP
    };
    let mut picked = None;
    let area = egui::Area::new(egui::Id::new("phone_action_bar_area")).order(egui::Order::Foreground).fixed_pos(Pos2::new(PILL_SIDE, y)).show(ctx, |ui| {
        Frame::NONE
            .fill(t.menu_bg)
            .stroke(Stroke::new(1.0, t.ribbon_border))
            .corner_radius(CornerRadius::same(12))
            .inner_margin(Margin::symmetric(4, 0))
            .show(ui, |ui| {
                ui.set_width((width - 8.0).max(0.0));
                egui::ScrollArea::horizontal().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (label, icon, act) in BAR_ITEMS.iter() {
                            if bar_button(ui, t, lang.tr(label), *icon) {
                                picked = Some(*act);
                            }
                        }
                    });
                });
            });
    });
    ctx.data_mut(|d| d.insert_temp(bar_rect_id(), area.response.rect));
    if let Some(act) = picked {
        set_bar_open(ctx, false);
        // The tapped button leaves with the bar: drop its focus so none is left on a missing widget.
        if let Some(id) = ctx.memory(|m| m.focused()) {
            ctx.memory_mut(|m| m.surrender_focus(id));
        }
        run_act(app, ctx, act);
    }
}

/// One action button: its icon above its label, in a 72 x 64 pt touch target. Returns whether it was tapped.
fn bar_button(ui: &mut Ui, t: Tokens, label: &str, icon: Icon) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(BAR_BTN_W, BAR_H), Sense::click());
    if resp.is_pointer_button_down_on() {
        ui.painter().rect_filled(r, CornerRadius::same(10), t.pressed);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, CornerRadius::same(10), t.hover);
    }
    let painter = ui.painter();
    icons::paint(painter, Rect::from_center_size(Pos2::new(r.center().x, r.top() + 20.0), vec2(24.0, 24.0)), icon, t.text);
    painter.text(Pos2::new(r.center().x, r.top() + 46.0), Align2::CENTER_CENTER, label, theme::ui_font(11.0), t.text_dim);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    resp.clicked()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dragging_the_end_handle_keeps_the_start_corner() {
        let a1_b4 = RangeRef::new(CellRef::new(0, 0), CellRef::new(3, 1));
        // The bottom-right handle dragged to D6: the area grows from A1.
        assert_eq!(handle_range(a1_b4, false, CellRef::new(5, 3)).a1(), "A1:D6");
        // The top-left handle dragged to C2: the area is B2:C4, normalised around the fixed B4 corner.
        assert_eq!(handle_range(a1_b4, true, CellRef::new(1, 2)).a1(), "B2:C4");
    }

    #[test]
    fn dragging_a_handle_past_the_other_corner_flips_the_area() {
        let a1_b4 = RangeRef::new(CellRef::new(0, 0), CellRef::new(3, 1));
        // The top-left handle dragged below and right of B4: the fixed corner is now the start.
        assert_eq!(handle_range(a1_b4, true, CellRef::new(6, 4)).a1(), "B4:E7");
    }
}
