use napi_derive::napi;

/// What `detectFormat` and `PdfDown` recognise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[napi(string_enum)]
pub enum DocumentFormat {
  Pdf,
  Xlsx,
  Unknown,
}

/// The outcome of `detectFormat`: the format, and for `Unknown` what the input was instead.
#[derive(Debug, Clone, PartialEq)]
#[napi(object)]
pub struct FormatDetection {
  pub format: DocumentFormat,
  pub detail: Option<String>,
}

/// Workbook options shared by `spreadsheetCells` and `layout`.
#[derive(Debug, Clone, Default)]
#[napi(object)]
pub struct SpreadsheetOptions {
  /// Sheet names to include, in workbook order; default: every visible sheet.
  pub sheets: Option<Vec<String>>,
  /// Include hidden sheets (default false).
  pub hidden_sheets: Option<bool>,
  /// Cells in hidden rows and columns. For `spreadsheetCells` the default is true: they are listed
  /// with `hidden: true`. For `layout` the default is false: zero extent and omitted; true gives
  /// them their real extents back.
  pub hidden_cells: Option<bool>,
}

/// The fields every cell carries, however it is typed.
pub(crate) struct CellCommon {
  pub reference: String,
  pub row: u32,
  pub col: u32,
  pub text: String,
  pub raw: String,
  pub format: Option<String>,
  pub format_supported: bool,
  pub formula: Option<String>,
  pub shared_formula: Option<String>,
  pub cached: bool,
  pub merge: Option<String>,
  pub hidden: bool,
}

macro_rules! sheet_cell {
  // Variant payloads are written in brackets and forwarded as raw token trees (see forms.rs).
  ($($variant:ident, $ctor:ident { $($field:ident : [$($ty:tt)+]),* $(,)? }),+ $(,)?) => {
    /// One cell of a worksheet: a discriminated union on `type` whose typed value is `value`
    /// (none for `Blank`). `text` is the display text, `raw` the stored spelling (the serial for
    /// dates), `format` the code that produced `text`, `formula` the expression stored on this
    /// cell, `sharedFormula` the master cell when this cell follows a shared formula, `cached`
    /// whether a result was stored, `merge` the range this cell anchors, `hidden` whether its row
    /// or column is hidden.
    #[derive(Debug, Clone)]
    #[napi(object_from_js = false)]
    pub enum SheetCell {
      $($variant {
        r#ref: String,
        row: u32,
        col: u32,
        text: String,
        raw: String,
        format: Option<String>,
        format_supported: bool,
        formula: Option<String>,
        shared_formula: Option<String>,
        cached: bool,
        merge: Option<String>,
        hidden: bool,
        $($field: $($ty)+,)*
      },)+
    }

    impl SheetCell {
      $(pub(crate) fn $ctor(c: CellCommon $(, $field: $($ty)+)*) -> SheetCell {
        SheetCell::$variant {
          r#ref: c.reference,
          row: c.row,
          col: c.col,
          text: c.text,
          raw: c.raw,
          format: c.format,
          format_supported: c.format_supported,
          formula: c.formula,
          shared_formula: c.shared_formula,
          cached: c.cached,
          merge: c.merge,
          hidden: c.hidden,
          $($field,)*
        }
      })+

    }
  };
}

sheet_cell! {
  Text, text { value: [String] },
  Number, number { value: [f64] },
  Boolean, boolean { value: [bool] },
  Error, error { value: [String] },
  Date, date { value: [String] },
  Blank, blank {}
}

/// One selected worksheet's cells, with the sheet facts a reader needs beside them.
#[derive(Debug, Clone)]
#[napi(object, object_from_js = false)]
pub struct SheetCells {
  pub sheet: String,
  /// Position in the workbook, 0-based.
  pub index: u32,
  pub hidden: bool,
  /// The declared `<dimension>`, which a producer may overstate.
  pub dimension: Option<String>,
  pub print_area: Option<String>,
  /// Merged ranges in A1 form.
  pub merges: Vec<String>,
  pub warnings: Vec<String>,
  pub cells: Vec<SheetCell>,
}
