use super::geo::Geo;
use super::{CellRef, Painter, Pos2, RangeRef, Rect, Sheet, Stroke, StrokeKind, Tokens, pos2, vec2};

pub fn filter_button_rect(geo: &Geo, sh: &Sheet, c: CellRef) -> Rect {
    let rect = geo.cell_rect(sh, c);
    // Keep the button within the same header slot that AutoFit reserves.
    let pad = 2.0 * geo.z;
    let s = (rect.height() - 4.0).clamp(10.0, 18.0).min((sh.filter_button_space(c) * geo.z - pad).max(0.0));
    Rect::from_min_size(pos2(rect.right() - s - pad, rect.bottom() - s - pad), vec2(s, s))
}

/// A Table column heading that shows the column's header name in place of its letters.
#[derive(Clone, Debug)]
pub struct TableHeading {
    pub col: u32,
    /// The table's header row, which the filter menu is keyed by.
    pub header_row: u32,
    pub name: String,
    /// The heading slot over the column (logical coordinates).
    pub rect: Rect,
    /// The filter button, when the table shows filter buttons.
    pub button: Option<Rect>,
    /// The column is filtered (the button shows the funnel).
    pub filtered: bool,
}

/// The Table whose header names replace the column letters: the one holding the active cell,
/// with its header row scrolled out of view and some of its rows still in view (Excel, #1423).
pub fn heading_table<'a>(geo: &Geo, sh: &'a Sheet, active: CellRef) -> Option<&'a gridcraft_engine::model::Table> {
    // Page Layout view keeps the letters.
    if geo.paper.is_some() || geo.header_h <= 0.0 {
        return None;
    }
    let tb = sh.tables.iter().find(|t| t.header_row && t.range.contains(active))?;
    let header = tb.range.start.row;
    // A header row in the frozen rows is always on screen.
    if header < geo.fr {
        return None;
    }
    let top = geo.cells.top() + geo.frozen_h;
    let bottom = |r: u32| geo.y(sh, r) + sh.row_height(r) * geo.z;
    (bottom(header) <= top + 0.5 && bottom(tb.range.end.row) > top + 0.5).then_some(tb)
}

/// The headings of `heading_table`'s visible columns.
pub fn table_headings(geo: &Geo, sh: &Sheet, active: CellRef) -> Vec<TableHeading> {
    let Some(tb) = heading_table(geo, sh, active) else { return Vec::new() };
    let af = sh.autofilter.as_ref().filter(|af| af.range == tb.range);
    let mut out = Vec::new();
    for col in tb.range.start.col..=tb.range.end.col {
        let w = sh.col_width(col) * geo.z;
        let off = col - tb.range.start.col;
        let Some(column) = tb.columns.get(off as usize) else { continue };
        if w <= 0.0 {
            continue;
        }
        let rect = Rect::from_min_size(pos2(geo.x(sh, col), geo.head_top()), vec2(w, geo.header_h));
        // Clear of the 4-point column resize edge, and never wider than the column.
        let s = (rect.height() - 6.0).clamp(10.0, 16.0).min(w - 7.0);
        let button = (tb.filter_button && s > 0.0).then(|| Rect::from_min_size(pos2(rect.right() - s - 5.0, rect.center().y - s / 2.0), vec2(s, s)));
        let filtered = tb.filter.iter().any(|(o, _)| *o == off) || af.is_some_and(|af| af.criteria.iter().any(|(o, _)| *o == off));
        out.push(TableHeading { col, header_row: tb.range.start.row, name: column.name.clone(), rect, button, filtered });
    }
    out
}

pub(super) fn paint_filter_button(p: &Painter, r: Rect, filtered: bool) {
    let t = Tokens::get(p.ctx());
    p.rect_filled(r, 2.0, t.sheet_control_bg);
    p.rect_stroke(r, 2.0, Stroke::new(1.0, t.sheet_control_border), StrokeKind::Inside);
    crate::icons::paint_on_sheet(
        p,
        r.shrink(1.5),
        if filtered { crate::icons::Icon::Filter } else { crate::icons::Icon::Chevron },
        t.sheet_control_text,
    );
}

/// The square a filter arrow takes presses in. On the desktop, the arrow alone. On the phone: 48 pt tall,
/// centred on the arrow, and across the right half of its header `cell` (at least the arrow plus 8 pt).
/// It never reaches the cell's centre, so a tap on the rest of the cell selects it.
pub(super) fn filter_hit(cell: Rect, r: Rect, phone: bool) -> Rect {
    if !phone {
        return r;
    }
    let w = (r.width() + 8.0).max(cell.width() / 2.0);
    let left = (cell.right() - w).max(cell.center().x + 1.0);
    Rect::from_min_max(pos2(left, r.center().y - 24.0), pos2(cell.right(), r.center().y + 24.0))
}

/// The header cells whose filter arrows are at `p`'s cell, or with `around`, also the cells next to it
/// (a handle's square reaches past its own cell), with each arrow's hit square.
fn filter_arrow_cells(geo: &Geo, sh: &Sheet, p: Pos2, phone: bool, around: bool) -> Vec<(CellRef, Rect)> {
    let c = geo.cell_at(sh, p);
    let mut cells = vec![c];
    if around {
        let near = |v: u32, d: i64| u32::try_from(i64::from(v) + d).ok();
        cells = (-1..=1)
            .flat_map(|dc| (-1..=0).map(move |dr| (dc, dr)))
            .filter_map(|(dc, dr)| Some(CellRef::new(near(c.row, dr)?, near(c.col, dc)?)))
            .collect();
    }
    cells
        .into_iter()
        .filter(|&k| sh.has_filter_button(k))
        .map(|k| (k, filter_hit(geo.cell_rect(sh, k), filter_button_rect(geo, sh, k), phone)))
        .collect()
}

/// The header cell whose filter arrow `p` presses, if any. On the phone a tap counts only in the
/// arrow's own cell: the square spills vertically, and a neighbour's tap must not open this menu.
pub(super) fn filter_button_cell(geo: &Geo, sh: &Sheet, p: Pos2, phone: bool) -> Option<CellRef> {
    filter_arrow_cells(geo, sh, p, phone, false).into_iter().find(|(_, r)| r.contains(p)).map(|(k, _)| k)
}

/// The phone's filter arrows that a selection handle's touch square `hit` overlaps, with their hit
/// squares. The handle is drawn under them, so an arrow's tap goes to the arrow.
pub(crate) fn filter_arrows_under(geo: &Geo, sh: &Sheet, hit: Rect) -> Vec<(CellRef, Rect)> {
    filter_arrow_cells(geo, sh, hit.center(), true, true).into_iter().filter(|(_, r)| r.intersects(hit)).collect()
}

/// The table whose header cell (`row`, `col`) was clicked, not just the first table in the column.
pub(super) fn filter_table_range(sh: &Sheet, col: u32, row: u32) -> Option<RangeRef> {
    sh.tables.iter().find(|t| t.range.start.row == row && col >= t.range.start.col && col <= t.range.end.col).map(|t| t.range)
}
