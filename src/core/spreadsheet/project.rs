//! Projection of a workbook's sheets to logical `PageLayout` pages.
//!
//! One page per selected sheet. The page range is the print area when the sheet has one, else
//! the bounding box of its non-empty cells. Columns and rows inside the range get their points
//! from the stored widths and heights (hidden ones contribute nothing unless the caller asks for
//! hidden cells); a cell's text is placed in its rectangle by the style's alignment, with a
//! character-count width estimate, and split into words that carry the cell reference through
//! the shared line clusterer. Coordinates are y-up from the page's bottom-left, like every page.

use super::workbook::{
  Cell, CellRange, CellValue, FontSpec, Horizontal, Sheet, Styles, Vertical, Workbook, cell_ref,
  selected_sheets,
};
use crate::core::layout::interpreter::FontMeta;
use crate::core::layout::words::{Word, words_to_lines};
use crate::types::{PageLayout, SpreadsheetOptions, TextSource};

/// Maximum digit width of the Normal style's font (Calibri 11) in pixels at 96 dpi.
pub(crate) const MDW_PX: f64 = 7.0;
/// Row height when the sheet states none.
pub(crate) const DEFAULT_ROW_HEIGHT_PT: f64 = 15.0;
/// What Excel stores for a default column: `baseColWidth` 8 plus padding, 64 px.
pub(crate) const DEFAULT_COL_WIDTH: f64 = 9.140625;
/// Estimated advance of one character as a fraction of the font size.
pub(crate) const CHAR_EM: f64 = 0.5;
/// Height of one text fragment as a fraction of the font size.
const LINE_EM: f64 = 1.2;
const PT_PER_PX: f64 = 72.0 / 96.0;

/// A stored `<col width>` (characters of the maximum digit width) to points: the SpreadsheetML
/// pixel formula, then 72/96.
pub(crate) fn column_width_points(width: f64) -> f64 {
  let px = (((256.0 * width + (128.0 / MDW_PX).trunc()) / 256.0) * MDW_PX).trunc();
  px * PT_PER_PX
}

/// The logical pages of the selected sheets, numbered from 1 in selection order.
pub(crate) fn project(workbook: &Workbook, opts: &SpreadsheetOptions) -> Vec<PageLayout> {
  let hidden_cells = opts.hidden_cells.unwrap_or(false);
  selected_sheets(workbook, opts)
    .into_iter()
    .enumerate()
    .map(|(i, sheet)| project_sheet(sheet, &workbook.styles, i as u32 + 1, hidden_cells))
    .collect()
}

/// Cumulative offsets of `count` consecutive extents: `offsets[i]` is where item `i` starts and
/// `offsets[count]` the total.
fn offsets(count: usize, extent: impl Fn(usize) -> f64) -> Vec<f64> {
  let mut out = Vec::with_capacity(count + 1);
  let mut total = 0.0;
  out.push(total);
  for i in 0..count {
    total += extent(i);
    out.push(total);
  }
  out
}

fn column_points(sheet: &Sheet, col: u32, hidden_cells: bool) -> f64 {
  if sheet.col_hidden(col) && !hidden_cells {
    return 0.0;
  }
  let stored = sheet
    .cols
    .iter()
    .find(|c| col + 1 >= c.min && col < c.max && c.width > 0.0)
    .map(|c| c.width)
    .or(sheet.default_col_width)
    .filter(|w| *w > 0.0)
    .unwrap_or(DEFAULT_COL_WIDTH);
  column_width_points(stored)
}

fn row_points(sheet: &Sheet, row: u32, hidden_cells: bool) -> f64 {
  if sheet.row_hidden(row) && !hidden_cells {
    return 0.0;
  }
  let default = if sheet.default_row_height > 0.0 {
    sheet.default_row_height
  } else {
    DEFAULT_ROW_HEIGHT_PT
  };
  sheet
    .rows
    .get(&row)
    .and_then(|r| r.height)
    .filter(|h| *h > 0.0)
    .unwrap_or(default)
}

/// The print area, else the bounding box of the sheet's non-empty cells.
fn page_range(sheet: &Sheet) -> Option<CellRange> {
  if let Some(area) = sheet.print_area {
    return Some(area);
  }
  let mut range: Option<CellRange> = None;
  for &(row, col) in sheet.cells.keys() {
    range = Some(match range {
      None => CellRange {
        r0: row,
        c0: col,
        r1: row,
        c1: col,
      },
      Some(r) => CellRange {
        r0: r.r0.min(row),
        c0: r.c0.min(col),
        r1: r.r1.max(row),
        c1: r.c1.max(col),
      },
    });
  }
  range
}

fn font_meta(font: &FontSpec) -> FontMeta {
  FontMeta {
    name: font.name.clone(),
    bold: font.bold,
    italic: font.italic,
    symbolic: false,
    weight_known: true,
  }
}

/// Greedy word wrap at `max_chars` characters; a single word longer than the width stands alone.
fn wrap(text: &str, max_chars: usize) -> Vec<String> {
  let mut lines: Vec<String> = Vec::new();
  let mut current = String::new();
  let mut current_len = 0usize;
  for word in text.split_whitespace() {
    let len = word.chars().count();
    if current.is_empty() {
      current.push_str(word);
      current_len = len;
    } else if current_len + 1 + len <= max_chars {
      current.push(' ');
      current.push_str(word);
      current_len += 1 + len;
    } else {
      lines.push(std::mem::take(&mut current));
      current.push_str(word);
      current_len = len;
    }
  }
  if !current.is_empty() || lines.is_empty() {
    lines.push(current);
  }
  lines
}

/// Runs of non-whitespace characters as `(start, end)` character offsets.
fn word_spans(text: &str) -> Vec<(usize, usize, String)> {
  let mut spans = Vec::new();
  let mut start: Option<usize> = None;
  let mut current = String::new();
  for (i, c) in text.chars().enumerate() {
    if c.is_whitespace() {
      if let Some(s) = start.take() {
        spans.push((s, i, std::mem::take(&mut current)));
      }
    } else {
      if start.is_none() {
        start = Some(i);
      }
      current.push(c);
    }
  }
  if let Some(s) = start {
    spans.push((s, text.chars().count(), current));
  }
  spans
}

struct PageFrame {
  range: CellRange,
  col_x: Vec<f64>,
  row_y: Vec<f64>,
  width: f64,
  height: f64,
}

impl PageFrame {
  fn new(sheet: &Sheet, range: CellRange, hidden_cells: bool) -> PageFrame {
    let col_x = offsets((range.c1 - range.c0 + 1) as usize, |i| {
      column_points(sheet, range.c0 + i as u32, hidden_cells)
    });
    let row_y = offsets((range.r1 - range.r0 + 1) as usize, |i| {
      row_points(sheet, range.r0 + i as u32, hidden_cells)
    });
    PageFrame {
      range,
      width: *col_x.last().unwrap_or(&0.0),
      height: *row_y.last().unwrap_or(&0.0),
      col_x,
      row_y,
    }
  }

  fn contains(&self, row: u32, col: u32) -> bool {
    (self.range.r0..=self.range.r1).contains(&row) && (self.range.c0..=self.range.c1).contains(&col)
  }

  /// The rectangle of a cell, spanning its merged range clipped to the page: `(x0, y0, x1, y1)`.
  fn rect(&self, row: u32, col: u32, merge: Option<CellRange>) -> (f64, f64, f64, f64) {
    let (r1, c1) = match merge {
      Some(m) => (
        m.r1.min(self.range.r1).max(row),
        m.c1.min(self.range.c1).max(col),
      ),
      None => (row, col),
    };
    let x0 = self.col_x[(col - self.range.c0) as usize];
    let x1 = self.col_x[(c1 - self.range.c0 + 1) as usize];
    let y1 = self.height - self.row_y[(row - self.range.r0) as usize];
    let y0 = self.height - self.row_y[(r1 - self.range.r0 + 1) as usize];
    (x0, y0, x1, y1)
  }
}

struct Placement<'a> {
  cell: &'a Cell,
  rect: (f64, f64, f64, f64),
  font: &'a FontSpec,
  font_index: usize,
  horizontal: Horizontal,
  vertical: Vertical,
  wrap: bool,
  indent: f64,
}

/// The words of one cell, placed; `true` when wrapping broke a fragment.
fn place_cell(
  p: &Placement<'_>,
  frame: &PageFrame,
  cell_index: usize,
  out: &mut Vec<Word>,
) -> bool {
  let size = if p.font.size > 0.0 { p.font.size } else { 11.0 };
  let cw = CHAR_EM * size;
  let fh = LINE_EM * size;
  let (x0, y0, x1, y1) = p.rect;
  let avail = (x1 - x0 - p.indent).max(cw);
  let mut wrapped = false;
  let mut fragments: Vec<String> = Vec::new();
  for part in p.cell.text.split('\n') {
    let part = part.trim_end_matches('\r');
    if p.wrap {
      let lines = wrap(part, ((avail / cw).floor() as usize).max(1));
      wrapped |= lines.len() > 1;
      fragments.extend(lines);
    } else {
      fragments.push(part.to_string());
    }
  }
  let block = fh * fragments.len() as f64;
  let top = match p.vertical {
    Vertical::Top => y1,
    Vertical::Center => y1 - ((y1 - y0) - block) / 2.0,
    Vertical::Bottom => y0 + block,
  };
  let general_right = matches!(p.cell.value, CellValue::Number(_) | CellValue::Date(_));
  let general_center = matches!(p.cell.value, CellValue::Boolean(_) | CellValue::Error(_));
  for (i, fragment) in fragments.iter().enumerate() {
    let fy1 = top - fh * i as f64;
    let fy0 = fy1 - fh;
    let fw = fragment.chars().count() as f64 * cw;
    let fx0 = match p.horizontal {
      Horizontal::Left => x0 + p.indent,
      Horizontal::Right => x1 - p.indent - fw,
      Horizontal::Center => x0 + ((x1 - x0) - fw) / 2.0,
      Horizontal::General if general_right => x1 - fw,
      Horizontal::General if general_center => x0 + ((x1 - x0) - fw) / 2.0,
      Horizontal::General => x0,
    };
    for (start, end, text) in word_spans(fragment) {
      // Unwrapped text overflows its cell; everything is clipped to the page.
      let wx0 = (fx0 + start as f64 * cw).clamp(0.0, frame.width);
      let wx1 = (fx0 + end as f64 * cw).clamp(0.0, frame.width);
      let wy0 = fy0.clamp(0.0, frame.height);
      let wy1 = fy1.clamp(0.0, frame.height);
      if wx1 <= wx0 || wy1 <= wy0 {
        continue;
      }
      out.push(Word {
        text,
        x0: wx0,
        y0: wy0,
        x1: wx1,
        y1: wy1,
        baseline: wy0,
        font_size: Some(size),
        font: Some(p.font_index),
        confidence: None,
        cell: Some(cell_index),
      });
    }
  }
  wrapped
}

fn project_sheet(sheet: &Sheet, styles: &Styles, page: u32, hidden_cells: bool) -> PageLayout {
  let mut warnings = sheet.warnings.clone();
  let mut fonts: Vec<FontMeta> = styles.fonts.iter().map(font_meta).collect();
  fonts.push(font_meta(&styles.normal_font));
  let normal_index = styles.fonts.len();

  let Some(range) = page_range(sheet) else {
    warnings.push("the sheet has no content".to_string());
    return finish(page, sheet, 0.0, 0.0, Vec::new(), &fonts, &[], warnings);
  };
  let frame = PageFrame::new(sheet, range, hidden_cells);

  let mut words: Vec<Word> = Vec::new();
  let mut refs: Vec<String> = Vec::new();
  let mut off_page = 0usize;
  let mut omitted = 0usize;
  let mut wrapped = 0usize;
  for (&(row, col), cell) in &sheet.cells {
    if !frame.contains(row, col) {
      off_page += 1;
      continue;
    }
    if (sheet.row_hidden(row) || sheet.col_hidden(col)) && !hidden_cells {
      omitted += 1;
      continue;
    }
    if cell.text.trim().is_empty() {
      continue;
    }
    let xf = styles.xfs.get(cell.style);
    let (font, font_index) = xf
      .and_then(|xf| styles.fonts.get(xf.font_id).map(|f| (f, xf.font_id)))
      .unwrap_or((&styles.normal_font, normal_index));
    let placement = Placement {
      cell,
      rect: frame.rect(row, col, cell.merge),
      font,
      font_index,
      horizontal: xf.map(|x| x.horizontal).unwrap_or_default(),
      vertical: xf.map(|x| x.vertical).unwrap_or_default(),
      wrap: xf.is_some_and(|x| x.wrap),
      indent: xf.map(|x| x.indent as f64 * font.size).unwrap_or(0.0),
    };
    refs.push(cell_ref(row, col));
    if place_cell(&placement, &frame, refs.len() - 1, &mut words) {
      wrapped += 1;
    }
  }

  if let Some(area) = sheet.print_area {
    let bloated = sheet
      .dimension
      .is_some_and(|d| d.r0 < area.r0 || d.c0 < area.c0 || d.r1 > area.r1 || d.c1 > area.c1);
    if off_page > 0 || bloated {
      let dimension = sheet
        .dimension
        .map(|d| d.a1())
        .unwrap_or_else(|| "unknown".to_string());
      warnings.push(format!(
        "print area {} is smaller than the sheet dimension {dimension}; {off_page} cell(s) outside it are off the page",
        area.a1()
      ));
    }
  }
  if omitted > 0 {
    warnings.push(format!(
      "{omitted} cell(s) in hidden rows or columns are omitted; pass hiddenCells to include them"
    ));
  }
  if wrapped > 0 {
    warnings.push(format!("{wrapped} wrapped cell(s) use estimated widths"));
  }
  if sheet.drawings > 0 {
    warnings.push(format!("{} drawing(s) are not projected", sheet.drawings));
  }
  finish(
    page,
    sheet,
    frame.width,
    frame.height,
    words,
    &fonts,
    &refs,
    warnings,
  )
}

#[allow(clippy::too_many_arguments)]
fn finish(
  page: u32,
  sheet: &Sheet,
  width: f64,
  height: f64,
  words: Vec<Word>,
  fonts: &[FontMeta],
  refs: &[String],
  warnings: Vec<String>,
) -> PageLayout {
  let (words, lines) = words_to_lines(words, fonts, refs);
  PageLayout {
    page,
    width,
    height,
    rotate: 0,
    content_rotation: 0,
    source: TextSource::Spreadsheet,
    sheet: Some(sheet.name.clone()),
    words,
    lines,
    images: Vec::new(),
    warnings,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn column_widths_follow_the_spreadsheetml_pixel_formula() {
    assert_eq!(column_width_points(9.140625), 48.0);
    assert_eq!(column_width_points(20.0), 105.0);
    assert_eq!(column_width_points(12.0), 63.0);
    assert_eq!(column_width_points(14.0), 73.5);
  }
}
