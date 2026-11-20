//! The List style of the phone's toolbar sheets (phone plan part 1.6a), an option beside Tiles in
//! File › Options › General › Phone layout. It has no tab bar: a floating quick bar holds a few
//! common commands and More, which opens the sheet at the tab used last. The sheet's header is a
//! tab menu (every tab, the contextual tabs, then Search and All tools). Its body draws the same
//! model as the tiles (`tiles::sections`): one-tap formats as a row of icon buttons, every other
//! command as a list row.

use egui::{Align2, Context, CornerRadius, Panel, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, vec2};
use serde_json::json;

use super::tiles::{self, Item};
use super::{Act, GUTTER, ICON_BTN, SubPage, TABS, TOUCH, set_menu, set_palette, tools};
use crate::SheetApp;
use crate::icons::{self, Icon};
use crate::theme::{self, Tokens};

/// The quick bar: its height and the gap around it.
const QUICK_H: f32 = 56.0;
const QUICK_GAP: f32 = 8.0;
/// The sheet button's width in the quick bar.
const CHIP_W: f32 = super::SHEET_BTN_W;
/// A thin divider between groups.
const DIVIDER_W: f32 = 9.0;

/// Whether the List style is on.
pub(super) fn on(app: &SheetApp) -> bool {
    app.ui.phone_list_style
}

fn last_tab_id() -> egui::Id {
    egui::Id::unique("phone_list_last_tab")
}

/// Remembers the tab a sheet showed, for More to reopen.
pub(super) fn remember(ctx: &Context, tab: usize) {
    ctx.data_mut(|d| d.insert_temp(last_tab_id(), tab));
}

/// The tab More opens: the one used last, else Home.
fn last_tab(ctx: &Context) -> usize {
    ctx.data(|d| d.get_temp::<usize>(last_tab_id())).filter(|t| *t < TABS.len()).unwrap_or(0)
}

/// What a quick bar button does.
#[derive(Clone, Copy)]
enum Quick {
    Run(&'static str),
    /// Opens a tab's sheet at a sub-page.
    Page(usize, SubPage),
}

/// The quick bar's buttons after the sheet button, in order: dropped from the right when they do
/// not fit. Bold shows on for the active cell.
const QUICK: [(Icon, &str, Quick); 4] = [
    (Icon::Bold, "home.bold", Quick::Run("home.bold")),
    (Icon::Fill, "home.fillColor", Quick::Page(0, SubPage::Fill)),
    (Icon::Sum, "formulas.autoSum", Quick::Run("formulas.autoSum")),
    (Icon::SortAsc, "data.sortAscending", Quick::Run("data.sortAscending")),
];

/// How many quick buttons fit a quick bar `pill_w` wide, beside the sheet button, More and two
/// dividers.
pub(super) fn quick_fit(pill_w: f32) -> usize {
    let room = pill_w - 8.0 - CHIP_W - 2.0 * DIVIDER_W - ICON_BTN;
    (((room + 1.0) / ICON_BTN).floor().max(0.0) as usize).min(QUICK.len())
}

/// The quick bar (List style, nothing open): a floating pill at the bottom with the sheet button,
/// the quick commands and More.
pub(super) fn quick_bar(app: &mut SheetApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let t = Tokens::get(&ctx);
    let lang = app.ui.language;
    // The tab bar is not drawn in this style: a press where it was must not count as on it.
    ctx.data_mut(|d| d.insert_temp(super::tab_bar_rect_id(), Rect::NOTHING));
    let st = tiles::active_style(app);
    Panel::bottom("phone_quick_bar").exact_size(QUICK_H + 2.0 * QUICK_GAP).frame(egui::Frame::NONE.fill(t.cell_bg)).show(ui, |ui| {
        let outer = ui.max_rect();
        let pill = Rect::from_min_max(
            Pos2::new(outer.left() + GUTTER, outer.top() + QUICK_GAP),
            Pos2::new(outer.right() - GUTTER, outer.bottom() - QUICK_GAP),
        );
        ui.painter().rect(pill, CornerRadius::same((QUICK_H / 2.0) as u8), t.window, Stroke::new(1.0, t.ribbon_border), StrokeKind::Inside);
        let y = pill.center().y;
        let chip = Rect::from_min_size(Pos2::new(pill.left() + 6.0, y - 22.0), vec2(CHIP_W, 44.0));
        super::sheet_button(app, ui, chip);
        let mut x = chip.right() + DIVIDER_W / 2.0;
        divider(ui, t, x, y);
        x += DIVIDER_W / 2.0;
        let shown = quick_fit(pill.width());
        let more_x = pill.right() - 4.0 - ICON_BTN;
        // The quick buttons are centred in the room between the divider and More's divider.
        let room = more_x - DIVIDER_W - x;
        let start = x + (room - shown as f32 * ICON_BTN).max(0.0) / 2.0;
        for (i, (icon, cmd_id, act)) in QUICK.iter().take(shown).enumerate() {
            let r = Rect::from_min_size(Pos2::new(start + i as f32 * ICON_BTN, y - ICON_BTN / 2.0), vec2(ICON_BTN, ICON_BTN));
            let label = gridcraft_engine::cmd::find_command(cmd_id).map_or(*cmd_id, |s| s.label);
            let label = lang.tr(label.trim_end_matches('…'));
            let is_on = tiles::is_on(app, &st, cmd_id);
            if icon_button(ui, t, r, *icon, label, is_on, ("phone_quick", i)) {
                match *act {
                    Quick::Run(id) => app.run_or_alert(id, json!({})),
                    Quick::Page(tab, page) => {
                        set_menu(&ctx, Some(tab));
                        set_palette(&ctx, Some(page));
                    }
                }
            }
        }
        divider(ui, t, more_x - DIVIDER_W / 2.0, y);
        let r = Rect::from_min_size(Pos2::new(more_x, y - ICON_BTN / 2.0), vec2(ICON_BTN, ICON_BTN));
        if icon_button(ui, t, r, Icon::More, lang.tr("More"), false, ("phone_quick", 99)) {
            set_menu(&ctx, Some(last_tab(&ctx)));
        }
    });
}

fn divider(ui: &Ui, t: Tokens, x: f32, y: f32) {
    ui.painter().vline(x, (y - 14.0)..=(y + 14.0), Stroke::new(1.0, t.ribbon_border));
}

/// A 48 pt icon button at `r`. An "on" button has the accent tint behind its icon. True when tapped.
fn icon_button(ui: &mut Ui, t: Tokens, r: Rect, icon: Icon, label: &str, on: bool, id: impl std::hash::Hash + std::fmt::Debug) -> bool {
    let resp = ui.interact(r, ui.scope_id().with(id), Sense::click());
    let bg = r.shrink(2.0);
    if on {
        ui.painter().rect_filled(bg, CornerRadius::same(12), t.accent_soft);
    }
    if resp.is_pointer_button_down_on() {
        ui.painter().rect_filled(bg, CornerRadius::same(12), t.pressed);
    }
    icons::paint(ui.painter(), Rect::from_center_size(r.center(), vec2(24.0, 24.0)), icon, if on { t.accent } else { t.text });
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, label));
    resp.on_hover_text(label).clicked()
}

/// The tab menu chip in the sheet's header (List style): the tab's name and a down arrow. Its menu
/// lists every tab, the contextual tabs that apply now under Selection, then Search and All tools.
pub(super) fn tab_menu(app: &mut SheetApp, ui: &mut Ui, tab: usize) {
    let ctx = ui.ctx().clone();
    let t = Tokens::get(&ctx);
    let lang = app.ui.language;
    let name = lang.tr(TABS.get(tab).map_or("", |tb| tb.0)).to_string();
    let font = theme::ui_font(16.0);
    let text_w = ui.painter().layout_no_wrap(name.clone(), font.clone(), t.text).size().x;
    let (r, resp) = ui.allocate_exact_size(vec2(text_w + 52.0, TOUCH), Sense::click());
    let chip = Rect::from_center_size(r.center(), vec2(r.width(), 40.0));
    let fill = if resp.is_pointer_button_down_on() { t.pressed } else { t.hover };
    ui.painter().rect_filled(chip, CornerRadius::same(20), fill);
    ui.painter().text(Pos2::new(chip.left() + 16.0, chip.center().y), Align2::LEFT_CENTER, &name, font, t.text);
    icons::paint(ui.painter(), Rect::from_center_size(Pos2::new(chip.right() - 20.0, chip.center().y), vec2(16.0, 16.0)), Icon::Chevron, t.text_dim);
    let menu_label = format!("{}: {name}", lang.tr("Tabs"));
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, menu_label.clone()));
    egui::Popup::menu(&resp).width(236.0).show(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        egui::ScrollArea::vertical().max_height((ctx.content_rect().height() * 0.45).max(TOUCH * 4.0)).show(ui, |ui| {
            for (i, (label, _)) in TABS.iter().enumerate() {
                if menu_row(ui, t, lang.tr(label), None, i == tab) {
                    set_menu(&ctx, Some(i));
                    ui.close();
                }
            }
            let contextual = tools::contextual_now(app);
            if !contextual.is_empty() {
                tiles::heading(ui, t, lang.tr("Selection"));
                for c in contextual {
                    if menu_row(ui, t, lang.tr(tools::tab_label(c)), None, false) {
                        tools::open(&ctx, tools::Tool::Tab(c));
                        ui.close();
                    }
                }
            }
        });
        // Pinned under the tabs, always in view: Search and All tools.
        ui.separator();
        if menu_row(ui, t, lang.tr("Search"), Some(Icon::Search), false) {
            tools::open(&ctx, tools::Tool::Search(String::new()));
            ui.close();
        }
        if menu_row(ui, t, lang.tr("All tools"), Some(Icon::SheetList), false) {
            tools::open(&ctx, tools::Tool::Tabs);
            ui.close();
        }
    });
}

/// A row of the tab menu: 48 pt, an optional icon, the current tab tinted with a check.
fn menu_row(ui: &mut Ui, t: Tokens, label: &str, icon: Option<Icon>, current: bool) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width().max(220.0), TOUCH), Sense::click());
    let pill = r.shrink2(vec2(2.0, 1.0));
    if current {
        ui.painter().rect_filled(pill, CornerRadius::same(23), t.accent_soft);
    } else if resp.is_pointer_button_down_on() {
        ui.painter().rect_filled(pill, CornerRadius::same(23), t.pressed);
    } else if resp.hovered() {
        ui.painter().rect_filled(pill, CornerRadius::same(23), t.hover);
    }
    let mut x = r.left() + 16.0;
    if let Some(i) = icon {
        icons::paint(ui.painter(), Rect::from_center_size(Pos2::new(x + 10.0, r.center().y), vec2(20.0, 20.0)), i, t.text);
        x += 32.0;
    }
    let ink = if current { t.accent } else { t.text };
    ui.painter().text(Pos2::new(x, r.center().y), Align2::LEFT_CENTER, label, theme::ui_font(15.0), ink);
    if current {
        icons::paint(ui.painter(), Rect::from_center_size(Pos2::new(r.right() - 20.0, r.center().y), vec2(18.0, 18.0)), Icon::Check, t.accent);
    }
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, current, label));
    resp.clicked()
}

/// Whether an item is a one-tap format, drawn in the row of icon buttons: a formatting command that
/// keeps the sheet open and needs no values.
fn one_tap(item: &Item) -> bool {
    item.keep
        && match item.act {
            Act::Run(id) => gridcraft_engine::cmd::find_command(id).is_some_and(|s| tools::required(s.params).is_empty()),
            Act::Zoom(_) => true,
            _ => false,
        }
}

/// The body of a tab's sheet in the List style: Home's font row, then each section: its one-tap
/// formats as a row of icon buttons (it scrolls sideways when it overflows), then its other
/// commands as list rows, with a chevron where one opens a page or asks for values.
pub(super) fn body(app: &mut SheetApp, ui: &mut Ui, tab: usize) {
    let ctx = ui.ctx().clone();
    let t = Tokens::get(&ctx);
    let lang = app.ui.language;
    let st = tiles::active_style(app);
    if tab == 0 {
        tiles::font_row(app, ui, &st);
    }
    for section in tiles::sections(tab) {
        tiles::heading(ui, t, lang.tr(section.heading));
        let quick: Vec<&Item> = section.items.iter().filter(|i| one_tap(i)).collect();
        if !quick.is_empty() {
            egui::ScrollArea::horizontal().id_salt(("phone_list_quick", tab, section.heading)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    for (i, item) in quick.iter().enumerate() {
                        let (r, _) = ui.allocate_exact_size(vec2(ICON_BTN, ICON_BTN), Sense::hover());
                        let label = lang.tr(tiles::label(item)).to_string();
                        let enabled = tiles::enabled(app, item);
                        let is_on = tiles::is_on(app, &st, item.cmd);
                        if enabled && icon_button(ui, t, r, item.icon, &label, is_on, ("phone_list_btn", tab, section.heading, i)) {
                            tiles::tap(app, &ctx, item);
                        }
                    }
                });
            });
        }
        for item in section.items.iter().filter(|i| !one_tap(i)) {
            let label = lang.tr(tiles::label(item)).to_string();
            let enabled = tiles::enabled(app, item);
            let resp = super::list_row(ui, t, &label, item.icon, enabled);
            let chevron = matches!(item.act, Act::Page(_))
                || matches!(item.act, Act::Run(id) if gridcraft_engine::cmd::find_command(id).is_some_and(|s| !tools::required(s.params).is_empty()));
            if chevron {
                let r = resp.rect;
                icons::paint(
                    ui.painter(),
                    Rect::from_center_size(Pos2::new(r.right() - 16.0, r.center().y), vec2(20.0, 20.0)),
                    Icon::Right,
                    t.text_dim,
                );
            }
            if resp.clicked() {
                tiles::tap(app, &ctx, item);
            }
        }
    }
    ui.add_space(GUTTER);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_quick_bar_drops_buttons_from_the_right_on_a_narrow_phone() {
        // The pill is the window less the side gutters and the panel's own margins (about 17 pt).
        let pill = |w: f32| w - 2.0 * GUTTER - 17.0;
        assert_eq!(quick_fit(pill(412.0)), QUICK.len(), "a 412 pt phone shows every quick button");
        assert_eq!(quick_fit(pill(360.0)), 2, "a 360 pt phone keeps Bold and Fill Color");
        assert_eq!(quick_fit(100.0), 0, "never fewer than none");
    }
}
