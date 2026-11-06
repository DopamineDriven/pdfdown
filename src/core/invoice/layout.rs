//! The layout track: invoice fields read from `PageLayout` pages, whatever produced them (a
//! native PDF, the OCR cascade, a workbook's logical pages, or a consumer's own layout call).
//! This file is the engine's entry; the modules beside it do the reading.

use super::amounts::{Separator, parse_amount};
use super::anchors::{Candidate, MAX_VALUE_TOKENS, candidates, score};
use super::dates::parse_date;
use super::identifiers::{parse_identifier, parse_registration, parse_serial};
use super::lexicon::Field;
use super::lexicon::Hit;
use super::parties::{Party, party_block, vendor};
use super::priors::{Priors, infer};
use super::segments::{Segment, label_hits, segments, span_of};
use super::totals::{TotalCandidate, Totals, resolve};
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
  /// The last page with words when it is not the first: where multi-page invoices total up.
  last: Vec<Segment>,
  last_hits: Vec<Vec<Hit>>,
  priors: Priors,
  opts: Option<&'a InvoiceOptions>,
  warnings: Vec<String>,
  withheld: usize,
}

impl<'a> Reader<'a> {
  fn page(&self, index: usize) -> Option<&'a PageLayout> {
    self.pages.get(index)
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
      // A value is a handful of tokens at most; try the leading runs from the longest down.
      let tokens: Vec<&str> = c.text.split_whitespace().take(MAX_VALUE_TOKENS).collect();
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
    let page = self.page(c.page_index)?;
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

  /// Every amount a totals label points at, on the first and the last page.
  fn money_candidates(&self, field: Field) -> Vec<TotalCandidate> {
    let separator: Separator = self.priors.separator;
    let mut out = Vec::new();
    for (segs, hits) in [(&self.first, &self.hits), (&self.last, &self.last_hits)] {
      for c in candidates(segs, hits, field) {
        let tokens: Vec<&str> = c.text.split_whitespace().collect();
        // The whole text; else its last token, else its first. A long remainder is prose, and a
        // number at its end is an amount only when it is written with decimals (`Total hours
        // worked this period 12` is not).
        let mut attempts: Vec<(String, Vec<usize>)> = vec![(c.text.clone(), c.words.clone())];
        let last = (
          tokens.last().map(|t| (*t).to_string()).unwrap_or_default(),
          c.words.last().copied().into_iter().collect(),
        );
        if tokens.len() <= 3 {
          attempts.push(last);
          attempts.push((
            tokens.first().map(|t| (*t).to_string()).unwrap_or_default(),
            c.words.first().copied().into_iter().collect(),
          ));
        } else if has_decimals(&last.0) {
          attempts.push(last);
        }
        let Some((text, words, amount)) = attempts
          .into_iter()
          .find_map(|(text, words)| parse_amount(&text, separator).map(|a| (text, words, a)))
        else {
          continue;
        };
        let Some(page) = self.page(c.page_index) else {
          continue;
        };
        out.push(TotalCandidate {
          field,
          amount,
          text,
          span: span_of(page, &words),
          base: score(c.label_score, 1.0, c.geometry_score, c.ocr_confidence).min(0.99),
        });
      }
    }
    out
  }

  fn totals(&self) -> Totals {
    let mut items = Vec::new();
    for field in [Field::SubTotal, Field::Tax, Field::Total, Field::AmountDue] {
      items.extend(self.money_candidates(field));
    }
    resolve(&items, self.priors.currency.as_deref())
  }

  /// Credit titles in the top band of the first page.
  fn credit_titles(&self, page_height: f64) -> Vec<String> {
    self
      .first
      .iter()
      .zip(&self.hits)
      .filter(|(s, _)| s.y1 >= 0.75 * page_height)
      .filter(|(_, hits)| {
        hits
          .iter()
          .any(|h| h.start == 0 && h.score >= 0.8 && h.label.field == Field::CreditTitle)
      })
      .map(|(s, _)| format!("credit title: {}", s.text))
      .collect()
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

type PartyFields = (
  Option<StringField>,
  Option<AddressField>,
  Option<StringField>,
  Option<StringField>,
  Option<StringField>,
);

/// `(name, address, email, phone, id)`.
fn party_fields(party: Option<Party>) -> PartyFields {
  match party {
    Some(p) => (p.name, p.address, p.email, p.phone, p.id),
    None => (None, None, None, None, None),
  }
}

/// Whether a token ends in a decimal separator and two digits (`1,234.56`, `93,50`, `(8.00)`).
fn has_decimals(token: &str) -> bool {
  let digits: Vec<char> = token
    .trim_end_matches(|c: char| !c.is_ascii_digit())
    .chars()
    .collect();
  digits.len() >= 3
    && matches!(digits[digits.len() - 3], '.' | ',')
    && digits[digits.len() - 2..].iter().all(char::is_ascii_digit)
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
  {
    let mut numbers: Vec<u32> = pages.iter().map(|p| p.page).collect();
    numbers.sort_unstable();
    if numbers.windows(2).any(|w| w[0] == w[1]) {
      warnings.push(
        "page numbers repeat; spans carry the numbers as given and fields were read by position"
          .to_string(),
      );
    }
  }
  let first_index = pages.iter().position(|p| !p.words.is_empty()).unwrap_or(0);
  let first = segments(first_page, first_index);
  let hits = label_hits(&first);
  let priors = infer(&first, &hits, opts);
  let last_index = pages
    .iter()
    .rposition(|p| !p.words.is_empty())
    .filter(|&i| i != first_index);
  let last = last_index
    .map(|i| segments(&pages[i], i))
    .unwrap_or_default();
  let last_hits = label_hits(&last);
  let mut reader = Reader {
    pages,
    first,
    hits,
    last,
    last_hits,
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
  let (vendor_name, vendor_address, vendor_email, vendor_phone, _) = party_fields(vendor_party);
  let (customer_name, customer_address, _, _, customer_id) =
    party_fields(party_block(&reader.first, &reader.hits, Field::BillTo));
  let (_, shipping_address, _, _, _) =
    party_fields(party_block(&reader.first, &reader.hits, Field::ShipTo));
  let (_, remittance_address, _, _, _) =
    party_fields(party_block(&reader.first, &reader.hits, Field::RemitTo));
  let totals = reader.totals();
  let fields = InvoiceFields {
    sub_total: totals.sub_total,
    total_tax: totals.total_tax,
    invoice_total: totals.invoice_total,
    amount_due: totals.amount_due,
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
    customer_id,
    customer_address,
    shipping_address,
    remittance_address,
    currency_code: reader.priors.currency.clone(),
    ..InvoiceFields::default()
  };

  let mut credit_signals = reader.credit_titles(first_page.height);
  if totals.negative {
    let text = fields
      .invoice_total
      .as_ref()
      .map(|f| f.text.clone())
      .unwrap_or_default();
    credit_signals.push(format!("negative total: {text}"));
  }
  let key: Vec<f64> = [
    fields.invoice_id.as_ref().map(|f| f.confidence),
    fields.invoice_date.as_ref().map(|f| f.confidence),
    fields.vendor_name.as_ref().map(|f| f.confidence),
    fields.invoice_total.as_ref().map(|f| f.confidence),
    fields.amount_due.as_ref().map(|f| f.confidence),
  ]
  .into_iter()
  .flatten()
  .collect();
  let confidence = if key.is_empty() {
    0.0
  } else {
    (key.iter().sum::<f64>() / key.len() as f64 + if totals.reconciled { 0.1 } else { 0.0 })
      .min(0.99)
  };
  let document_type = if !credit_signals.is_empty() {
    InvoiceDocumentType::CreditNote
  } else if key.is_empty() {
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
    credit_signals,
  }
}

#[cfg(test)]
mod tests {
  use super::super::testing::acme_page;
  use super::*;
  use crate::types::CurrencySource;

  #[test]
  fn repeated_page_numbers_never_abort_the_host() {
    let mut empty = acme_page();
    empty.words.clear();
    empty.lines.clear();
    let r = extract_from_pages(&[empty, acme_page()], None);
    assert_eq!(r.fields.invoice_id.unwrap().value, "A-10045");
    assert!(
      r.warnings.iter().any(|w| w.contains("page number")),
      "{:?}",
      r.warnings
    );
  }

  #[test]
  fn everyday_lines_do_not_fabricate_fields() {
    use super::super::testing::page;
    let r = extract_from_pages(
      &[page(&[
        (50.0, 760.0, 14.0, true, "ACME AVIATION SERVICES"),
        (380.0, 744.0, 10.0, false, "Due Date: 01/31/2016"),
        (380.0, 730.0, 10.0, false, "Invoice Date: 01/25/2016"),
        (380.0, 716.0, 10.0, false, "Invoice No: A-1"),
        (380.0, 702.0, 10.0, false, "Vendor #: 4471"),
        (380.0, 688.0, 10.0, false, "Customer No: 10045"),
        (50.0, 660.0, 10.0, false, "Terms: Net 30"),
        (50.0, 640.0, 10.0, false, "Total hours worked on site 12"),
        (400.0, 300.0, 10.0, false, "Total"),
        (500.0, 300.0, 10.0, false, "$93.50"),
        (400.0, 286.0, 10.0, false, "Amount Due"),
        (500.0, 286.0, 10.0, false, "-$12.50"),
      ])],
      None,
    );
    assert_eq!(r.fields.invoice_date.unwrap().value, "2016-01-25");
    assert_eq!(r.fields.due_date.unwrap().value, "2016-01-31");
    assert_eq!(
      r.fields.vendor_name.unwrap().value,
      "ACME AVIATION SERVICES"
    );
    assert_eq!(r.fields.customer_id.unwrap().value, "10045");
    assert!(r.fields.customer_name.is_none());
    assert!(
      r.fields.sub_total.is_none(),
      "`Net 30` is terms, not a subtotal"
    );
    assert_eq!(
      r.fields.invoice_total.unwrap().value,
      93.5,
      "prose with a trailing number is not a total"
    );
    assert_eq!(
      r.document_type,
      InvoiceDocumentType::Invoice,
      "an overpaid balance is not a credit note"
    );
    assert!(r.credit_signals.is_empty());
  }

  #[test]
  fn a_bare_title_anchors_nothing_below_or_beside_it() {
    use super::super::testing::page;
    let r = extract_from_pages(
      &[page(&[
        (50.0, 760.0, 12.0, true, "INVOICE"),
        (50.0, 744.0, 10.0, false, "1200 Hangar Road"),
        (300.0, 760.0, 10.0, false, "03/14/2024"),
        (50.0, 700.0, 14.0, true, "ACME AVIATION SERVICES"),
      ])],
      None,
    );
    assert!(r.fields.invoice_id.is_none(), "{:?}", r.fields.invoice_id);
    assert!(r.fields.total_tax.is_none());
    let r = extract_from_pages(
      &[page(&[
        (50.0, 760.0, 12.0, true, "TAX INVOICE"),
        (50.0, 744.0, 10.0, false, "123 Main Street"),
      ])],
      None,
    );
    assert!(r.fields.invoice_id.is_none());
    assert!(r.fields.total_tax.is_none());
  }

  #[test]
  fn a_two_thousand_word_line_is_read_in_well_under_a_second() {
    use super::super::testing::page;
    let text: String = (0..400)
      .map(|i| format!("word{i} alpha beta Date gamma"))
      .collect::<Vec<_>>()
      .join(" ");
    let p = page(&[(10.0, 700.0, 10.0, false, &text)]);
    let started = std::time::Instant::now();
    let _ = extract_from_pages(&[p], None);
    assert!(
      started.elapsed().as_millis() < 1000,
      "{} ms",
      started.elapsed().as_millis()
    );
  }

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
