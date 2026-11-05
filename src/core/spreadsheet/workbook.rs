//! The workbook model: parts to a sparse cell model with styles, shared formulas and caches.
//!
//! Parts are found through relationships, never by conventional name. Cells keep their stored
//! spelling (`raw`), their typed value, the formula stored on them or the shared master they
//! follow, and whether a result was cached. Numeric `text` is the format module's rendering of
//! the value under the cell's format code; a code this release does not render leaves `text` as
//! the stored spelling and marks the cell unsupported.

use std::collections::{BTreeMap, HashMap};

use roxmltree::Node;

use super::format::{NumberFormat, iso_date, render_general};
use super::package::{Package, SpreadsheetError, parse_xml};
use crate::types::{SheetCell, SheetCells, SpreadsheetOptions};

/// A workbook with more cells or merges than this is refused rather than modelled.
pub(crate) const MAX_CELLS: usize = 2_000_000;
pub(crate) const MAX_MERGES: usize = 200_000;
/// Excel's grid: a reference past it is malformed, which also bounds every per-row and per-column
/// allocation downstream.
pub(crate) const MAX_ROWS: u32 = 1_048_576;
pub(crate) const MAX_COLS: u32 = 16_384;
/// Excel's maxima for a column width (characters), a row height (points) and a font size (points).
const MAX_COL_WIDTH: f64 = 255.0;
const MAX_ROW_HEIGHT: f64 = 409.5;
const MAX_FONT_SIZE: f64 = 409.0;

/// What a cell holds, typed; dates carry both the serial and its ISO rendering.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum CellValue {
  Text(String),
  Number(f64),
  Boolean(bool),
  Error(String),
  /// The ISO rendering of a serial under a date code; the serial itself stays in `raw`.
  Date(String),
  Blank,
}

/// A rectangular range as 0-based inclusive corners.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CellRange {
  pub r0: u32,
  pub c0: u32,
  pub r1: u32,
  pub c1: u32,
}

impl CellRange {
  /// `A1:C3`, `C3:A1` (normalised) or a single `B2`.
  pub(crate) fn parse(text: &str) -> Option<CellRange> {
    let mut parts = text.split(':');
    let (r0, c0) = parse_cell_ref(parts.next()?)?;
    let (r1, c1) = match parts.next() {
      Some(end) => parse_cell_ref(end)?,
      None => (r0, c0),
    };
    if parts.next().is_some() {
      return None;
    }
    Some(CellRange {
      r0: r0.min(r1),
      c0: c0.min(c1),
      r1: r0.max(r1),
      c1: c0.max(c1),
    })
  }

  pub(crate) fn a1(&self) -> String {
    format!(
      "{}:{}",
      cell_ref(self.r0, self.c0),
      cell_ref(self.r1, self.c1)
    )
  }
}

/// `BC23` or `$BC$23` to 0-based `(row, column)`.
pub(crate) fn parse_cell_ref(text: &str) -> Option<(u32, u32)> {
  let cleaned: String = text.chars().filter(|&c| c != '$').collect();
  let split = cleaned.find(|c: char| c.is_ascii_digit())?;
  let (letters, digits) = cleaned.split_at(split);
  if letters.is_empty() || digits.is_empty() {
    return None;
  }
  let mut col: u32 = 0;
  for letter in letters.chars() {
    if !letter.is_ascii_alphabetic() {
      return None;
    }
    col = col
      .checked_mul(26)?
      .checked_add(u32::from(letter.to_ascii_uppercase() as u8 - b'A') + 1)?;
  }
  let row: u32 = digits.parse().ok()?;
  if row == 0 || col == 0 || row > MAX_ROWS || col > MAX_COLS {
    return None;
  }
  Some((row - 1, col - 1))
}

/// 0-based `(row, column)` to `BC23`.
pub(crate) fn cell_ref(row: u32, col: u32) -> String {
  let mut letters = Vec::new();
  let mut n = col + 1;
  while n > 0 {
    let rem = (n - 1) % 26;
    letters.push((b'A' + rem as u8) as char);
    n = (n - 1) / 26;
  }
  letters.reverse();
  format!("{}{}", letters.into_iter().collect::<String>(), row + 1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Horizontal {
  #[default]
  General,
  Left,
  Center,
  Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Vertical {
  Top,
  Center,
  #[default]
  Bottom,
}

/// One `cellXfs` entry: the number format, font and alignment a cell style selects.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Xf {
  pub num_fmt_id: u32,
  pub font_id: usize,
  pub horizontal: Horizontal,
  pub vertical: Vertical,
  pub wrap: bool,
  pub indent: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FontSpec {
  pub name: String,
  pub size: f64,
  pub bold: bool,
  pub italic: bool,
}

impl FontSpec {
  fn calibri_11() -> FontSpec {
    FontSpec {
      name: "Calibri".to_string(),
      size: 11.0,
      bold: false,
      italic: false,
    }
  }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Styles {
  pub xfs: Vec<Xf>,
  pub fonts: Vec<FontSpec>,
  pub custom_formats: HashMap<u32, String>,
  /// The Normal style's font, which SpreadsheetML column widths are measured in.
  pub normal_font: FontSpec,
}

impl Default for Styles {
  fn default() -> Self {
    Styles {
      xfs: Vec::new(),
      fonts: vec![FontSpec::calibri_11()],
      custom_formats: HashMap::new(),
      normal_font: FontSpec::calibri_11(),
    }
  }
}

/// Built-in number formats by id (ECMA-376 §18.8.30), in their en-US spellings.
fn builtin_format(id: u32) -> Option<&'static str> {
  Some(match id {
    1 => "0",
    2 => "0.00",
    3 => "#,##0",
    4 => "#,##0.00",
    5 => "$#,##0_);($#,##0)",
    6 => "$#,##0_);[Red]($#,##0)",
    7 => "$#,##0.00_);($#,##0.00)",
    8 => "$#,##0.00_);[Red]($#,##0.00)",
    9 => "0%",
    10 => "0.00%",
    11 => "0.00E+00",
    12 => "# ?/?",
    13 => "# ??/??",
    14 => "m/d/yyyy",
    15 => "d-mmm-yy",
    16 => "d-mmm",
    17 => "mmm-yy",
    18 => "h:mm AM/PM",
    19 => "h:mm:ss AM/PM",
    20 => "h:mm",
    21 => "h:mm:ss",
    22 => "m/d/yyyy h:mm",
    37 => "#,##0 ;(#,##0)",
    38 => "#,##0 ;[Red](#,##0)",
    39 => "#,##0.00;(#,##0.00)",
    40 => "#,##0.00;[Red](#,##0.00)",
    41 => "_(* #,##0_);_(* (#,##0);_(* \"-\"_);_(@_)",
    42 => "_($* #,##0_);_($* (#,##0);_($* \"-\"_);_(@_)",
    43 => "_(* #,##0.00_);_(* (#,##0.00);_(* \"-\"??_);_(@_)",
    44 => "_($* #,##0.00_);_($* (#,##0.00);_($* \"-\"??_);_(@_)",
    45 => "mm:ss",
    46 => "[h]:mm:ss",
    47 => "mmss.0",
    48 => "##0.0E+0",
    49 => "@",
    _ => return None,
  })
}

/// The format code a cell style selects: the file's own `<numFmt>` for the id when it has one,
/// else the built-in spelling; `None` for General, an unknown style, or a built-in id this
/// release has no spelling for (see `unknown_builtin_id`).
pub(crate) fn format_code(styles: &Styles, style: usize) -> Option<String> {
  let id = styles.xfs.get(style)?.num_fmt_id;
  if id == 0 {
    return None;
  }
  styles
    .custom_formats
    .get(&id)
    .cloned()
    .or_else(|| builtin_format(id).map(str::to_string))
}

/// A non-zero built-in id with neither a `<numFmt>` nor a spelling in the table: a format this
/// release cannot render and cannot even name.
fn unknown_builtin_id(styles: &Styles, style: usize) -> Option<u32> {
  let id = styles.xfs.get(style)?.num_fmt_id;
  (id != 0 && !styles.custom_formats.contains_key(&id) && builtin_format(id).is_none())
    .then_some(id)
}

/// A length from the file: a non-negative number capped at Excel's maximum for it (an overflowing
/// spelling such as `1e999` is the maximum); `NaN` and negatives are no length.
fn length_attr(value: Option<&str>, max: f64) -> Option<f64> {
  value
    .and_then(|v| v.parse::<f64>().ok())
    .filter(|v| !v.is_nan() && *v >= 0.0)
    .map(|v| v.min(max))
}

fn attr<'a>(node: Node<'a, 'a>, name: &str) -> Option<&'a str> {
  node.attribute(name)
}

fn is_on(value: Option<&str>) -> bool {
  !matches!(value, Some("0" | "false" | "off"))
}

/// `styles.xml`: fonts, custom number formats and every `cellXfs` entry, with or without children.
pub(crate) fn parse_styles(bytes: &[u8]) -> Result<Styles, SpreadsheetError> {
  let doc = parse_xml(bytes)?;
  let mut styles = Styles {
    fonts: Vec::new(),
    ..Styles::default()
  };
  let root = doc.root_element();
  let children = |name: &str| {
    root
      .children()
      .find(|n| n.is_element() && n.tag_name().name() == name)
  };
  if let Some(fonts) = children("fonts") {
    for font in fonts
      .children()
      .filter(|n| n.is_element() && n.tag_name().name() == "font")
    {
      let mut spec = FontSpec::calibri_11();
      for prop in font.children().filter(|n| n.is_element()) {
        match prop.tag_name().name() {
          "name" => spec.name = attr(prop, "val").unwrap_or("Calibri").to_string(),
          "sz" => spec.size = length_attr(attr(prop, "val"), MAX_FONT_SIZE).unwrap_or(11.0),
          "b" => spec.bold = is_on(attr(prop, "val")),
          "i" => spec.italic = is_on(attr(prop, "val")),
          _ => {}
        }
      }
      styles.fonts.push(spec);
    }
  }
  if styles.fonts.is_empty() {
    styles.fonts.push(FontSpec::calibri_11());
  }
  styles.normal_font = styles.fonts[0].clone();
  if let Some(formats) = children("numFmts") {
    for f in formats
      .children()
      .filter(|n| n.is_element() && n.tag_name().name() == "numFmt")
    {
      if let (Some(id), Some(code)) = (
        attr(f, "numFmtId").and_then(|v| v.parse().ok()),
        attr(f, "formatCode"),
      ) {
        styles.custom_formats.insert(id, code.to_string());
      }
    }
  }
  if let Some(xfs) = children("cellXfs") {
    for xf in xfs
      .children()
      .filter(|n| n.is_element() && n.tag_name().name() == "xf")
    {
      let alignment = xf
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "alignment");
      let horizontal = match alignment.and_then(|a| attr(a, "horizontal")) {
        Some("left") => Horizontal::Left,
        Some("center" | "centerContinuous") => Horizontal::Center,
        Some("right") => Horizontal::Right,
        _ => Horizontal::General,
      };
      let vertical = match alignment.and_then(|a| attr(a, "vertical")) {
        Some("top") => Vertical::Top,
        Some("center") => Vertical::Center,
        _ => Vertical::Bottom,
      };
      styles.xfs.push(Xf {
        num_fmt_id: attr(xf, "numFmtId")
          .and_then(|v| v.parse().ok())
          .unwrap_or(0),
        font_id: attr(xf, "fontId").and_then(|v| v.parse().ok()).unwrap_or(0),
        horizontal,
        vertical,
        wrap: alignment
          .is_some_and(|a| is_on(attr(a, "wrapText")) && attr(a, "wrapText").is_some()),
        indent: alignment
          .and_then(|a| attr(a, "indent"))
          .and_then(|v| v.parse().ok())
          .unwrap_or(0),
      });
    }
  }
  Ok(styles)
}

/// `sharedStrings.xml`: every `<si>` keeps its slot; runs concatenate; phonetic runs are skipped.
pub(crate) fn parse_shared_strings(bytes: &[u8]) -> Result<Vec<String>, SpreadsheetError> {
  let doc = parse_xml(bytes)?;
  Ok(
    doc
      .root_element()
      .children()
      .filter(|n| n.is_element() && n.tag_name().name() == "si")
      .map(|si| rich_text(si))
      .collect(),
  )
}

/// The text of an `<si>` or `<is>`: every `<t>` in order, except those inside `<rPh>`.
fn rich_text(node: Node<'_, '_>) -> String {
  let mut out = String::new();
  for t in node
    .descendants()
    .filter(|n| n.is_element() && n.tag_name().name() == "t")
  {
    let phonetic = t
      .ancestors()
      .any(|a| a.is_element() && a.tag_name().name() == "rPh");
    if !phonetic {
      out.push_str(t.text().unwrap_or(""));
    }
  }
  out
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ColSpec {
  pub min: u32,
  pub max: u32,
  pub width: f64,
  pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct RowSpec {
  pub height: Option<f64>,
  pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Cell {
  pub value: CellValue,
  pub raw: String,
  pub text: String,
  pub format: Option<String>,
  pub format_supported: bool,
  pub formula: Option<String>,
  pub shared_master: Option<(u32, u32)>,
  pub cached: bool,
  pub style: usize,
  pub merge: Option<CellRange>,
  /// The style's built-in number format id when this release has no spelling for it.
  pub builtin_id: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Sheet {
  pub name: String,
  pub index: u32,
  pub hidden: bool,
  pub dimension: Option<CellRange>,
  pub print_area: Option<CellRange>,
  pub cols: Vec<ColSpec>,
  pub rows: BTreeMap<u32, RowSpec>,
  pub default_row_height: f64,
  pub default_col_width: Option<f64>,
  pub cells: BTreeMap<(u32, u32), Cell>,
  pub merges: Vec<CellRange>,
  pub drawings: usize,
  pub warnings: Vec<String>,
}

impl Sheet {
  pub(crate) fn col_hidden(&self, col: u32) -> bool {
    self
      .cols
      .iter()
      .any(|c| c.hidden && col + 1 >= c.min && col < c.max)
  }

  pub(crate) fn row_hidden(&self, row: u32) -> bool {
    self.rows.get(&row).is_some_and(|r| r.hidden)
  }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Workbook {
  pub sheets: Vec<Sheet>,
  pub date1904: bool,
  pub styles: Styles,
}

struct SheetRef {
  name: String,
  rid: String,
  hidden: bool,
}

/// `Form!$A$1:$F$12` (the sheet part possibly quoted) to a range.
/// `Sheet!$A$1:$F$12` to a range; `None` for a union of ranges (a top-level comma), which is no
/// print area for the page.
fn print_area_range(text: &str) -> Option<CellRange> {
  let mut quoted = false;
  for c in text.chars() {
    match c {
      '\'' => quoted = !quoted,
      ',' if !quoted => return None,
      _ => {}
    }
  }
  let range = text.rsplit_once('!').map(|(_, r)| r).unwrap_or(text);
  CellRange::parse(range.trim())
}

/// Read a whole workbook from its package.
pub(crate) fn parse_workbook(package: &mut Package) -> Result<Workbook, SpreadsheetError> {
  let main = package.officedocument()?;
  let (dir, file) = main.rsplit_once('/').unwrap_or(("", main.as_str()));
  let rels_path = if dir.is_empty() {
    format!("_rels/{file}.rels")
  } else {
    format!("{dir}/_rels/{file}.rels")
  };
  let rels = package.relationships(&rels_path)?;
  let by_id: HashMap<&str, &str> = rels
    .iter()
    .filter(|r| !r.external)
    .map(|r| (r.id.as_str(), r.target.as_str()))
    .collect();
  let target_of_kind = |suffix: &str| {
    rels
      .iter()
      .find(|r| !r.external && r.kind.ends_with(suffix))
      .map(|r| r.target.clone())
  };

  let workbook_xml = package.part(&main)?;
  let doc = parse_xml(&workbook_xml)?;
  let root = doc.root_element();
  let mut date1904 = false;
  let mut sheet_refs = Vec::new();
  let mut print_areas: HashMap<usize, CellRange> = HashMap::new();
  let mut print_area_warnings: HashMap<usize, String> = HashMap::new();
  for n in root.descendants().filter(|n| n.is_element()) {
    match n.tag_name().name() {
      "workbookPr" => date1904 = matches!(attr(n, "date1904"), Some("1" | "true")),
      "sheet" => sheet_refs.push(SheetRef {
        name: attr(n, "name").unwrap_or("").to_string(),
        rid: attr(n, "id").unwrap_or("").to_string(),
        hidden: matches!(attr(n, "state"), Some("hidden" | "veryHidden")),
      }),
      "definedName" if attr(n, "name") == Some("_xlnm.Print_Area") => {
        if let (Some(sheet), Some(text)) = (
          attr(n, "localSheetId").and_then(|v| v.parse::<usize>().ok()),
          n.text(),
        ) {
          match print_area_range(text) {
            Some(range) => {
              print_areas.insert(sheet, range);
            }
            None => {
              print_area_warnings.insert(
                sheet,
                "the print area is not a single rectangle; the page is the content extent"
                  .to_string(),
              );
            }
          }
        }
      }
      _ => {}
    }
  }
  if sheet_refs.is_empty() {
    return Err(SpreadsheetError::Malformed(
      "the workbook lists no sheets".to_string(),
    ));
  }

  let shared = match target_of_kind("/sharedStrings") {
    Some(path) => parse_shared_strings(&package.part(&path)?)?,
    None => Vec::new(),
  };
  let styles = match target_of_kind("/styles") {
    Some(path) => parse_styles(&package.part(&path)?)?,
    None => Styles::default(),
  };

  let mut sheets = Vec::with_capacity(sheet_refs.len());
  let mut cells_total = 0usize;
  let mut merges_total = 0usize;
  for (index, sheet_ref) in sheet_refs.iter().enumerate() {
    let Some(path) = by_id.get(sheet_ref.rid.as_str()) else {
      return Err(SpreadsheetError::Malformed(format!(
        "sheet \"{}\" has no relationship for {}",
        sheet_ref.name, sheet_ref.rid
      )));
    };
    let bytes = package.part(path)?;
    let mut sheet = parse_sheet(
      &bytes,
      &sheet_ref.name,
      index as u32,
      sheet_ref.hidden,
      WorkbookContext {
        shared: &shared,
        styles: &styles,
        date1904,
      },
      print_areas.get(&index).copied(),
    )?;
    if let Some(warning) = print_area_warnings.remove(&index) {
      sheet.warnings.push(warning);
    }
    cells_total += sheet.cells.len();
    merges_total += sheet.merges.len();
    if cells_total > MAX_CELLS {
      return Err(SpreadsheetError::LimitExceeded(format!(
        "the workbook has more than {MAX_CELLS} cells"
      )));
    }
    if merges_total > MAX_MERGES {
      return Err(SpreadsheetError::LimitExceeded(format!(
        "the workbook has more than {MAX_MERGES} merged ranges"
      )));
    }
    sheets.push(sheet);
  }
  Ok(Workbook {
    sheets,
    date1904,
    styles,
  })
}

/// One `<c>` as read, before its value is resolved.
#[derive(Default)]
struct RawCell {
  kind: Option<String>,
  style: usize,
  value: Option<String>,
  inline: Option<String>,
  formula: Option<String>,
  shared_index: Option<u32>,
  shared_ref: Option<String>,
}

/// What every sheet of a workbook shares: the string table, the styles and the date system.
#[derive(Debug, Clone, Copy)]
pub(crate) struct WorkbookContext<'a> {
  pub shared: &'a [String],
  pub styles: &'a Styles,
  pub date1904: bool,
}

/// One worksheet part to a `Sheet`. Malformed references and ranges become warnings, never panics.
pub(crate) fn parse_sheet(
  bytes: &[u8],
  name: &str,
  index: u32,
  hidden: bool,
  context: WorkbookContext<'_>,
  print_area: Option<CellRange>,
) -> Result<Sheet, SpreadsheetError> {
  let doc = parse_xml(bytes)?;
  let root = doc.root_element();
  let mut sheet = Sheet {
    name: name.to_string(),
    index,
    hidden,
    dimension: None,
    print_area,
    cols: Vec::new(),
    rows: BTreeMap::new(),
    default_row_height: 15.0,
    default_col_width: None,
    cells: BTreeMap::new(),
    merges: Vec::new(),
    drawings: 0,
    warnings: Vec::new(),
  };
  let mut shared_masters: HashMap<u32, (u32, u32)> = HashMap::new();

  for n in root.children().filter(|n| n.is_element()) {
    match n.tag_name().name() {
      "dimension" => sheet.dimension = attr(n, "ref").and_then(CellRange::parse),
      "sheetFormatPr" => {
        sheet.default_row_height =
          length_attr(attr(n, "defaultRowHeight"), MAX_ROW_HEIGHT).unwrap_or(15.0);
        sheet.default_col_width = length_attr(attr(n, "defaultColWidth"), MAX_COL_WIDTH);
      }
      "cols" => {
        for c in n
          .children()
          .filter(|c| c.is_element() && c.tag_name().name() == "col")
        {
          if let (Some(min), Some(max)) = (
            attr(c, "min").and_then(|v| v.parse().ok()),
            attr(c, "max").and_then(|v| v.parse().ok()),
          ) {
            sheet.cols.push(ColSpec {
              min,
              max,
              width: length_attr(attr(c, "width"), MAX_COL_WIDTH).unwrap_or(0.0),
              hidden: attr(c, "hidden").is_some_and(|v| is_on(Some(v))),
            });
          }
        }
      }
      "sheetData" => parse_rows(n, &mut sheet, context, &mut shared_masters),
      "mergeCells" => {
        for m in n
          .children()
          .filter(|m| m.is_element() && m.tag_name().name() == "mergeCell")
        {
          match attr(m, "ref").and_then(CellRange::parse) {
            Some(range) => sheet.merges.push(range),
            None => sheet.warnings.push(format!(
              "mergeCell ref {:?} is not a range; skipped",
              attr(m, "ref").unwrap_or("")
            )),
          }
        }
      }
      "drawing" => sheet.drawings += 1,
      _ => {}
    }
  }
  for range in &sheet.merges {
    if let Some(cell) = sheet.cells.get_mut(&(range.r0, range.c0)) {
      cell.merge = Some(*range);
    }
  }
  let unsupported = sheet.cells.values().filter(|c| !c.format_supported).count();
  if unsupported > 0 {
    sheet.warnings.push(format!(
      "{unsupported} cell(s) use number formats this release does not render"
    ));
  }
  let unknown_ids: std::collections::BTreeSet<u32> =
    sheet.cells.values().filter_map(|c| c.builtin_id).collect();
  if !unknown_ids.is_empty() {
    let ids: Vec<String> = unknown_ids.iter().map(u32::to_string).collect();
    sheet.warnings.push(format!(
      "built-in number format id(s) {} have no spelling this release knows; those cells keep their stored text",
      ids.join(", ")
    ));
  }
  Ok(sheet)
}

fn parse_rows(
  sheet_data: Node<'_, '_>,
  sheet: &mut Sheet,
  context: WorkbookContext<'_>,
  shared_masters: &mut HashMap<u32, (u32, u32)>,
) {
  let mut current_row: u32 = 0;
  for row in sheet_data
    .children()
    .filter(|r| r.is_element() && r.tag_name().name() == "row")
  {
    current_row = attr(row, "r")
      .and_then(|v| v.parse::<u32>().ok())
      .map(|r| r.saturating_sub(1))
      .unwrap_or(current_row);
    if current_row >= MAX_ROWS {
      sheet.warnings.push(format!(
        "row {} is beyond the grid; skipped",
        current_row.saturating_add(1)
      ));
      current_row = current_row.saturating_add(1);
      continue;
    }
    let spec = RowSpec {
      height: length_attr(attr(row, "ht"), MAX_ROW_HEIGHT),
      hidden: attr(row, "hidden").is_some_and(|v| is_on(Some(v))),
    };
    if spec != RowSpec::default() {
      sheet.rows.insert(current_row, spec);
    }
    let mut next_col: u32 = 0;
    for c in row
      .children()
      .filter(|c| c.is_element() && c.tag_name().name() == "c")
    {
      let position = match attr(c, "r") {
        Some(text) => match parse_cell_ref(text) {
          Some(pos) => pos,
          None => {
            sheet
              .warnings
              .push(format!("cell reference {text:?} is malformed; skipped"));
            continue;
          }
        },
        None => (current_row, next_col),
      };
      if position.1 >= MAX_COLS {
        sheet
          .warnings
          .push("a cell without a reference falls beyond the grid; skipped".to_string());
        continue;
      }
      next_col = position.1.saturating_add(1);
      let mut raw = RawCell {
        kind: attr(c, "t").map(str::to_string),
        style: attr(c, "s").and_then(|v| v.parse().ok()).unwrap_or(0),
        ..RawCell::default()
      };
      for child in c.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
          "v" => raw.value = Some(child.text().unwrap_or("").to_string()),
          "is" => raw.inline = Some(rich_text(child)),
          "f" => {
            let text = child
              .text()
              .map(str::to_string)
              .filter(|t| !t.trim().is_empty());
            raw.formula = text;
            if attr(child, "t") == Some("shared") {
              raw.shared_index = attr(child, "si").and_then(|v| v.parse().ok());
              raw.shared_ref = attr(child, "ref").map(str::to_string);
            }
          }
          _ => {}
        }
      }
      if let Some(cell) = resolve_cell(raw, position, context, shared_masters, &mut sheet.warnings)
      {
        sheet.cells.insert(position, cell);
      }
    }
    current_row = current_row.saturating_add(1);
  }
}

fn resolve_cell(
  raw: RawCell,
  position: (u32, u32),
  context: WorkbookContext<'_>,
  shared_masters: &mut HashMap<u32, (u32, u32)>,
  warnings: &mut Vec<String>,
) -> Option<Cell> {
  let WorkbookContext {
    shared,
    styles,
    date1904,
  } = context;
  let reference = cell_ref(position.0, position.1);
  let mut formula = raw.formula.clone();
  let mut shared_master = None;
  if let Some(si) = raw.shared_index {
    if raw.shared_ref.is_some() && formula.is_some() {
      shared_masters.insert(si, position);
    } else if let Some(master) = shared_masters.get(&si) {
      shared_master = Some(*master);
      formula = None;
    } else {
      warnings.push(format!(
        "{reference}: shared formula {si} has no master; skipped"
      ));
    }
  }
  let cached = raw.value.is_some();
  let has_formula = formula.is_some() || shared_master.is_some();
  let stored = raw.value.clone().unwrap_or_default();
  let (value, text) = match raw.kind.as_deref() {
    Some("s") => {
      let Some(text) = stored
        .trim()
        .parse::<usize>()
        .ok()
        .and_then(|i| shared.get(i))
      else {
        warnings.push(format!(
          "{reference}: shared string index {stored:?} is out of range; skipped"
        ));
        return None;
      };
      (CellValue::Text(text.clone()), text.clone())
    }
    Some("str") => (CellValue::Text(stored.clone()), stored.clone()),
    Some("inlineStr") => {
      let text = raw.inline.clone().unwrap_or_default();
      (CellValue::Text(text.clone()), text)
    }
    Some("b") => {
      let on = matches!(stored.trim(), "1" | "true" | "TRUE");
      (
        CellValue::Boolean(on),
        if on { "TRUE" } else { "FALSE" }.to_string(),
      )
    }
    Some("e") => (CellValue::Error(stored.clone()), stored.clone()),
    _ => match raw.value.as_deref().map(str::trim) {
      Some(number) if !number.is_empty() => match number.parse::<f64>() {
        Ok(n) if n.is_finite() => (CellValue::Number(n), number.to_string()),
        _ => (CellValue::Text(number.to_string()), number.to_string()),
      },
      Some(_) => (CellValue::Text(String::new()), String::new()),
      None if has_formula => (CellValue::Blank, String::new()),
      None => return None,
    },
  };
  let raw_text = match &value {
    CellValue::Text(_) | CellValue::Error(_) | CellValue::Boolean(_) => text.clone(),
    _ => stored,
  };
  // Cell type comes before number format: only a number is rendered through its code.
  let format = format_code(styles, raw.style);
  let builtin_id = if format.is_none() {
    unknown_builtin_id(styles, raw.style)
  } else {
    None
  };
  let mut format_supported = true;
  let (value, text) = match (value, &format) {
    (CellValue::Number(n), Some(code)) => match NumberFormat::parse(code) {
      Some(parsed) if parsed.is_date() => {
        match (
          parsed.render(n, date1904),
          iso_date(n, date1904, parsed.has_time()),
        ) {
          (Some(rendered), Some(iso)) => (CellValue::Date(iso), rendered),
          _ => {
            warnings.push(if n == 60.0 && !date1904 {
              format!(
                "{reference}: serial 60 is the 1900 leap day Excel never had; kept as a number"
              )
            } else {
              format!(
                "{reference}: serial {raw_text} is not a date under {code:?}; kept as a number"
              )
            });
            (CellValue::Number(n), text)
          }
        }
      }
      Some(parsed) => (
        CellValue::Number(n),
        parsed.render(n, date1904).unwrap_or(text),
      ),
      None => {
        format_supported = false;
        (CellValue::Number(n), text)
      }
    },
    (CellValue::Number(n), None) => match builtin_id {
      Some(_) => {
        format_supported = false;
        (CellValue::Number(n), text)
      }
      None => (CellValue::Number(n), render_general(n)),
    },
    (value, _) => (value, text),
  };
  Some(Cell {
    format_supported,
    value,
    raw: raw_text,
    text,
    format,
    formula,
    shared_master,
    cached,
    style: raw.style,
    merge: None,
    builtin_id,
  })
}

/// The sheets a caller asked for: by name, visible ones by default.
pub(crate) fn selected_sheets<'a>(
  workbook: &'a Workbook,
  opts: &SpreadsheetOptions,
) -> Vec<&'a Sheet> {
  let hidden_sheets = opts.hidden_sheets.unwrap_or(false);
  workbook
    .sheets
    .iter()
    .filter(|s| match &opts.sheets {
      Some(names) => names.iter().any(|n| n == &s.name),
      None => hidden_sheets || !s.hidden,
    })
    .collect()
}

/// The public cells of the selected sheets.
pub(crate) fn cells_of(workbook: &Workbook, opts: &SpreadsheetOptions) -> Vec<SheetCells> {
  let hidden_cells = opts.hidden_cells.unwrap_or(true);
  selected_sheets(workbook, opts)
    .into_iter()
    .map(|sheet| {
      let cells = sheet
        .cells
        .iter()
        .filter_map(|(&(row, col), cell)| {
          let hidden = sheet.row_hidden(row) || sheet.col_hidden(col);
          if hidden && !hidden_cells {
            return None;
          }
          Some(public_cell(row, col, cell, hidden))
        })
        .collect();
      SheetCells {
        sheet: sheet.name.clone(),
        index: sheet.index,
        hidden: sheet.hidden,
        dimension: sheet.dimension.map(|d| d.a1()),
        print_area: sheet.print_area.map(|p| p.a1()),
        merges: sheet.merges.iter().map(CellRange::a1).collect(),
        warnings: sheet.warnings.clone(),
        cells,
      }
    })
    .collect()
}

fn public_cell(row: u32, col: u32, cell: &Cell, hidden: bool) -> SheetCell {
  let common = crate::types::CellCommon {
    reference: cell_ref(row, col),
    row,
    col,
    text: cell.text.clone(),
    raw: cell.raw.clone(),
    format: cell.format.clone(),
    format_supported: cell.format_supported,
    formula: cell.formula.clone(),
    shared_formula: cell.shared_master.map(|(r, c)| cell_ref(r, c)),
    cached: cell.cached,
    merge: cell.merge.map(|m| m.a1()),
    hidden,
  };
  match &cell.value {
    CellValue::Text(v) => SheetCell::text(common, v.clone()),
    CellValue::Number(n) => SheetCell::number(common, *n),
    CellValue::Boolean(b) => SheetCell::boolean(common, *b),
    CellValue::Error(e) => SheetCell::error(common, e.clone()),
    CellValue::Date(iso) => SheetCell::date(common, iso.clone()),
    CellValue::Blank => SheetCell::blank(common),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn cell_references_and_ranges() {
    assert_eq!(parse_cell_ref("A1"), Some((0, 0)));
    assert_eq!(parse_cell_ref("$C$3"), Some((2, 2)));
    assert_eq!(parse_cell_ref("XFD1048576"), Some((1_048_575, 16_383)));
    assert_eq!(parse_cell_ref("A0"), None);
    assert_eq!(parse_cell_ref("1A"), None);
    assert_eq!(parse_cell_ref(""), None);
    assert_eq!(cell_ref(1, 27), "AB2");
    assert_eq!(CellRange::parse("C3:A1").unwrap().a1(), "A1:C3");
    assert_eq!(CellRange::parse("B2").unwrap().a1(), "B2:B2");
    assert_eq!(CellRange::parse("A1:B2:C3"), None);
  }

  #[test]
  fn styles_count_every_xf_and_resolve_format_codes() {
    let xml = br##"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><numFmts count="1"><numFmt numFmtId="164" formatCode="#,##0.00&quot;/hr&quot;"/></numFmts><fonts count="2"><font><sz val="11"/><name val="Calibri"/></font><font><b/><sz val="10"/><name val="Arial"/></font></fonts><cellXfs count="2"><xf numFmtId="4" fontId="1"><alignment vertical="center" wrapText="1" indent="1"/></xf><xf numFmtId="164" fontId="0"/></cellXfs></styleSheet>"##;
    let styles = parse_styles(xml).unwrap();
    assert_eq!(styles.xfs.len(), 2);
    assert_eq!(styles.xfs[0].vertical, Vertical::Center);
    assert!(styles.xfs[0].wrap);
    assert_eq!(styles.xfs[0].indent, 1);
    assert_eq!(styles.xfs[0].font_id, 1);
    assert!(styles.fonts[1].bold);
    assert_eq!(styles.fonts[1].name, "Arial");
    assert_eq!(format_code(&styles, 0).as_deref(), Some("#,##0.00"));
    assert_eq!(format_code(&styles, 1).as_deref(), Some("#,##0.00\"/hr\""));
    assert_eq!(format_code(&styles, 9), None);
  }

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

  #[test]
  fn references_beyond_the_grid_are_malformed_and_rows_never_wrap() {
    assert_eq!(parse_cell_ref("XFD1048576"), Some((1_048_575, 16_383)));
    assert_eq!(parse_cell_ref("A1048577"), None, "row past 1,048,576");
    assert_eq!(parse_cell_ref("XFE1"), None, "column past XFD");
    assert_eq!(parse_cell_ref("A999999999"), None);
    let xml = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1048576"><c><v>1</v></c></row><row><c><v>2</v></c><c r="A999999999"><v>3</v></c></row><row r="4294967295"><c><v>4</v></c></row><row><c><v>5</v></c></row></sheetData></worksheet>"##;
    let sheet = sheet_of(xml, &Styles::default());
    assert_eq!(
      sheet.cells.keys().collect::<Vec<_>>(),
      [&(1_048_575, 0)],
      "only the last row of the grid holds a cell: {:?}",
      sheet.warnings
    );
    assert!(sheet.cells.values().all(|c| c.text == "1"));
    assert!(
      sheet
        .warnings
        .iter()
        .filter(|w| w.contains("beyond"))
        .count()
        >= 3,
      "{:?}",
      sheet.warnings
    );
  }

  #[test]
  fn general_cells_render_through_general_and_values_are_finite() {
    let xml = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1"><v>0.30000000000000004</v></c><c r="B1"><v>427.49999999999994</v></c><c r="C1"><v>1e300</v></c><c r="D1"><v>NaN</v></c><c r="E1"><v>inf</v></c></row></sheetData></worksheet>"##;
    let sheet = sheet_of(xml, &Styles::default());
    assert_eq!(sheet.cells[&(0, 0)].text, "0.3");
    assert_eq!(sheet.cells[&(0, 0)].raw, "0.30000000000000004");
    assert_eq!(sheet.cells[&(0, 1)].text, "427.5");
    assert_eq!(sheet.cells[&(0, 2)].text, "1E+300");
    assert_eq!(
      sheet.cells[&(0, 3)].value,
      CellValue::Text("NaN".to_string())
    );
    assert_eq!(
      sheet.cells[&(0, 4)].value,
      CellValue::Text("inf".to_string())
    );
  }

  #[test]
  fn builtin_ids_render_or_are_unsupported_and_a_files_own_code_wins() {
    let xml = br##"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><numFmts count="1"><numFmt numFmtId="4" formatCode="0.0"/></numFmts><cellXfs count="4"><xf numFmtId="44"/><xf numFmtId="27"/><xf numFmtId="4"/><xf numFmtId="7"/></cellXfs></styleSheet>"##;
    let styles = parse_styles(xml).unwrap();
    assert_eq!(
      format_code(&styles, 0).as_deref(),
      Some("_($* #,##0.00_);_($* (#,##0.00);_($* \"-\"??_);_(@_)")
    );
    assert_eq!(
      format_code(&styles, 2).as_deref(),
      Some("0.0"),
      "the file's own code for id 4 wins"
    );
    assert_eq!(
      format_code(&styles, 3).as_deref(),
      Some("$#,##0.00_);($#,##0.00)")
    );
    let sheet_xml = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" s="0"><v>1234.5</v></c><c r="B1" s="1"><v>42247</v></c><c r="C1" s="1"><v>7</v></c></row></sheetData></worksheet>"##;
    let sheet = sheet_of(sheet_xml, &styles);
    assert_eq!(sheet.cells[&(0, 0)].text, "$1,234.50");
    let unknown = &sheet.cells[&(0, 1)];
    assert!(
      !unknown.format_supported,
      "built-in 27 has no en-US code this release knows"
    );
    assert_eq!(unknown.text, "42247");
    assert_eq!(unknown.value, CellValue::Number(42247.0));
    assert!(
      sheet
        .warnings
        .iter()
        .any(|w| w.contains("built-in") && w.contains("27")),
      "{:?}",
      sheet.warnings
    );
  }

  #[test]
  fn a_multi_range_print_area_is_no_print_area() {
    assert_eq!(print_area_range("S!$A$1:$B$2,S!$D$1:$E$2"), None);
    assert_eq!(
      print_area_range("'My Sheet'!$A$1:$F$12").unwrap().a1(),
      "A1:F12"
    );
  }

  #[test]
  fn geometry_from_the_file_is_finite_and_within_excels_maxima() {
    let styles_xml = br##"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="2"><font><sz val="1e999"/></font><font><sz val="NaN"/></font></fonts><cellXfs count="1"><xf numFmtId="0" fontId="0"/></cellXfs></styleSheet>"##;
    let styles = parse_styles(styles_xml).unwrap();
    assert_eq!(styles.fonts[0].size, 409.0);
    assert_eq!(styles.fonts[1].size, 11.0);
    let xml = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetFormatPr defaultRowHeight="inf" defaultColWidth="-5"/><cols><col min="1" max="1" width="1e999"/><col min="2" max="2" width="NaN"/></cols><sheetData><row r="1" ht="1e999"><c r="A1"><v>1</v></c></row><row r="2" ht="-3"><c r="A2"><v>2</v></c></row></sheetData></worksheet>"##;
    let sheet = sheet_of(xml, &styles);
    assert_eq!(
      sheet.default_row_height, 409.5,
      "an overflowing spelling is the maximum"
    );
    assert_eq!(
      sheet.default_col_width, None,
      "a negative width is no width"
    );
    assert_eq!(sheet.cols[0].width, 255.0);
    assert_eq!(
      sheet.cols[1].width, 0.0,
      "an unparsable width means the default"
    );
    assert_eq!(sheet.rows[&0].height, Some(409.5));
    assert_eq!(sheet.rows.get(&1).and_then(|r| r.height), None);
  }

  #[test]
  fn malformed_references_indices_and_merges_warn_and_skip() {
    let xml = br##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="ZZZZ"><v>1</v></c><c r="B1" t="s"><v>99</v></c><c r="C1" t="s"><v>1</v></c></row></sheetData><mergeCells><mergeCell ref="nope"/></mergeCells></worksheet>"##;
    let shared = vec!["zero".to_string(), "one".to_string()];
    let sheet = parse_sheet(
      xml,
      "S",
      0,
      false,
      WorkbookContext {
        shared: &shared,
        styles: &Styles::default(),
        date1904: false,
      },
      None,
    )
    .unwrap();
    assert_eq!(sheet.warnings.len(), 3, "{:?}", sheet.warnings);
    assert_eq!(sheet.cells.len(), 1);
    assert_eq!(sheet.cells[&(0, 2)].text, "one");
  }
}
