//! The layout track: invoice fields read from `PageLayout` pages, whatever produced them (a
//! native PDF, the OCR cascade, a workbook's logical pages, or a consumer's own layout call).
//! This file is the engine's entry; the modules beside it do the reading.

use crate::types::{
  InvoiceDocumentType, InvoiceFields, InvoiceOptions, InvoiceResult, InvoiceSource, PageLayout,
  TextSource,
};

pub(crate) const NO_WORDS_WARNING: &str = "no words on the first page; nothing to read";
pub(crate) const NOT_IMPLEMENTED_WARNING: &str = "header extraction is not implemented yet";

/// The tier the pages with words came from: one tier when they agree, `Mixed` when they differ,
/// `None` when no page has a word.
fn source_of(pages: &[PageLayout]) -> InvoiceSource {
  let mut tiers: Vec<TextSource> = pages
    .iter()
    .filter(|p| !p.words.is_empty())
    .map(|p| p.source)
    .collect();
  tiers.sort_by_key(|t| *t as u8);
  tiers.dedup();
  match tiers.as_slice() {
    [] => InvoiceSource::None,
    [TextSource::Native] => InvoiceSource::Native,
    [TextSource::Ocr] => InvoiceSource::Ocr,
    [TextSource::Rendered] => InvoiceSource::Rendered,
    [TextSource::Spreadsheet] => InvoiceSource::Spreadsheet,
    _ => InvoiceSource::Mixed,
  }
}

/// Header fields from pages. Until the reading modules land, the result carries the source and
/// a warning only.
pub(crate) fn extract_from_pages(
  pages: &[PageLayout],
  opts: Option<&InvoiceOptions>,
) -> InvoiceResult {
  let _ = opts;
  let mut warnings = Vec::new();
  if pages.first().is_none_or(|p| p.words.is_empty()) {
    warnings.push(NO_WORDS_WARNING.to_string());
  } else {
    warnings.push(NOT_IMPLEMENTED_WARNING.to_string());
  }
  InvoiceResult {
    source: source_of(pages),
    document_type: InvoiceDocumentType::Unknown,
    confidence: 0.0,
    fields: InvoiceFields::default(),
    warnings,
    embedded_xml: None,
    currency_source: None,
    credit_signals: Vec::new(),
  }
}
