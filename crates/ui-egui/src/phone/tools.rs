//! All tools (the shared shell, phone plan part 1.7): every command of the engine, reachable on a
//! phone. Ported from DeckCraft's `phone_tools`.
//!
//! The tile sheets are the everyday front page. This is the rest: one screen per ribbon tab, built
//! from the engine's own command list (`command_specs`), so a command the engine gains shows up here
//! without a hand-kept table. A row runs its command the way the ribbon does (a command that needs
//! values opens its dialog); a row that needs one value from a short list opens a picker; a row that
//! needs typed values and has no dialog opens a small form. Search finds any command by name or id.
//!
//! Back closes one level: form or picker, then the tab, then the list of tabs, then back to More.

use egui::{Align2, Context, Pos2, Rect, Sense, Ui, vec2};
use gridcraft_engine::cmd::{CommandSpec, command_specs, find_command};
use serde_json::{Value, json};

use super::{GUTTER, ROW_H, TOUCH, close_all, set_menu};
use crate::SheetApp;
use crate::icons::{self, Icon};
use crate::theme::{self, Tokens};

/// The ribbon and contextual tabs, in the order they are listed. A command whose menu names none of
/// them is folded into one by [`tab_of`], or listed under "Other".
pub(super) const TABS: [&str; 16] = [
    "Home",
    "Insert",
    "Draw",
    "Page Layout",
    "Formulas",
    "Data",
    "Review",
    "View",
    "Automate",
    "Table Design",
    "Chart Design",
    "PivotTable Analyze",
    "PivotTable Design",
    "Format",
    "File",
    "Other",
];

/// Tabs listed only while the selection calls for them.
pub(super) const CONTEXTUAL: [&str; 5] = ["Table Design", "Chart Design", "PivotTable Analyze", "PivotTable Design", "Format"];

/// One screen of the tools. The stack kept in egui's memory is the way back.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Tool {
    /// The list of tabs.
    Tabs,
    /// Search every command (the text typed so far).
    Search(String),
    /// One tab's commands.
    Tab(&'static str),
    /// A choice for one command.
    Pick(Pick),
    /// Values to type for one command.
    Form(Form),
}

/// A choice: each option is the parameters to run the command with, and its label.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Pick {
    pub id: &'static str,
    pub options: Vec<(Value, String)>,
}

/// Values to type: (name, what the params doc says of it, the text so far).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Form {
    pub id: &'static str,
    pub fields: Vec<(String, String, String)>,
}

fn stack_id() -> egui::Id {
    egui::Id::unique("phone_tools_stack")
}

/// Where Back from the tools' last screen goes: the sheet a tile opened them from, else More.
fn return_id() -> egui::Id {
    egui::Id::unique("phone_tools_return")
}

/// The open screens, the top last. Empty while the tools are closed.
pub(super) fn stack(ctx: &Context) -> Vec<Tool> {
    ctx.data(|d| d.get_temp::<Vec<Tool>>(stack_id())).unwrap_or_default()
}

fn set_stack(ctx: &Context, s: Vec<Tool>) {
    ctx.data_mut(|d| d.insert_temp(stack_id(), s));
}

/// Opens the tools at `first` (the list of tabs, Search, or one tab).
pub(super) fn open(ctx: &Context, first: Tool) {
    let mut s = vec![Tool::Tabs];
    if first != Tool::Tabs {
        s.push(first);
    }
    set_stack(ctx, s);
    ctx.data_mut(|d| d.insert_temp::<Option<usize>>(return_id(), None));
    set_menu(ctx, Some(super::TOOLS));
}

/// Opens one screen of the tools (a picker or a form) from a sheet's tile: Back returns to `from`.
fn open_from(ctx: &Context, tool: Tool, from: Option<usize>) {
    set_stack(ctx, vec![tool]);
    ctx.data_mut(|d| d.insert_temp(return_id(), from));
    set_menu(ctx, Some(super::TOOLS));
}

/// A tile's command that needs values, from the sheet `from`: see [`launch`]. A picker or form opened
/// here returns to the sheet on Back.
pub(super) fn start(app: &mut SheetApp, ctx: &Context, spec: &'static CommandSpec, from: Option<usize>) {
    launch(app, ctx, spec, Some(from));
}

/// Closes the tools.
pub(super) fn clear(ctx: &Context) {
    set_stack(ctx, Vec::new());
}

/// Back one level; from the list of tabs, back to the More sheet.
pub(super) fn back(ctx: &Context) {
    let mut s = stack(ctx);
    s.pop();
    if s.is_empty() {
        let from = ctx.data(|d| d.get_temp::<Option<usize>>(return_id())).flatten();
        set_menu(ctx, Some(from.unwrap_or(super::MORE)));
    }
    set_stack(ctx, s);
}

/// The tab a command is listed under. Menu-only groups are folded into a tab.
pub(super) fn tab_of(spec: &CommandSpec) -> &'static str {
    let Some(first) = spec.menu.first() else { return "Other" };
    let m = match *first {
        "Edit" => "Home",
        "Window" => "View",
        "Table" => "Table Design",
        other => other,
    };
    TABS.iter().copied().find(|t| *t == m && *t != "Other").unwrap_or("Other")
}

/// The words a tab is called.
pub(super) fn tab_label(tab: &str) -> &str {
    match tab {
        "Format" => "Shape Format",
        "Other" => "Other Commands",
        other => other,
    }
}

/// Commands that act (queries that only answer have no row).
fn is_action(spec: &CommandSpec) -> bool {
    spec.journal
}

/// The commands listed under `tab`, in the engine's order.
pub(super) fn tab_specs(tab: &str) -> Vec<&'static CommandSpec> {
    command_specs().iter().filter(|s| is_action(s) && tab_of(s) == tab).collect()
}

/// The contextual tabs that apply now: the ribbon's own (a table, a chart), and the PivotTable tabs
/// while the sheet has a PivotTable.
pub(super) fn contextual_now(app: &SheetApp) -> Vec<&'static str> {
    let mut v = crate::ribbon::contextual_tabs(app);
    if app.session.active().and_then(|d| d.wb.active()).is_some_and(|sh| !sh.pivots.is_empty()) {
        v.extend(["PivotTable Analyze", "PivotTable Design"]);
    }
    v
}

/// The tabs to list now: every tab with commands, a contextual one only while it applies.
pub(super) fn available_tabs(app: &SheetApp) -> Vec<&'static str> {
    let ctx = contextual_now(app);
    TABS.iter().copied().filter(|t| (!CONTEXTUAL.contains(t) || ctx.contains(t)) && !tab_specs(t).is_empty()).collect()
}

// ---------------------------------------------------------------- reading a params doc

/// One parameter in a params doc: its name, whether it may be left out, and what the doc says of it.
#[derive(Debug, PartialEq)]
pub(super) struct Param {
    pub name: String,
    pub optional: bool,
    pub about: String,
}

/// Splits `s` at `sep` where it is not inside quotes or brackets.
fn split_top(s: &str, sep: char) -> Vec<String> {
    let mut out = vec![String::new()];
    let (mut depth, mut quote) = (0i32, false);
    for c in s.chars() {
        match c {
            '"' => quote = !quote,
            '{' | '[' | '(' if !quote => depth += 1,
            '}' | ']' | ')' if !quote => depth -= 1,
            _ => {}
        }
        if c == sep && depth == 0 && !quote {
            out.push(String::new());
        } else if let Some(last) = out.last_mut() {
            last.push(c);
        }
    }
    out
}

/// The name of a `name?: about` part, if it starts with one.
fn param_name(part: &str) -> Option<(&str, bool, &str)> {
    let (head, about) = part.split_once(':').map_or((part, ""), |(h, a)| (h, a));
    let head = head.trim();
    let optional = head.ends_with('?');
    let name = head.trim_end_matches('?');
    (!name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')).then_some((name, optional, about.trim()))
}

/// The parameters of a params doc such as `{range?, preset: bottom|top, color?}`. Where a part offers
/// alternative names (`height: 20 | points: 15`) the first is taken. Text after `→` is the answer.
pub(super) fn params(doc: &str) -> Vec<Param> {
    let doc = doc.split('→').next().unwrap_or("").trim();
    let inner = doc.strip_prefix('{').and_then(|d| d.strip_suffix('}')).unwrap_or("");
    let mut out = Vec::new();
    for part in split_top(inner, ',') {
        // `a: x | b: y` names alternatives; `a: x|y` or `a: "x" | "y"` lists values for one name.
        let first = split_top(&part, '|').into_iter().fold(String::new(), |acc, seg| {
            if !acc.is_empty() && param_name(&seg).is_some_and(|(_, _, about)| !about.is_empty()) && seg.contains(':') {
                return acc;
            }
            if acc.is_empty() { seg } else { format!("{acc}|{seg}") }
        });
        if let Some((name, optional, about)) = param_name(&first) {
            out.push(Param { name: name.to_string(), optional, about: about.to_string() });
        }
    }
    out
}

/// The parameters a command cannot run without.
pub(super) fn required(doc: &str) -> Vec<Param> {
    params(doc).into_iter().filter(|p| !p.optional).collect()
}

/// Options spelled out in a doc: `a|b|c` when every one is a plain word and the list is not cut short.
pub(super) fn spelled_out(about: &str) -> Option<Vec<String>> {
    if about.contains('…') || about.contains("...") {
        return None;
    }
    let first = about.split([' ', ',', '}', '(']).next().unwrap_or("");
    let parts: Vec<&str> = first.split('|').collect();
    let plain = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    (parts.len() >= 2 && parts.iter().all(|p| plain(p))).then(|| parts.into_iter().map(str::to_string).collect())
}

/// Words made readable: `thickOutside` becomes "Thick outside".
pub(super) fn readable(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c == '-' || c == '_' {
            out.push(' ');
        } else if c.is_uppercase() && i > 0 {
            out.push(' ');
            out.extend(c.to_lowercase());
        } else if i == 0 {
            out.extend(c.to_uppercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// A typed value as JSON: a number, true/false, a list or object when it reads as one, else text.
pub(super) fn parse_value(text: &str) -> Value {
    let t = text.trim();
    serde_json::from_str::<Value>(t).unwrap_or_else(|_| Value::String(t.to_string()))
}

/// The options the phone offers for `param` of command `id`: the lists the sheets use, else values
/// spelled out in the doc.
pub(super) fn choices(id: &str, param: &str, about: &str) -> Option<Vec<(Value, String)>> {
    let list: Vec<(Value, String)> = match (id, param) {
        ("home.fontName", "name") => crate::ribbon::FONTS.iter().map(|f| (json!({"name": f}), f.to_string())).collect(),
        ("home.fontSize", "size") => super::FONT_SIZES.iter().map(|s| (json!({"size": s}), format!("{s}"))).collect(),
        ("home.numberFormat", "format") => super::NUMBER_FORMATS.iter().map(|f| (json!({"format": f}), f.to_string())).collect(),
        ("view.zoom", "percent") => [50, 75, 90, 100, 125, 150, 200].iter().map(|p| (json!({"percent": p}), format!("{p}%"))).collect(),
        _ => spelled_out(about)?.into_iter().map(|v| (json!({ param: v.clone() }), readable(&v))).collect(),
    };
    (!list.is_empty()).then_some(list)
}

/// What tapping a command does.
enum Plan {
    /// Run it as the ribbon does (its dialog opens when it needs values).
    Run,
    /// Choose its one value from a list.
    Pick(Pick),
    /// Type its values, if running it as the ribbon does fails.
    Form(Form),
}

fn plan(spec: &'static CommandSpec) -> Plan {
    let req = required(spec.params);
    if req.is_empty() {
        return Plan::Run;
    }
    if let [one] = req.as_slice()
        && let Some(options) = choices(spec.id, &one.name, &one.about)
    {
        return Plan::Pick(Pick { id: spec.id, options });
    }
    Plan::Form(Form { id: spec.id, fields: req.into_iter().map(|p| (p.name, p.about, String::new())).collect() })
}

/// Runs a command picked or typed in the tools, then closes them. An error is shown as the ribbon
/// shows one, and the tools stay open so the values can be changed.
fn run_with(app: &mut SheetApp, ctx: &Context, id: &str, params: Value) {
    match app.run(id, params) {
        Ok(_) => close_all(ctx),
        Err(e) => app.message = Some(("GridCraft".into(), e)),
    }
}

/// What a tap on a command's row does.
fn tap(app: &mut SheetApp, ctx: &Context, spec: &'static CommandSpec) {
    launch(app, ctx, spec, None);
}

/// The dialog that asks for a command's values, and what it opens with: the ribbon's, where its
/// name differs from the command's; else a dialog named as the command (`home.rowHeight` →
/// `rowHeight`), if there is one.
fn dialog_for(id: &str) -> (&str, Value) {
    match id {
        "insert.link" => ("insertLink", json!({})),
        "data.validation" => ("dataValidation", json!({})),
        "edit.replace" => ("find", json!({"replace": true})),
        "home.conditionalFormat" => ("cfQuick", json!({"type": "expression", "title": "New Formatting Rule"})),
        "insert.sparkline" => ("sparkline", json!({"type": "line"})),
        other => (other.rsplit('.').next().unwrap_or(other), json!({})),
    }
}

/// Opens the dialog for command `id`. True when one opened.
fn try_dialog(app: &mut SheetApp, id: &str) -> bool {
    let (name, params) = dialog_for(id);
    // "open" is the file picker, not a dialog: it is not tried here.
    if name == "open" || app.dialog.is_some() {
        return false;
    }
    app.open_dialog(name, params);
    app.dialog.is_some()
}

/// Runs a command from the tools or a tile. A command that needs no values runs as the ribbon runs
/// it. One that needs values opens, in this order: its dialog; a picker, when it needs one value
/// from a list; else it runs as the ribbon does; and only when that fails, a form. A picker or form
/// opened from a sheet (`from`) returns to that sheet on Back; else it is pushed on the tools.
fn launch(app: &mut SheetApp, ctx: &Context, spec: &'static CommandSpec, from: Option<Option<usize>>) {
    let next = match plan(spec) {
        Plan::Run => {
            close_all(ctx);
            app.run_or_alert(spec.id, json!({}));
            return;
        }
        _ if try_dialog(app, spec.id) => {
            close_all(ctx);
            return;
        }
        Plan::Pick(p) => Tool::Pick(p),
        Plan::Form(f) => {
            if app.run(spec.id, json!({})).is_ok() {
                close_all(ctx);
                return;
            }
            Tool::Form(f)
        }
    };
    match from {
        Some(sheet) => open_from(ctx, next, sheet),
        None => push(ctx, next),
    }
}

fn push(ctx: &Context, t: Tool) {
    let mut s = stack(ctx);
    s.push(t);
    set_stack(ctx, s);
}

/// The title of a screen, before translation.
pub(super) fn title(tool: &Tool) -> String {
    match tool {
        Tool::Tabs => "All tools".into(),
        Tool::Search(_) => "Search".into(),
        Tool::Tab(t) => tab_label(t).to_string(),
        Tool::Pick(p) => find_command(p.id).map_or(p.id, |s| s.label).to_string(),
        Tool::Form(f) => find_command(f.id).map_or(f.id, |s| s.label).to_string(),
    }
}

// ---------------------------------------------------------------- drawing

/// A list row: the label, an optional second line, and a chevron. A disabled row is greyed and inert.
fn row(ui: &mut Ui, t: Tokens, label: &str, sub: Option<&str>, chevron: bool, enabled: bool) -> bool {
    let h = if sub.is_some() { ROW_H + 10.0 } else { ROW_H };
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), sense);
    if enabled && resp.is_pointer_button_down_on() {
        ui.painter().rect_filled(r, 0.0, t.pressed);
    } else if enabled && resp.hovered() {
        ui.painter().rect_filled(r, 0.0, t.hover);
    }
    let ink = if enabled { t.text } else { t.text_disabled };
    let right = if chevron { 40.0 } else { 8.0 };
    let y = if sub.is_some() { r.center().y - 10.0 } else { r.center().y };
    super::paint_truncated(ui.painter(), r.left() + 4.0, y, (r.width() - 4.0 - right).max(0.0), label, theme::ui_font(15.0), ink);
    if let Some(s) = sub {
        super::paint_truncated(
            ui.painter(),
            r.left() + 4.0,
            r.center().y + 12.0,
            (r.width() - 4.0 - right).max(0.0),
            s,
            theme::ui_font(12.0),
            t.text_dim,
        );
    }
    if chevron {
        icons::paint(ui.painter(), Rect::from_center_size(Pos2::new(r.right() - 20.0, r.center().y), vec2(20.0, 20.0)), Icon::Right, ink);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, label));
    resp.clicked()
}

/// A command's row: its label, why it cannot run now (when it cannot), and a chevron when it asks
/// for a value. True when tapped.
fn command_row(app: &SheetApp, ui: &mut Ui, t: Tokens, spec: &'static CommandSpec) -> bool {
    let lang = app.ui.language;
    let reason = (spec.enabled)(&app.session).err();
    let asks = !required(spec.params).is_empty();
    row(ui, t, lang.tr(spec.label), reason.as_deref(), asks, reason.is_none())
}

/// The body of the tools' top screen.
pub(super) fn body(app: &mut SheetApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let t = Tokens::get(&ctx);
    let lang = app.ui.language;
    let mut s = stack(&ctx);
    let Some(top) = s.last_mut() else { return };
    let mut next: Option<Tool> = None;
    let mut tapped: Option<&'static CommandSpec> = None;
    let mut ran: Option<(&'static str, Value)> = None;
    match top {
        Tool::Tabs => {
            if row(ui, t, lang.tr("Search all commands"), None, true, true) {
                next = Some(Tool::Search(String::new()));
            }
            super::tiles::heading(ui, t, lang.tr("Ribbon tabs"));
            let contextual = contextual_now(app);
            let tabs = available_tabs(app);
            for tab in tabs.iter().filter(|t| !contextual.contains(t)) {
                let label = format!("{}  ({})", lang.tr(tab_label(tab)), tab_specs(tab).len());
                if row(ui, t, &label, None, true, true) {
                    next = Some(Tool::Tab(tab));
                }
            }
            if tabs.iter().any(|t| contextual.contains(t)) {
                super::tiles::heading(ui, t, lang.tr("Selection"));
                for tab in tabs.iter().filter(|t| contextual.contains(t)) {
                    let label = format!("{}  ({})", lang.tr(tab_label(tab)), tab_specs(tab).len());
                    if row(ui, t, &label, None, true, true) {
                        next = Some(Tool::Tab(tab));
                    }
                }
            }
        }
        Tool::Search(q) => {
            let edit = super::touch_edit(ui, egui::TextEdit::singleline(q).hint_text(lang.tr("Search commands")).desired_width(f32::INFINITY));
            let r = ui.add(edit);
            r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, lang.tr("Search commands")));
            // The field takes the focus once, as the Search screen opens.
            let once = egui::Id::unique("phone_tools_search_focused");
            if !ctx.data(|d| d.get_temp::<bool>(once).unwrap_or(false)) {
                r.request_focus();
                ctx.data_mut(|d| d.insert_temp(once, true));
            }
            let needle = q.trim().to_lowercase();
            if needle.is_empty() {
                super::tiles::heading(ui, t, lang.tr("Type a name, such as freeze, chart or border"));
            } else {
                let found = command_specs()
                    .iter()
                    .filter(|s| {
                        is_action(s)
                            && (lang.tr(s.label).to_lowercase().contains(&needle)
                                || s.label.to_lowercase().contains(&needle)
                                || s.id.to_lowercase().contains(&needle))
                    })
                    .take(80);
                for spec in found {
                    if command_row(app, ui, t, spec) {
                        tapped = Some(spec);
                    }
                }
            }
        }
        Tool::Tab(tab) => {
            let mut group: Option<&str> = None;
            for spec in tab_specs(tab) {
                let g = spec.menu.get(1).copied();
                if g != group {
                    group = g;
                    if let Some(name) = g {
                        super::tiles::heading(ui, t, lang.tr(name));
                    }
                }
                if command_row(app, ui, t, spec) {
                    tapped = Some(spec);
                }
            }
        }
        Tool::Pick(p) => {
            for (params, label) in &p.options {
                if row(ui, t, label, None, false, true) {
                    ran = Some((p.id, params.clone()));
                }
            }
        }
        Tool::Form(f) => {
            for (name, about, value) in &mut f.fields {
                ui.add_space(8.0);
                let l = ui.label(egui::RichText::new(readable(name)).font(theme::ui_font(13.0)).color(t.text_dim));
                ui.add(super::touch_edit(ui, egui::TextEdit::singleline(value).hint_text(about.as_str()).desired_width(f32::INFINITY)))
                    .labelled_by(l.id);
            }
            ui.add_space(16.0);
            let label = lang.tr(find_command(f.id).map_or(f.id, |s| s.label)).trim_end_matches('…').to_string();
            let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), TOUCH), Sense::click());
            let fill = if resp.is_pointer_button_down_on() { t.accent_dark } else { t.accent };
            ui.painter().rect_filled(r, TOUCH / 2.0, fill);
            ui.painter().text(r.center(), Align2::CENTER_CENTER, &label, theme::ui_bold(15.0), egui::Color32::WHITE);
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label.clone()));
            if resp.clicked() {
                let mut params = json!({});
                for (name, _, value) in &f.fields {
                    if !value.trim().is_empty() {
                        params[name.as_str()] = parse_value(value);
                    }
                }
                ran = Some((f.id, params));
            }
        }
    }
    ui.add_space(GUTTER);
    // Keep what was typed, then apply the tap: the tap may change the stack.
    set_stack(&ctx, s.clone());
    if let Some(n) = next {
        if matches!(n, Tool::Search(_)) {
            ctx.data_mut(|d| d.insert_temp(egui::Id::unique("phone_tools_search_focused"), false));
        }
        push(&ctx, n);
    } else if let Some(spec) = tapped {
        tap(app, &ctx, spec);
    } else if let Some((id, params)) = ran {
        run_with(app, &ctx, id, params);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_command_is_in_exactly_one_tab() {
        for spec in command_specs().iter().filter(|s| is_action(s)) {
            let n = TABS.iter().filter(|t| tab_specs(t).iter().any(|s| s.id == spec.id)).count();
            assert_eq!(n, 1, "{} is listed in {n} tabs", spec.id);
        }
    }

    #[test]
    fn params_docs_read_names_and_optional_parts() {
        let p = params("{range?, preset: bottom|top|left, style?: thin|thick…, color?}");
        assert_eq!(
            p.iter().map(|p| (p.name.as_str(), p.optional)).collect::<Vec<_>>(),
            vec![("range", true), ("preset", false), ("style", true), ("color", true)]
        );
        assert_eq!(p[1].about, "bottom|top|left");
        // Alternative names: the first is taken.
        let p = params("{height: 20 (pixels) | points: 15}");
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].name, "height");
        // Values listed with spaced bars stay one parameter's.
        let p = params("{range?, color: \"#FFFF00\" | \"none\" | {theme, tint}}");
        assert_eq!(p.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["range", "color"]);
        assert!(params("{}").is_empty());
        assert_eq!(required("{range?, size: 11} → {ok}").iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["size"]);
    }

    #[test]
    fn spelled_out_lists_need_every_word_and_no_ellipsis() {
        assert_eq!(spelled_out("portrait|landscape"), Some(vec!["portrait".into(), "landscape".into()]));
        assert_eq!(spelled_out("thin|thick…"), None);
        assert_eq!(spelled_out("\"#FFFF00\""), None);
        assert_eq!(spelled_out("11"), None);
    }

    #[test]
    fn typed_values_become_numbers_lists_or_words() {
        assert_eq!(parse_value("14"), json!(14));
        assert_eq!(parse_value(" true "), json!(true));
        assert_eq!(parse_value("[1,2]"), json!([1, 2]));
        assert_eq!(parse_value("Sheet 2"), json!("Sheet 2"));
    }

    #[test]
    fn readable_words() {
        assert_eq!(readable("thickOutside"), "Thick outside");
        assert_eq!(readable("top_bottom"), "Top bottom");
    }

    #[test]
    fn the_sheets_lists_are_offered_as_choices() {
        assert!(choices("home.fontSize", "size", "11").is_some_and(|c| c.iter().any(|(v, _)| v["size"] == 14)));
        assert!(choices("home.numberFormat", "format", "").is_some_and(|c| c.len() == super::super::NUMBER_FORMATS.len()));
        assert!(choices("some.command", "mode", "a|b").is_some_and(|c| c.len() == 2));
        assert!(choices("some.command", "text", "\"x\"").is_none());
    }
}
