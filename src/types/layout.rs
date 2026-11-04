use napi_derive::napi;

/// The deepest tier of the OCR cascade that ran for a page: `Native` when the
/// page's own glyphs were the page, `Ocr` when its embedded rasters were OCR'd,
/// `Rendered` when the page was rasterised and OCR'd (only with the `render`
/// feature). A hybrid page keeps its native words alongside the OCR words and
/// still reports the OCR tier, even when that tier added no words; tell the
/// two apart by `confidence`, which only OCR words carry, and see the "native
/// words kept" warnings. Present in every build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[napi(string_enum)]
pub enum TextSource {
  Native,
  Ocr,
  Rendered,
}

/// One word with its box in PDF user space (points, y up).
#[derive(Debug, Clone, PartialEq)]
#[napi(object)]
pub struct TextWord {
  pub text: String,
  pub x0: f64,
  pub y0: f64,
  pub x1: f64,
  pub y1: f64,
  /// Index into `PageLayout.lines`.
  pub line: u32,
  /// Native only: font size in user space (after the CTM), in points.
  pub font_size: Option<f64>,
  /// Native only: base font name with any subset prefix stripped.
  pub font_name: Option<String>,
  /// Native: from the font descriptor and name; absent when the font gives no weight evidence
  /// (anonymous names such as `CIDFont+F1`) and for OCR words.
  pub bold: Option<bool>,
  pub italic: Option<bool>,
  /// Native only: true when the word is set in a barcode font, so its glyphs are bars, not letters.
  pub symbolic: Option<bool>,
  /// OCR only: Tesseract word confidence, 0..1.
  pub confidence: Option<f64>,
}

/// Words sharing a baseline band, left to right. `firstWord` / `wordCount`
/// address a contiguous slice of `PageLayout.words`.
#[derive(Debug, Clone, PartialEq)]
#[napi(object)]
pub struct TextLine {
  pub text: String,
  pub x0: f64,
  pub y0: f64,
  pub x1: f64,
  pub y1: f64,
  pub first_word: u32,
  pub word_count: u32,
}

/// Where an image XObject was painted: the unit square under the CTM at the
/// `Do` operator, as an axis-aligned box in user space.
#[derive(Debug, Clone, PartialEq)]
#[napi(object)]
pub struct PlacedImage {
  pub xobject_name: String,
  pub x0: f64,
  pub y0: f64,
  pub x1: f64,
  pub y1: f64,
}

#[derive(Debug, Clone, PartialEq)]
#[napi(object)]
pub struct PageLayout {
  pub page: u32,
  /// CropBox (fallback MediaBox) size in points. Coordinates in `words`,
  /// `lines` and `images` are raw user space, not shifted to this box's origin.
  pub width: f64,
  pub height: f64,
  /// `/Rotate`, reported and not applied.
  pub rotate: u32,
  /// Clockwise degrees (0, 90, 180, 270) the OCR tiers turned the page so its
  /// text reads upright: the turn carrying the most confident OCR text across
  /// the page's images (the tiles of a tiled scan vote together). Every word,
  /// line and image placement is expressed in that turned frame, and `width`
  /// and `height` swap for a quarter turn. An image with more than a few words
  /// that reads upright at a different turn is named in a warning: its words
  /// read sideways in the page frame. The render tier reads the page as
  /// displayed, so its turn includes `/Rotate`. Always 0 for `Native` pages.
  /// For any page, displayed coordinates are this frame turned clockwise by
  /// `rotate - contentRotation` (mod 360).
  pub content_rotation: u32,
  /// Deepest cascade tier that ran: `Native`, or `Ocr` / `Rendered` on a page
  /// whose native words (if any) were kept alongside the OCR words.
  pub source: TextSource,
  pub words: Vec<TextWord>,
  pub lines: Vec<TextLine>,
  pub images: Vec<PlacedImage>,
  pub warnings: Vec<String>,
}
