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
