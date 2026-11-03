use napi_derive::napi;

/// Where a page's text came from. Present in every build; `Rendered` is only
/// produced when the `render` feature rasterised the page.
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
  /// Native: from the font descriptor and name. OCR: not reported.
  pub bold: Option<bool>,
  pub italic: Option<bool>,
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
  /// Clockwise degrees (0, 90, 180, 270) the OCR tiers had to turn the raster
  /// before its text read upright; `width` and `height` are swapped for a
  /// quarter turn. Always 0 for `Native` pages. Rotation is detected per image
  /// and this reports the last image that needed a turn: tier-2 words stay
  /// anchored at their own image placement, as painted or turned about its
  /// corner, so on a page whose images disagree, or whose image is painted
  /// away from the origin, words can lie outside the reported box. Every
  /// turned image adds a warning naming it.
  pub content_rotation: u32,
  pub source: TextSource,
  pub words: Vec<TextWord>,
  pub lines: Vec<TextLine>,
  pub images: Vec<PlacedImage>,
  pub warnings: Vec<String>,
}
