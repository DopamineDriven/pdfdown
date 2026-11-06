//! The layout track: invoice fields read from `PageLayout` pages, whatever produced them (a
//! native PDF, the OCR cascade, a workbook's logical pages, or a consumer's own layout call).
//! This file is the engine's entry; the modules beside it do the reading.

use super::anchors::{Candidate, candidates, score};
use super::dates::parse_date;
use super::identifiers::{parse_identifier, parse_registration, parse_serial};
use super::lexicon::Field;
use super::lexicon::Hit;
use super::parties::{Party, party_block, vendor};
use super::priors::{Priors, infer};
use super::segments::{Segment, label_hits, segments, span_of};
use crate::types::{
  AddressField, AircraftReference, DateField, FieldSpan, InvoiceDocumentType, InvoiceFields,
  InvoiceOptions, InvoiceResult, InvoiceSource, PageLayout, StringField, TextSource,
};

pub(crate) const NO_WORDS_WARNING: &str = "no words on the first page; nothing to read";
/// A field scoring under this is withheld (counted in a warning) rather than reported.
pub(crate) const CONFIDENCE_FLOOR: f64 = 0.3;

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

/// A value a validator accepted from a candidate: the normalised value, the validator's score,
/// and how many leading tokens of the candidate it used.
struct Accepted {
  value: String,
  validator: f64,
  tokens_used: usize,
}

/// The engine's working state for one document.
struct Reader<'a> {
  pages: &'a [PageLayout],
  first: Vec<Segment>,
  hits: Vec<Vec<Hit>>,
  priors: Priors,
  opts: Option<&'a InvoiceOptions>,
  warnings: Vec<String>,
  withheld: usize,
}

impl<'a> Reader<'a> {
  fn page(&self, number: u32) -> Option<&'a PageLayout> {
    self.pages.iter().find(|p| p.page == number)
  }

  fn preferred_lang(&self, lang: &str) -> bool {
    match self.opts.and_then(|o| o.languages.as_ref()) {
      Some(langs) => langs.iter().any(|l| l.eq_ignore_ascii_case(lang)),
      None => lang == "en",
    }
  }

  /// The best candidate for `field` under `validate`, which may accept the whole candidate text
  /// or a shorter run of its leading tokens. Returns the value, its text, score and span.
  fn best(
    &mut self,
    field: Field,
    validate: &dyn Fn(&str) -> Option<(String, f64)>,
  ) -> Option<(String, String, f64, FieldSpan)> {
    let cands = candidates(&self.first, &self.hits, field);
    let mut picked: Option<(f64, &Candidate, Accepted)> = None;
    let mut seen: Vec<String> = Vec::new();
    for c in &cands {
      let tokens: Vec<&str> = c.text.split_whitespace().collect();
      let mut accepted: Option<Accepted> = None;
      for k in (1..=tokens.len()).rev() {
        let text = tokens[..k].join(" ");
        if let Some((value, validator)) = validate(&text) {
          accepted = Some(Accepted {
            value,
            validator,
            tokens_used: k,
          });
          break;
        }
      }
      let Some(a) = accepted else { continue };
      let mut total = score(
        c.label_score,
        a.validator,
        c.geometry_score,
        c.ocr_confidence,
      );
      if self.preferred_lang(c.label.lang) {
        total += 0.01;
      }
      if self.opts.and_then(|o| o.include_candidates) == Some(true) {
        seen.push(format!("{} ({:.2})", a.value, total));
      }
      if picked.as_ref().is_none_or(|(s, _, _)| total > *s) {
        picked = Some((total, c, a));
      }
    }
    if !seen.is_empty() {
      self
        .warnings
        .push(format!("candidates for {field:?}: {}", seen.join(" | ")));
    }
    let (total, c, a) = picked?;
    let confidence = total.min(0.99);
    if confidence < CONFIDENCE_FLOOR {
      self.withheld += 1;
      return None;
    }
    let page = self.page(c.page)?;
    let used = if a.tokens_used >= c.words.len() {
      c.words.clone()
    } else {
      c.words[..a.tokens_used].to_vec()
    };
    let text = c
      .text
      .split_whitespace()
      .take(a.tokens_used)
      .collect::<Vec<_>>()
      .join(" ");
    Some((a.value, text, confidence, span_of(page, &used)))
  }

  fn string_field(
    &mut self,
    field: Field,
    validate: &dyn Fn(&str) -> Option<String>,
  ) -> Option<StringField> {
    self
      .best(field, &|t| validate(t).map(|v| (v, 1.0)))
      .map(|(value, text, confidence, span)| StringField {
        value,
        text,
        confidence,
        span: Some(span),
      })
  }

  fn date_field(&mut self, field: Field) -> Option<DateField> {
    let order = self.priors.order;
    self
      .best(field, &|t| {
        parse_date(t, order).map(|(v, unambiguous)| (v, if unambiguous { 1.0 } else { 0.8 }))
      })
      .map(|(value, text, confidence, span)| DateField {
        value,
        text,
        confidence,
        span: Some(span),
      })
  }
}

fn party_fields(
  party: Option<Party>,
) -> (
  Option<StringField>,
  Option<AddressField>,
  Option<StringField>,
  Option<StringField>,
) {
  match party {
    Some(p) => (Some(p.name), p.address, p.email, p.phone),
    None => (None, None, None, None),
  }
}

/// Header fields from pages.
pub(crate) fn extract_from_pages(
  pages: &[PageLayout],
  opts: Option<&InvoiceOptions>,
) -> InvoiceResult {
  let source = source_of(pages);
  let mut warnings = Vec::new();
  let Some(first_page) = pages.iter().find(|p| !p.words.is_empty()) else {
    warnings.push(NO_WORDS_WARNING.to_string());
    return InvoiceResult {
      source,
      document_type: InvoiceDocumentType::Unknown,
      confidence: 0.0,
      fields: InvoiceFields::default(),
      warnings,
      embedded_xml: None,
      currency_source: None,
      credit_signals: Vec::new(),
    };
  };
  if pages.first().is_some_and(|p| p.words.is_empty()) {
    warnings.push(NO_WORDS_WARNING.to_string());
  }
  let first = segments(first_page);
  let hits = label_hits(&first);
  let priors = infer(&first, &hits, opts);
  let mut reader = Reader {
    pages,
    first,
    hits,
    priors,
    opts,
    warnings,
    withheld: 0,
  };

  let invoice_id = reader.string_field(Field::InvoiceId, &parse_identifier);
  let invoice_date = reader.date_field(Field::InvoiceDate);
  let due_date = reader.date_field(Field::DueDate);
  let purchase_order = reader.string_field(Field::PurchaseOrder, &parse_identifier);
  let work_order = reader.string_field(Field::WorkOrder, &parse_identifier);
  let registration = reader.string_field(Field::Registration, &parse_registration);
  let serial = reader.string_field(Field::Serial, &parse_serial);
  let engine_serial = reader.string_field(Field::EngineSerial, &parse_serial);
  let mut aircraft = Vec::new();
  if registration.is_some() || serial.is_some() || engine_serial.is_some() {
    let span = registration
      .as_ref()
      .or(serial.as_ref())
      .or(engine_serial.as_ref())
      .and_then(|f| f.span.clone());
    aircraft.push(AircraftReference {
      registration,
      serial_number: serial,
      engine_serial_number: engine_serial,
      span,
      ..AircraftReference::default()
    });
  }
  let vendor_party = party_block(&reader.first, &reader.hits, Field::Vendor)
    .or_else(|| vendor(&reader.first, &reader.hits, first_page.height));
  let (vendor_name, vendor_address, vendor_email, vendor_phone) = party_fields(vendor_party);
  let (customer_name, customer_address, _, _) =
    party_fields(party_block(&reader.first, &reader.hits, Field::BillTo));
  let (_, shipping_address, _, _) =
    party_fields(party_block(&reader.first, &reader.hits, Field::ShipTo));
  let (_, remittance_address, _, _) =
    party_fields(party_block(&reader.first, &reader.hits, Field::RemitTo));
  let fields = InvoiceFields {
    invoice_id,
    invoice_date,
    due_date,
    purchase_order,
    work_order,
    aircraft,
    vendor_name,
    vendor_address,
    vendor_email,
    vendor_phone,
    customer_name,
    customer_address,
    shipping_address,
    remittance_address,
    currency_code: reader.priors.currency.clone(),
    ..InvoiceFields::default()
  };

  let key: Vec<f64> = [
    fields.invoice_id.as_ref().map(|f| f.confidence),
    fields.invoice_date.as_ref().map(|f| f.confidence),
    fields.vendor_name.as_ref().map(|f| f.confidence),
  ]
  .into_iter()
  .flatten()
  .collect();
  let confidence = if key.is_empty() {
    0.0
  } else {
    key.iter().sum::<f64>() / key.len() as f64
  };
  let document_type = if key.is_empty() {
    InvoiceDocumentType::Unknown
  } else {
    InvoiceDocumentType::Invoice
  };
  let mut warnings = std::mem::take(&mut reader.warnings);
  if reader.withheld > 0 {
    warnings.push(format!(
      "{} field(s) withheld under the {CONFIDENCE_FLOOR} confidence floor",
      reader.withheld
    ));
  }
  InvoiceResult {
    source,
    document_type,
    confidence,
    fields,
    warnings,
    embedded_xml: None,
    currency_source: Some(reader.priors.currency_source),
    credit_signals: Vec::new(),
  }
}

#[cfg(test)]
mod tests {
  use super::super::testing::acme_page;
  use super::*;
  use crate::types::CurrencySource;

  #[test]
  fn the_header_fields_come_off_the_acme_page() {
    let r = extract_from_pages(&[acme_page()], None);
    let id = r.fields.invoice_id.expect("invoice id");
    assert_eq!(id.value, "A-10045");
    assert_eq!(id.span.unwrap().page, 1);
    assert!(id.confidence >= 0.8, "{}", id.confidence);
    assert_eq!(r.fields.invoice_date.unwrap().value, "2024-03-14");
    assert_eq!(r.fields.purchase_order.unwrap().value, "PO-778812");
    assert_eq!(r.fields.work_order.unwrap().value, "WO-5521");
    assert_eq!(
      r.fields.vendor_name.unwrap().value,
      "ACME AVIATION SERVICES"
    );
    assert_eq!(r.fields.customer_name.unwrap().value, "Skyline Charter LLC");
    assert_eq!(r.fields.aircraft.len(), 1);
    assert_eq!(
      r.fields.aircraft[0].registration.as_ref().unwrap().value,
      "N417SK"
    );
    assert_eq!(
      r.fields.aircraft[0].serial_number.as_ref().unwrap().value,
      "20311"
    );
    assert_eq!(r.currency_source, Some(CurrencySource::Unknown));
    assert!(!r.warnings.iter().any(|w| w.contains("not implemented")));
  }
}
