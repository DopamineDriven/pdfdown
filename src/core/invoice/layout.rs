//! The layout track: invoice fields read from `PageLayout` pages, whatever produced them (a
//! native PDF, the OCR cascade, a workbook's logical pages, or a consumer's own layout call).
//! This file is the engine's entry; the modules beside it do the reading.

use super::amounts::{Separator, parse_amount};
use super::anchors::{Candidate, MAX_VALUE_TOKENS, candidates, score};
use super::dates::parse_date;
use super::identifiers::{parse_identifier, parse_registration, parse_serial};
use super::lexicon::Hit;
use super::lexicon::{Field, PageRole, role_of_title};
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

/// The role a page's top band declares; a page without words has none, a page without a title is
/// the invoice's own. A title must look like one: set larger than the page's median text or
/// bold, which keeps a boxed form's `PURCHASE ORDER` header cell from naming the page. An invoice
/// title on the page wins over any other title.
fn page_role(page: &PageLayout, index: usize) -> Option<PageRole> {
  if page.words.is_empty() {
    return None;
  }
  let segs = segments(page, index);
  let mut sizes: Vec<f64> = segs.iter().map(|s| s.font_size).collect();
  sizes.sort_by(f64::total_cmp);
  let median = sizes.get(sizes.len() / 2).copied().unwrap_or(0.0);
  let mut role = PageRole::Invoice;
  for s in segs
    .iter()
    .filter(|s| s.y1 >= 0.75 * page.height && (s.bold || s.font_size >= 1.2 * median))
  {
    let words: Vec<&str> = s.tokens.iter().map(String::as_str).collect();
    match role_of_title(&words) {
      Some(PageRole::Invoice) => return Some(PageRole::Invoice),
      Some(other) => role = other,
      None => {}
    }
  }
  Some(role)
}

fn role_name(role: PageRole) -> &'static str {
  match role {
    PageRole::Invoice => "invoice",
    PageRole::PurchaseOrder => "purchase order",
    PageRole::Delivery => "delivery note",
    PageRole::Remittance => "remittance advice",
  }
}

/// Header fields from pages.
pub(crate) fn extract_from_pages(
  pages: &[PageLayout],
  opts: Option<&InvoiceOptions>,
) -> InvoiceResult {
  let source = source_of(pages);
  let mut warnings = Vec::new();
  // Pages play roles: the invoice's own pages carry the fields; our own purchase order, a
  // delivery note or a remittance advice appended to them do not, and are read only for the
  // order number. Roles only ever set pages aside when an invoice page remains: a document
  // that is nothing but such pages is read as it stands, with a warning.
  let roles: Vec<Option<PageRole>> = pages
    .iter()
    .enumerate()
    .map(|(i, p)| page_role(p, i))
    .collect();
  let invoice_pages: Vec<usize> = roles
    .iter()
    .enumerate()
    .filter(|(_, r)| matches!(r, Some(PageRole::Invoice)))
    .map(|(i, _)| i)
    .collect();
  let other_pages: Vec<(usize, PageRole)> = roles
    .iter()
    .enumerate()
    .filter_map(|(i, r)| match r {
      Some(PageRole::Invoice) | None => None,
      Some(role) => Some((i, *role)),
    })
    .collect();
  let only_other_pages = invoice_pages.is_empty() && !other_pages.is_empty();
  let readable: Vec<usize> = if invoice_pages.is_empty() {
    (0..pages.len()).filter(|&i| roles[i].is_some()).collect()
  } else {
    invoice_pages
  };
  let Some(&first_index) = readable.first() else {
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
  let first_page = &pages[first_index];
  if pages.first().is_some_and(|p| p.words.is_empty()) {
    warnings.push(NO_WORDS_WARNING.to_string());
  }
  if only_other_pages {
    warnings.push(format!(
      "every readable page is titled as a {}; it was read as the invoice regardless",
      role_name(other_pages[0].1)
    ));
  } else {
    for (i, role) in &other_pages {
      warnings.push(format!(
        "page {} is a {} and was not read as the invoice",
        pages[*i].page,
        role_name(*role)
      ));
    }
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
  let first = segments(first_page, first_index);
  let hits = label_hits(&first);
  let priors = infer(&first, &hits, opts);
  let last_index = readable.last().copied().filter(|&i| i != first_index);
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
  let mut purchase_order = reader.string_field(Field::PurchaseOrder, &parse_identifier);
  // An appended purchase order is the most authoritative source of its own number.
  if purchase_order.is_none() && !only_other_pages {
    for &(i, role) in &other_pages {
      if role != PageRole::PurchaseOrder {
        continue;
      }
      let segs = segments(&pages[i], i);
      let po_hits = label_hits(&segs);
      let mut po_reader = Reader {
        pages,
        first: segs,
        hits: po_hits,
        last: Vec::new(),
        last_hits: Vec::new(),
        priors: reader.priors.clone(),
        opts,
        warnings: Vec::new(),
        withheld: 0,
      };
      purchase_order = po_reader.string_field(Field::PurchaseOrder, &parse_identifier);
      if purchase_order.is_some() {
        break;
      }
    }
  }
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
  // A labelled party block comes from the first page, else the last: a cover page often names
  // the customer only in an unlabelled block while every itemisation page says `Bill To:`.
  let party = |field: Field| {
    party_block(&reader.first, &reader.hits, field)
      .or_else(|| party_block(&reader.last, &reader.last_hits, field))
  };
  let (customer_name, customer_address, _, _, customer_id) = party_fields(party(Field::BillTo));
  let (_, shipping_address, _, _, _) = party_fields(party(Field::ShipTo));
  let (_, remittance_address, _, _, _) = party_fields(party(Field::RemitTo));
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
mod tests;
