//! Projection of a workbook's sheets to logical `PageLayout` pages, and the sheet geometry that
//! `spreadsheetCells` reports.
//!
//! One page per selected sheet. The page range is the print area when the sheet has one, else
//! the bounding box of its non-empty cells and merged ranges; the sheet frame that cell boxes use
//! runs from A1 to that same last cell. Columns and rows get their points from the stored widths
//! and heights (hidden ones contribute nothing unless the caller asks for hidden cells); a row
//! grows to fit text taller than it, as Excel's auto-fit does on open. A cell's text is placed in
//! its rectangle by the style's alignment, with a character-count width estimate, and split into
//! words that carry the cell reference through the shared line clusterer. Coordinates are y-up
//! from the frame's bottom-left, like every page.

use std::collections::{BTreeSet, HashMap};

use super::workbook::{
  Cell, CellRange, CellValue, FontSpec, Horizontal, Sheet, Styles, Vertical, Workbook, cell_ref,
  selected_sheets,
};
use crate::core::layout::interpreter::FontMeta;
use crate::core::layout::words::{Word, words_to_lines};
use crate::types::{CellBox, PageLayout, SheetColumn, SheetRow, SpreadsheetOptions, TextSource};

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

/// The sheet frame's extents and every cell's rectangle, for `spreadsheetCells`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SheetGeometry {
  pub width: f64,
  pub height: f64,
  pub default_column_width: f64,
  pub default_row_height: f64,
  pub columns: Vec<SheetColumn>,
  pub rows: Vec<SheetRow>,
  pub boxes: HashMap<(u32, u32), CellBox>,
}

/// The sheet frame: A1 to the last non-empty cell or merged range, in the same y-up points as
/// the pages, with rows grown to fit their text. Hidden rows and columns keep their extents when
/// `hidden_cells` is set and have none otherwise.
pub(crate) fn sheet_geometry(sheet: &Sheet, styles: &Styles, hidden_cells: bool) -> SheetGeometry {
  let default_column_width = column_width_points(default_col_width(sheet));
  let default_row_height = default_row_height(sheet);
  let Some(range) = sheet_range(sheet) else {
    return SheetGeometry {
      width: 0.0,
      height: 0.0,
      default_column_width,
      default_row_height,
      columns: Vec::new(),
      rows: Vec::new(),
      boxes: HashMap::new(),
    };
  };
  let built = build(sheet, styles, range, hidden_cells);
  let columns = built
    .widths
    .iter()
    .enumerate()
    .filter_map(|(i, &width)| {
      let index = range.c0 + i as u32;
      let hidden = sheet.col_hidden(index);
      ((width - default_column_width).abs() > 1e-9 || hidden).then_some(SheetColumn {
        index,
        width,
        hidden,
      })
    })
    .collect();
  let rows = built
    .heights
    .iter()
    .enumerate()
    .filter_map(|(i, &height)| {
      let index = range.r0 + i as u32;
      let hidden = sheet.row_hidden(index);
      ((height - default_row_height).abs() > 1e-9 || hidden).then_some(SheetRow {
        index,
        height,
        hidden,
      })
    })
    .collect();
  let boxes = sheet
    .cells
    .iter()
    .map(|(&(row, col), cell)| {
      let (r1, c1) = built.frame.span_end(row, col, cell.merge);
      let (x0, y0, x1, y1) = built.frame.rect(row, col, r1, c1);
      ((row, col), CellBox { x0, y0, x1, y1 })
    })
    .collect();
  SheetGeometry {
    width: built.frame.width,
    height: built.frame.height,
    default_column_width,
    default_row_height,
    columns,
    rows,
    boxes,
  }
}

/// Cumulative offsets of consecutive extents: `offsets[i]` is where item `i` starts and the last
/// entry the total.
fn offsets(extents: &[f64]) -> Vec<f64> {
  let mut out = Vec::with_capacity(extents.len() + 1);
  let mut total = 0.0;
  out.push(total);
  for extent in extents {
    total += extent;
    out.push(total);
  }
  out
}

fn default_col_width(sheet: &Sheet) -> f64 {
  sheet
    .default_col_width
    .filter(|w| *w > 0.0)
    .unwrap_or(DEFAULT_COL_WIDTH)
}

fn default_row_height(sheet: &Sheet) -> f64 {
  if sheet.default_row_height > 0.0 {
    sheet.default_row_height
  } else {
    DEFAULT_ROW_HEIGHT_PT
  }
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
    .unwrap_or_else(|| default_col_width(sheet));
  column_width_points(stored)
}

fn row_points(sheet: &Sheet, row: u32, hidden_cells: bool) -> f64 {
  if sheet.row_hidden(row) && !hidden_cells {
    return 0.0;
  }
  sheet
    .rows
    .get(&row)
    .and_then(|r| r.height)
    .filter(|h| *h > 0.0)
    .unwrap_or_else(|| default_row_height(sheet))
}

fn extend(range: &mut Option<CellRange>, r0: u32, c0: u32, r1: u32, c1: u32) {
  *range = Some(match *range {
    None => CellRange { r0, c0, r1, c1 },
    Some(r) => CellRange {
      r0: r.r0.min(r0),
      c0: r.c0.min(c0),
      r1: r.r1.max(r1),
      c1: r.c1.max(c1),
    },
  });
}

/// The bounding box of the non-empty cells and the ranges they anchor.
fn content_range(sheet: &Sheet) -> Option<CellRange> {
  let mut range = None;
  for (&(row, col), cell) in &sheet.cells {
    extend(&mut range, row, col, row, col);
    if let Some(m) = cell.merge {
      extend(&mut range, m.r0, m.c0, m.r1, m.c1);
    }
  }
  range
}

/// The sheet frame: A1 to the last non-empty cell or merged range.
fn sheet_range(sheet: &Sheet) -> Option<CellRange> {
  content_range(sheet).map(|r| CellRange {
    r0: 0,
    c0: 0,
    r1: r.r1,
    c1: r.c1,
  })
}

/// The print area, else the content range.
fn page_range(sheet: &Sheet) -> Option<CellRange> {
  sheet.print_area.or_else(|| content_range(sheet))
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
  fn contains(&self, row: u32, col: u32) -> bool {
    (self.range.r0..=self.range.r1).contains(&row) && (self.range.c0..=self.range.c1).contains(&col)
  }

  /// The last row and column a cell covers: its merged range clipped to the frame.
  fn span_end(&self, row: u32, col: u32, merge: Option<CellRange>) -> (u32, u32) {
    match merge {
      Some(m) => (
        m.r1.min(self.range.r1).max(row),
        m.c1.min(self.range.c1).max(col),
      ),
      None => (row, col),
    }
  }

  /// The rectangle from `(row, col)` through `(r1, c1)`: `(x0, y0, x1, y1)`.
  fn rect(&self, row: u32, col: u32, r1: u32, c1: u32) -> (f64, f64, f64, f64) {
    let x0 = self.col_x[(col - self.range.c0) as usize];
    let x1 = self.col_x[(c1 - self.range.c0 + 1) as usize];
    let y1 = self.height - self.row_y[(row - self.range.r0) as usize];
    let y0 = self.height - self.row_y[(r1 - self.range.r0 + 1) as usize];
    (x0, y0, x1, y1)
  }
}

/// A non-empty, visible cell inside the frame with its text already broken into fragments.
struct Prepared<'a> {
  row: u32,
  col: u32,
  r1: u32,
  c1: u32,
  cell: &'a Cell,
  font_index: usize,
  size: f64,
  horizontal: Horizontal,
  vertical: Vertical,
  indent: f64,
  fragments: Vec<String>,
}

/// A frame with its cells prepared: widths, heights after auto-fit, and the counts the page warns about.
struct Built<'a> {
  frame: PageFrame,
  widths: Vec<f64>,
  heights: Vec<f64>,
  prepared: Vec<Prepared<'a>>,
  omitted: usize,
  wrapped: usize,
  grown_rows: usize,
}

fn build<'a>(
  sheet: &'a Sheet,
  styles: &'a Styles,
  range: CellRange,
  hidden_cells: bool,
) -> Built<'a> {
  let widths: Vec<f64> = (range.c0..=range.c1)
    .map(|col| column_points(sheet, col, hidden_cells))
    .collect();
  let col_x = offsets(&widths);
  let normal_index = styles.fonts.len();
  let mut prepared: Vec<Prepared<'a>> = Vec::new();
  let mut omitted = 0usize;
  let mut wrapped = 0usize;
  for (&(row, col), cell) in &sheet.cells {
    if !(range.r0..=range.r1).contains(&row) || !(range.c0..=range.c1).contains(&col) {
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
    let size = if font.size > 0.0 { font.size } else { 11.0 };
    let indent = xf.map(|x| x.indent as f64 * size).unwrap_or(0.0);
    let (r1, c1) = match cell.merge {
      Some(m) => (m.r1.min(range.r1).max(row), m.c1.min(range.c1).max(col)),
      None => (row, col),
    };
    let x0 = col_x[(col - range.c0) as usize];
    let x1 = col_x[(c1 - range.c0 + 1) as usize];
    let cw = CHAR_EM * size;
    let avail = (x1 - x0 - indent).max(cw);
    let wrap_on = xf.is_some_and(|x| x.wrap);
    let mut fragments: Vec<String> = Vec::new();
    let mut did_wrap = false;
    for part in cell.text.split('\n') {
      let part = part.trim_end_matches('\r');
      if wrap_on {
        let lines = wrap(part, ((avail / cw).floor() as usize).max(1));
        did_wrap |= lines.len() > 1;
        fragments.extend(lines);
      } else {
        fragments.push(part.to_string());
      }
    }
    wrapped += usize::from(did_wrap);
    prepared.push(Prepared {
      row,
      col,
      r1,
      c1,
      cell,
      font_index,
      size,
      horizontal: xf.map(|x| x.horizontal).unwrap_or_default(),
      vertical: xf.map(|x| x.vertical).unwrap_or_default(),
      indent,
      fragments,
    });
  }

  // Auto-fit: a row grows to hold the tallest text block anchored in it, the way Excel sizes a
  // row it opens. A block over a merged range grows the tallest of its rows.
  let mut heights: Vec<f64> = (range.r0..=range.r1)
    .map(|row| row_points(sheet, row, hidden_cells))
    .collect();
  let mut grown: BTreeSet<usize> = BTreeSet::new();
  for p in &prepared {
    let block = LINE_EM * p.size * p.fragments.len() as f64;
    let first = (p.row - range.r0) as usize;
    let last = (p.r1 - range.r0) as usize;
    let have: f64 = heights[first..=last].iter().sum();
    if block > have + 1e-9 {
      let target = (first..=last)
        .rev()
        .max_by(|&a, &b| heights[a].total_cmp(&heights[b]))
        .unwrap_or(last);
      heights[target] += block - have;
      grown.insert(target);
    }
  }
  let row_y = offsets(&heights);
  let frame = PageFrame {
    range,
    width: *col_x.last().unwrap_or(&0.0),
    height: *row_y.last().unwrap_or(&0.0),
    col_x,
    row_y,
  };
  Built {
    frame,
    widths,
    heights,
    prepared,
    omitted,
    wrapped,
    grown_rows: grown.len(),
  }
}

/// The words of one prepared cell, placed in its rectangle.
fn place_cell(p: &Prepared<'_>, frame: &PageFrame, cell_index: usize, out: &mut Vec<Word>) {
  let cw = CHAR_EM * p.size;
  let fh = LINE_EM * p.size;
  let (x0, y0, x1, y1) = frame.rect(p.row, p.col, p.r1, p.c1);
  let block = fh * p.fragments.len() as f64;
  let top = match p.vertical {
    Vertical::Top => y1,
    Vertical::Center => y1 - ((y1 - y0) - block) / 2.0,
    Vertical::Bottom => y0 + block,
  }
  .min(y1);
  let general_right = matches!(
    p.cell.value,
    CellValue::Number(_) | CellValue::Date(_) | CellValue::Time(_)
  );
  let general_center = matches!(p.cell.value, CellValue::Boolean(_) | CellValue::Error(_));
  for (i, fragment) in p.fragments.iter().enumerate() {
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
      // Unwrapped text overflows its cell sideways; everything is clipped to the frame.
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
        font_size: Some(p.size),
        font: Some(p.font_index),
        confidence: None,
        cell: Some(cell_index),
      });
    }
  }
}

fn project_sheet(sheet: &Sheet, styles: &Styles, page: u32, hidden_cells: bool) -> PageLayout {
  let mut warnings = sheet.warnings.clone();
  let mut fonts: Vec<FontMeta> = styles.fonts.iter().map(font_meta).collect();
  fonts.push(font_meta(&styles.normal_font));

  let Some(range) = page_range(sheet) else {
    warnings.push("the sheet has no content".to_string());
    return finish(page, sheet, 0.0, 0.0, Vec::new(), &fonts, &[], warnings);
  };
  let built = build(sheet, styles, range, hidden_cells);
  let off_page = sheet
    .cells
    .keys()
    .filter(|&&(row, col)| !built.frame.contains(row, col))
    .count();

  let mut words: Vec<Word> = Vec::new();
  let mut refs: Vec<String> = Vec::with_capacity(built.prepared.len());
  for p in &built.prepared {
    refs.push(cell_ref(p.row, p.col));
    place_cell(p, &built.frame, refs.len() - 1, &mut words);
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
  if built.omitted > 0 {
    warnings.push(format!(
      "{} cell(s) in hidden rows or columns are omitted; pass hiddenCells to include them",
      built.omitted
    ));
  }
  if built.wrapped > 0 {
    warnings.push(format!(
      "{} wrapped cell(s) use estimated widths",
      built.wrapped
    ));
  }
  if built.grown_rows > 0 {
    warnings.push(format!(
      "{} row(s) grown to fit their text (auto-fit)",
      built.grown_rows
    ));
  }
  if sheet.drawings > 0 {
    warnings.push(format!("{} drawing(s) are not projected", sheet.drawings));
  }
  finish(
    page,
    sheet,
    built.frame.width,
    built.frame.height,
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
  use super::super::workbook::{WorkbookContext, parse_sheet};
  use super::*;
  use crate::types::{CellBox, SheetColumn, SheetRow};

  fn sheet_of(xml: &[u8], styles: &Styles) -> Sheet {
    parse_sheet(
      xml,
      "S",
      0,
      false,
      WorkbookContext {
        shared: &[],
        styles,
        date1904: false,
      },
      None,
    )
    .unwrap()
  }

  fn page_of(xml: &[u8]) -> PageLayout {
    let styles = Styles::default();
    project_sheet(&sheet_of(xml, &styles), &styles, 1, false)
  }

  #[test]
  fn column_widths_follow_the_spreadsheetml_pixel_formula() {
    assert_eq!(column_width_points(9.140625), 48.0);
    assert_eq!(column_width_points(20.0), 105.0);
    assert_eq!(column_width_points(12.0), 63.0);
    assert_eq!(column_width_points(14.0), 73.5);
  }

  #[test]
  fn text_taller_than_its_row_grows_the_row_and_never_joins_the_row_above() {
    let xml = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Above</t></is></c><c r="B1" t="inlineStr"><is><t>Right</t></is></c></row><row r="2"><c r="A2" t="inlineStr"><is><t>Line one&#10;Line two</t></is></c><c r="B2" t="inlineStr"><is><t>Beside</t></is></c></row><row r="3"><c r="A3" t="inlineStr"><is><t>Below</t></is></c></row></sheetData></worksheet>"##;
    let page = page_of(xml);
    assert_eq!(
      page
        .lines
        .iter()
        .map(|l| l.text.as_str())
        .collect::<Vec<_>>(),
      ["Above Right", "Line one", "Line two Beside", "Below"]
    );
    // Row 2 grows from 15 pt to two Calibri 11 lines (2 × 13.2 pt), like Excel's auto-fit.
    assert!(
      (page.height - (15.0 + 26.4 + 15.0)).abs() < 1e-9,
      "{}",
      page.height
    );
    assert!(
      page.words.iter().any(|w| w.text == "two"),
      "no word is dropped"
    );
    assert!(
      page
        .words
        .iter()
        .all(|w| w.y0 >= 0.0 && w.y1 <= page.height)
    );
    assert!(
      page.warnings.iter().any(|w| w.contains("grown")),
      "{:?}",
      page.warnings
    );
  }

  #[test]
  fn a_reference_past_the_grid_does_not_grow_the_page() {
    let xml = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1"><v>1</v></c><c r="A999999999"><v>2</v></c></row></sheetData></worksheet>"##;
    let page = page_of(xml);
    assert_eq!((page.width, page.height), (48.0, 15.0));
    assert_eq!(page.words.len(), 1);
    assert!(
      page.warnings.iter().any(|w| w.contains("malformed")),
      "{:?}",
      page.warnings
    );
  }

  #[test]
  fn sheet_geometry_lists_the_exceptions_and_boxes_every_cell_in_the_a1_frame() {
    let xml = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><cols><col min="1" max="1" width="20"/></cols><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>x</t></is></c></row><row r="2" ht="30"><c r="B2" t="inlineStr"><is><t>y</t></is></c></row></sheetData><mergeCells count="1"><mergeCell ref="A1:C1"/></mergeCells></worksheet>"##;
    let styles = Styles::default();
    let sheet = sheet_of(xml, &styles);
    let g = sheet_geometry(&sheet, &styles, true);
    // A 20 chars = 105 pt, B and C default 48 pt; the merge A1:C1 extends the frame to column C.
    assert_eq!((g.width, g.height), (201.0, 45.0));
    assert_eq!((g.default_column_width, g.default_row_height), (48.0, 15.0));
    assert_eq!(
      g.columns,
      vec![SheetColumn {
        index: 0,
        width: 105.0,
        hidden: false
      }]
    );
    assert_eq!(
      g.rows,
      vec![SheetRow {
        index: 1,
        height: 30.0,
        hidden: false
      }]
    );
    assert_eq!(
      g.boxes[&(0, 0)],
      CellBox {
        x0: 0.0,
        y0: 30.0,
        x1: 201.0,
        y1: 45.0
      },
      "a merged anchor spans its range"
    );
    assert_eq!(
      g.boxes[&(1, 1)],
      CellBox {
        x0: 105.0,
        y0: 0.0,
        x1: 153.0,
        y1: 30.0
      }
    );
  }
}
