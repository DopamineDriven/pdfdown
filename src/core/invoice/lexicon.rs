//! The label vocabulary: a reviewed table of spellings per field, each tagged with its language,
//! seeded from the service's retired parsers (1037.md §3.3) and the design (2026-11-01 §7.3).
//! Matching is on a normalised form that folds case, punctuation and OCR confusables, and is
//! scored exact > prefix > fuzzy. The table carries spellings, never where they were seen.

/// What a label names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Field {
  InvoiceId,
  InvoiceDate,
  DueDate,
  PurchaseOrder,
  BillTo,
  ShipTo,
  RemitTo,
  Vendor,
  SubTotal,
  Tax,
  Total,
  AmountDue,
  Terms,
  WorkOrder,
  Registration,
  Serial,
  EngineSerial,
  CreditTitle,
  DocumentTitle,
}

#[derive(Debug)]
pub(crate) struct Label {
  pub field: Field,
  pub text: &'static str,
  pub lang: &'static str,
}

const fn l(field: Field, text: &'static str, lang: &'static str) -> Label {
  Label { field, text, lang }
}

use Field::*;

pub(crate) const LABELS: &[Label] = &[
  // invoice number
  l(InvoiceId, "Invoice No", "en"),
  l(InvoiceId, "Invoice No.", "en"),
  l(InvoiceId, "Invoice Number", "en"),
  l(InvoiceId, "Invoice #", "en"),
  l(InvoiceId, "Invoice", "en"),
  l(InvoiceId, "Invoice Nr.", "en"),
  l(InvoiceId, "Invoice N°", "en"),
  l(InvoiceId, "Invoice Nbr", "en"),
  l(InvoiceId, "Tax Invoice No.", "en"),
  l(InvoiceId, "Customer Invoice #", "en"),
  l(InvoiceId, "Credit Memo #", "en"),
  l(InvoiceId, "Credit Memo No", "en"),
  l(InvoiceId, "Credit Note No", "en"),
  l(InvoiceId, "Credit Note #", "en"),
  l(InvoiceId, "Document No.", "en"),
  l(InvoiceId, "Billing #", "en"),
  l(InvoiceId, "Rechnung Nr.", "de"),
  l(InvoiceId, "Rechnungsnummer", "de"),
  l(InvoiceId, "Rechnungs-Nr.", "de"),
  l(InvoiceId, "Facture No", "fr"),
  l(InvoiceId, "Facture N°", "fr"),
  l(InvoiceId, "No de facture", "fr"),
  l(InvoiceId, "Numéro de facture", "fr"),
  l(InvoiceId, "Factura No", "es"),
  l(InvoiceId, "Número de factura", "es"),
  // invoice date
  l(InvoiceDate, "Invoice Date", "en"),
  l(InvoiceDate, "Date", "en"),
  l(InvoiceDate, "Date Issued", "en"),
  l(InvoiceDate, "Issue Date", "en"),
  l(InvoiceDate, "Date of Issue", "en"),
  l(InvoiceDate, "Document Date", "en"),
  l(InvoiceDate, "Billing Date", "en"),
  l(InvoiceDate, "Inv. Date", "en"),
  l(InvoiceDate, "Tax Point", "en"),
  l(InvoiceDate, "Datum", "de"),
  l(InvoiceDate, "Rechnungsdatum", "de"),
  l(InvoiceDate, "Date de facture", "fr"),
  l(InvoiceDate, "Fecha", "es"),
  l(InvoiceDate, "Fecha de factura", "es"),
  // due date
  l(DueDate, "Due Date", "en"),
  l(DueDate, "Payment Due", "en"),
  l(DueDate, "Pay By", "en"),
  l(DueDate, "Due", "en"),
  l(DueDate, "Fälligkeit", "de"),
  l(DueDate, "Fällig am", "de"),
  l(DueDate, "Échéance", "fr"),
  l(DueDate, "Date d'échéance", "fr"),
  // purchase order
  l(PurchaseOrder, "Purchase Order", "en"),
  l(PurchaseOrder, "Purchase Order No", "en"),
  l(PurchaseOrder, "Purchase Order Number", "en"),
  l(PurchaseOrder, "P.O. Number", "en"),
  l(PurchaseOrder, "P.O. No.", "en"),
  l(PurchaseOrder, "PO Number", "en"),
  l(PurchaseOrder, "PO#", "en"),
  l(PurchaseOrder, "PO #", "en"),
  l(PurchaseOrder, "PO No.", "en"),
  l(PurchaseOrder, "PO", "en"),
  l(PurchaseOrder, "Customer PO", "en"),
  l(PurchaseOrder, "Cust PO#", "en"),
  l(PurchaseOrder, "Cust. Ref.", "en"),
  l(PurchaseOrder, "Order Number", "en"),
  l(PurchaseOrder, "Order No.", "en"),
  l(PurchaseOrder, "Your Ref", "en"),
  l(PurchaseOrder, "Your Ref.", "en"),
  l(PurchaseOrder, "Y/Ref", "en"),
  l(PurchaseOrder, "Customer Reference", "en"),
  l(PurchaseOrder, "Customer Order No", "en"),
  l(PurchaseOrder, "PO Reference", "en"),
  l(PurchaseOrder, "Bestellnummer", "de"),
  l(PurchaseOrder, "Bestell-Nr.", "de"),
  l(PurchaseOrder, "Bon de commande", "fr"),
  l(PurchaseOrder, "No de commande", "fr"),
  // parties
  l(BillTo, "Bill To", "en"),
  l(BillTo, "Billed To", "en"),
  l(BillTo, "Invoice To", "en"),
  l(BillTo, "Invoiced To", "en"),
  l(BillTo, "Sold To", "en"),
  l(BillTo, "Customer", "en"),
  l(BillTo, "To", "en"),
  l(BillTo, "Rechnung an", "de"),
  l(BillTo, "Kunde", "de"),
  l(BillTo, "Facturé à", "fr"),
  l(BillTo, "Client", "fr"),
  l(BillTo, "Cliente", "es"),
  l(ShipTo, "Ship To", "en"),
  l(ShipTo, "Deliver To", "en"),
  l(ShipTo, "Delivery Address", "en"),
  l(ShipTo, "Lieferadresse", "de"),
  l(ShipTo, "Livré à", "fr"),
  l(RemitTo, "Remit To", "en"),
  l(RemitTo, "Pay To", "en"),
  l(RemitTo, "Remittance", "en"),
  l(Vendor, "From", "en"),
  l(Vendor, "Vendor", "en"),
  l(Vendor, "Supplier", "en"),
  l(Vendor, "Seller", "en"),
  // totals
  l(SubTotal, "Sub Total", "en"),
  l(SubTotal, "Subtotal", "en"),
  l(SubTotal, "Sub-Total", "en"),
  l(SubTotal, "Net", "en"),
  l(SubTotal, "Net Amount", "en"),
  l(SubTotal, "Total Excluding VAT", "en"),
  l(SubTotal, "Total Excluding Tax", "en"),
  l(SubTotal, "Zwischensumme", "de"),
  l(SubTotal, "Nettobetrag", "de"),
  l(SubTotal, "Netto", "de"),
  l(SubTotal, "Sous-total", "fr"),
  l(SubTotal, "Montant HT", "fr"),
  l(SubTotal, "Total avant taxes", "fr"),
  l(SubTotal, "Subtotal", "es"),
  l(Tax, "Tax", "en"),
  l(Tax, "Sales Tax", "en"),
  l(Tax, "VAT", "en"),
  l(Tax, "GST", "en"),
  l(Tax, "HST", "en"),
  l(Tax, "PST", "en"),
  l(Tax, "QST", "en"),
  l(Tax, "TPS", "fr"),
  l(Tax, "TVQ", "fr"),
  l(Tax, "TVA", "fr"),
  l(Tax, "MwSt", "de"),
  l(Tax, "MwSt.", "de"),
  l(Tax, "USt", "de"),
  l(Tax, "Umsatzsteuer", "de"),
  l(Tax, "IVA", "es"),
  l(Total, "Total", "en"),
  l(Total, "Invoice Total", "en"),
  l(Total, "Total Amount", "en"),
  l(Total, "Grand Total", "en"),
  l(Total, "Total Payable", "en"),
  l(Total, "Invoice Amount", "en"),
  l(Total, "Total Invoice", "en"),
  l(Total, "Total Charges", "en"),
  l(Total, "Total (USD)", "en"),
  l(Total, "Total (CAD)", "en"),
  l(Total, "Total (EUR)", "en"),
  l(Total, "Total (GBP)", "en"),
  l(Total, "Total USD", "en"),
  l(Total, "USD Total", "en"),
  l(Total, "Total in USD", "en"),
  l(Total, "Total Credit", "en"),
  l(Total, "Rechnungsbetrag", "de"),
  l(Total, "Gesamtbetrag", "de"),
  l(Total, "Gesamt", "de"),
  l(Total, "Brutto", "de"),
  l(Total, "Montant total", "fr"),
  l(Total, "Total TTC", "fr"),
  l(Total, "Montant", "fr"),
  l(Total, "Importe total", "es"),
  l(Total, "Importe", "es"),
  l(Total, "Totale", "es"),
  l(Total, "Valor total", "es"),
  l(AmountDue, "Amount Due", "en"),
  l(AmountDue, "Balance Due", "en"),
  l(AmountDue, "Balance", "en"),
  l(AmountDue, "Total Amount Due", "en"),
  l(AmountDue, "Total Due", "en"),
  l(AmountDue, "Please Pay", "en"),
  l(AmountDue, "Zu zahlen", "de"),
  l(AmountDue, "Zahlbetrag", "de"),
  l(AmountDue, "Montant dû", "fr"),
  l(AmountDue, "Net à payer", "fr"),
  // stop words
  l(Terms, "Terms", "en"),
  l(Terms, "Payment Terms", "en"),
  l(Terms, "Net 30", "en"),
  l(Terms, "Net 60", "en"),
  l(Terms, "Zahlungsbedingungen", "de"),
  // work orders
  l(WorkOrder, "Work Order", "en"),
  l(WorkOrder, "Work Order No", "en"),
  l(WorkOrder, "Work Order Number", "en"),
  l(WorkOrder, "Work Order #", "en"),
  l(WorkOrder, "WO", "en"),
  l(WorkOrder, "WO #", "en"),
  l(WorkOrder, "WO No.", "en"),
  l(WorkOrder, "W/O No.", "en"),
  l(WorkOrder, "Workorder", "en"),
  l(WorkOrder, "Sales Order", "en"),
  l(WorkOrder, "Shop Order", "en"),
  l(WorkOrder, "Job Number", "en"),
  l(WorkOrder, "Job Card", "en"),
  l(WorkOrder, "Job No.", "en"),
  l(WorkOrder, "Maintenance Order No.", "en"),
  l(WorkOrder, "Cust RO", "en"),
  l(WorkOrder, "Cust RO #", "en"),
  l(WorkOrder, "Auftragsnummer", "de"),
  // aircraft
  l(Registration, "Tail", "en"),
  l(Registration, "Tail No.", "en"),
  l(Registration, "Tail Number", "en"),
  l(Registration, "Tail #", "en"),
  l(Registration, "Registration", "en"),
  l(Registration, "Registration Number", "en"),
  l(Registration, "Aircraft Registration", "en"),
  l(Registration, "Reg", "en"),
  l(Registration, "Reg.", "en"),
  l(Registration, "A/C Reg.", "en"),
  l(Registration, "Aircraft Reg", "en"),
  l(Registration, "Immatriculation", "fr"),
  l(Serial, "Serial No.", "en"),
  l(Serial, "Serial Number", "en"),
  l(Serial, "Serial #", "en"),
  l(Serial, "S/N", "en"),
  l(Serial, "SN", "en"),
  l(Serial, "SN#", "en"),
  l(Serial, "A/C S/N", "en"),
  l(Serial, "AC Serial No.", "en"),
  l(Serial, "Aircraft Serial Number", "en"),
  l(Serial, "Aircraft SN", "en"),
  l(Serial, "MSN", "en"),
  l(Serial, "Airframe", "en"),
  l(Serial, "Seriennummer", "de"),
  l(Serial, "Numéro de série", "fr"),
  l(EngineSerial, "ESN", "en"),
  l(EngineSerial, "Engine Serial", "en"),
  l(EngineSerial, "Engine S/N", "en"),
  l(EngineSerial, "Engine SN", "en"),
  l(EngineSerial, "Engine Serial #", "en"),
  l(EngineSerial, "Engine Serial Number", "en"),
  // credit titles
  l(CreditTitle, "Credit Memo", "en"),
  l(CreditTitle, "Credit Note", "en"),
  l(CreditTitle, "Credit Memorandum", "en"),
  l(CreditTitle, "Debit Memo", "en"),
  l(CreditTitle, "Debit Note", "en"),
  l(CreditTitle, "Note de crédit", "fr"),
  l(CreditTitle, "Avoir", "fr"),
  l(CreditTitle, "Gutschrift", "de"),
  l(CreditTitle, "Nota de crédito", "es"),
  // document titles: what a vendor line is not
  l(DocumentTitle, "Invoice", "en"),
  l(DocumentTitle, "Tax Invoice", "en"),
  l(DocumentTitle, "Commercial Invoice", "en"),
  l(DocumentTitle, "Pro-Forma Invoice", "en"),
  l(DocumentTitle, "Proforma", "en"),
  l(DocumentTitle, "Form", "en"),
  l(DocumentTitle, "Statement", "en"),
  l(DocumentTitle, "Receipt", "en"),
  l(DocumentTitle, "Quote", "en"),
  l(DocumentTitle, "Quotation", "en"),
  l(DocumentTitle, "Estimate", "en"),
  l(DocumentTitle, "Purchase Order", "en"),
  l(DocumentTitle, "Packing Slip", "en"),
  l(DocumentTitle, "Remittance Advice", "en"),
  l(DocumentTitle, "Credit Memo", "en"),
  l(DocumentTitle, "Credit Note", "en"),
  l(DocumentTitle, "Rechnung", "de"),
  l(DocumentTitle, "Gutschrift", "de"),
  l(DocumentTitle, "Facture", "fr"),
  l(DocumentTitle, "Factura", "es"),
];

/// Lower-case, OCR confusables folded, punctuation and whitespace dropped. `#` survives, so
/// `Invoice #` and `Invoice` stay different labels.
pub(crate) fn normalize(text: &str) -> String {
  let mut out = String::with_capacity(text.len());
  for c in text.chars() {
    let folded = match c {
      '0' | 'ο' | 'о' => 'o',
      '1' | 'l' | '|' | 'I' | 'í' | 'ì' | 'ı' => 'i',
      '5' => 's',
      '#' => '#',
      c if c.is_alphanumeric() => c.to_lowercase().next().unwrap_or(c),
      _ => continue,
    };
    out.push(folded);
  }
  out
}

/// A label found in a segment: the row, how well it matched, the words it occupies
/// (`start..end`, `end` exclusive) and the text that follows it in the segment.
#[derive(Debug, Clone)]
pub(crate) struct Hit {
  pub label: &'static Label,
  pub score: f64,
  pub start: usize,
  pub end: usize,
  pub remainder: String,
}

/// Characters a value may be glued to a label with (`Invoice #13594027`, `No.2015-0831`).
const GLUE: &[char] = &['#', ':', '.', '-', '/'];

/// The labels with their normalised forms, computed once.
struct NormLabel {
  label: &'static Label,
  norm: String,
}

fn norm_labels() -> &'static [NormLabel] {
  static CACHE: std::sync::OnceLock<Vec<NormLabel>> = std::sync::OnceLock::new();
  CACHE.get_or_init(|| {
    LABELS
      .iter()
      .map(|label| NormLabel {
        label,
        norm: normalize(label.text),
      })
      .collect()
  })
}

/// One attempt to match a normalised label at word `start`, given the words' normalised forms:
/// the label must begin the normalised text of the words from `start`; a match may end inside a
/// word only at a glue character or a digit. Returns the exclusive word index after the label
/// and the remainder text.
fn match_at(
  words: &[&str],
  norms: &[String],
  start: usize,
  label_norm: &str,
) -> Option<(usize, String)> {
  let mut built_len = 0usize;
  let mut built = String::new();
  for i in start..words.len() {
    let before = built_len;
    let norm_word = &norms[i];
    built_len += norm_word.len();
    if built_len < label_norm.len() {
      built.push_str(norm_word);
      if !label_norm.starts_with(built.as_str()) {
        return None;
      }
      continue;
    }
    built.push_str(norm_word);
    if !built.starts_with(label_norm) {
      return None;
    }
    let consumed_in_word = label_norm.len() - before;
    if consumed_in_word == norm_word.len() {
      let rest = words[i + 1..].join(" ");
      return Some((i + 1, trim_glue(&rest)));
    }
    // Ends inside the word: find the raw split after `consumed_in_word` normalised bytes.
    let word = words[i];
    let mut seen = 0usize;
    let mut split: Option<usize> = None;
    for (idx, c) in word.char_indices() {
      if seen >= consumed_in_word {
        split = Some(idx);
        break;
      }
      seen += normalize(&c.to_string()).len();
    }
    let split = split?;
    let after = &word[split..];
    let next = after.chars().next()?;
    if !(GLUE.contains(&next) || next.is_ascii_digit()) {
      return None;
    }
    let mut rest = after.to_string();
    if i + 1 < words.len() {
      rest.push(' ');
      rest.push_str(&words[i + 1..].join(" "));
    }
    return Some((i + 1, trim_glue(&rest)));
  }
  None
}

fn trim_glue(text: &str) -> String {
  text
    .trim()
    .trim_start_matches(|c: char| GLUE.contains(&c) || c.is_whitespace())
    .trim()
    .to_string()
}

/// Bounded Levenshtein distance; `None` when it exceeds `max`.
fn edit_distance(a: &str, b: &str, max: usize) -> Option<usize> {
  let a: Vec<char> = a.chars().collect();
  let b: Vec<char> = b.chars().collect();
  if a.len().abs_diff(b.len()) > max {
    return None;
  }
  let mut prev: Vec<usize> = (0..=b.len()).collect();
  let mut cur = vec![0; b.len() + 1];
  for (i, ca) in a.iter().enumerate() {
    cur[0] = i + 1;
    let mut row_min = cur[0];
    for (j, cb) in b.iter().enumerate() {
      let cost = usize::from(ca != cb);
      cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
      row_min = row_min.min(cur[j + 1]);
    }
    if row_min > max {
      return None;
    }
    std::mem::swap(&mut prev, &mut cur);
  }
  (prev[b.len()] <= max).then_some(prev[b.len()])
}

/// Every label of `field` (any field when `None`) found in the segment's words, best first.
/// A label matching the longest stretch at a position scores 1.0; a shorter label that is a
/// prefix of a longer match there scores 0.8; a fuzzy match (one edit per five characters, whole
/// words only) scores 0.6. Stop words (`Terms`) never hit as a date.
pub(crate) fn find_labels(words: &[&str], field: Option<Field>) -> Vec<Hit> {
  let norms: Vec<String> = words.iter().map(|w| normalize(w)).collect();
  let labels = norm_labels();
  let mut hits: Vec<Hit> = Vec::new();
  for start in 0..words.len() {
    if norms[start].is_empty() {
      continue;
    }
    let first = norms[start].chars().next();
    // The longest label of any field matching here decides who is a prefix.
    let mut longest = 0usize;
    let mut exact: Vec<(&NormLabel, usize, String)> = Vec::new();
    for nl in labels {
      if nl.norm.chars().next() != first {
        continue;
      }
      if let Some((end, remainder)) = match_at(words, &norms, start, &nl.norm) {
        longest = longest.max(nl.norm.len());
        exact.push((nl, end, remainder));
      }
    }
    let any_exact = !exact.is_empty();
    for (nl, end, remainder) in exact {
      if field.is_some_and(|f| f != nl.label.field) {
        continue;
      }
      let score = if nl.norm.len() == longest { 1.0 } else { 0.8 };
      hits.push(Hit {
        label: nl.label,
        score,
        start,
        end,
        remainder,
      });
    }
    // Fuzzy: whole-word prefixes only, and only where nothing exact matched.
    if !any_exact && norms[start].len() >= 4 {
      for nl in labels {
        if field.is_some_and(|f| f != nl.label.field) {
          continue;
        }
        let max = nl.norm.len() / 5;
        if max == 0 {
          continue;
        }
        let mut built = String::new();
        for i in start..words.len() {
          built.push_str(&norms[i]);
          if built.len() > nl.norm.len() + max {
            break;
          }
          if edit_distance(&built, &nl.norm, max).is_some() {
            hits.push(Hit {
              label: nl.label,
              score: 0.6,
              start,
              end: i + 1,
              remainder: trim_glue(&words[i + 1..].join(" ")),
            });
            break;
          }
        }
      }
    }
  }
  // A date label never reads a terms line.
  if matches!(field, Some(InvoiceDate) | Some(DueDate)) && starts_with_terms(&hits, &norms) {
    hits.clear();
  }
  hits.sort_by(|a, b| {
    b.score
      .partial_cmp(&a.score)
      .unwrap_or(std::cmp::Ordering::Equal)
      .then(a.start.cmp(&b.start))
      .then(b.end.cmp(&a.end))
  });
  hits
}

/// Whether the segment opens with a terms stop word (checked against the full table, since the
/// hits at hand may already be filtered to one field).
fn starts_with_terms(_hits: &[Hit], norms: &[String]) -> bool {
  norm_labels()
    .iter()
    .filter(|nl| nl.label.field == Terms)
    .any(|nl| {
      let mut built = String::new();
      for n in norms {
        built.push_str(n);
        if built.len() >= nl.norm.len() {
          return built.starts_with(&nl.norm) && built.len() == nl.norm.len() || built == nl.norm;
        }
      }
      false
    })
}

/// Whether a segment opens with a terms stop word, from its unfiltered hits.
pub(crate) fn has_terms_at_start(hits: &[Hit]) -> bool {
  hits
    .iter()
    .any(|h| h.start == 0 && h.label.field == Terms && h.score >= 0.8)
}

/// Whether a segment is the document's title rather than a name: every word is a title word, or
/// the whole segment is a title row, or the segment ends with a title word (`… Form`).
pub(crate) fn is_document_title(words: &[&str]) -> bool {
  let titles: Vec<String> = LABELS
    .iter()
    .filter(|l| l.field == DocumentTitle)
    .map(|l| normalize(l.text))
    .collect();
  let whole = normalize(&words.join(" "));
  if titles.contains(&whole) {
    return true;
  }
  let word_norms: Vec<String> = words
    .iter()
    .map(|w| normalize(w))
    .filter(|w| !w.is_empty())
    .collect();
  if word_norms.is_empty() {
    return false;
  }
  let is_title_word = |w: &String| titles.iter().any(|t| t == w);
  word_norms.iter().all(is_title_word) || word_norms.last().is_some_and(is_title_word)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn normalisation_folds_case_punctuation_and_ocr_confusables() {
    assert_eq!(normalize("lnvoice Nο."), normalize("Invoice No."));
    assert_eq!(normalize("P.O. Number:"), normalize("po number"));
  }

  #[test]
  fn labels_hit_at_the_start_of_a_segment_and_before_a_separator() {
    let hits = find_labels(&["Invoice", "#13594027"], Some(Field::InvoiceId));
    assert_eq!(hits[0].label.text, "Invoice #");
    assert_eq!(
      hits[0].end, 2,
      "the glued `#13594027` word belongs to the label match"
    );
    assert_eq!(hits[0].remainder, "13594027");

    let hits = find_labels(&["Rechnung", "Nr.", "2015-0831"], None);
    assert_eq!(hits[0].label.lang, "de");
    assert_eq!(hits[0].label.field, Field::InvoiceId);
    assert_eq!(hits[0].remainder, "2015-0831");

    let due = find_labels(&["Total", "Due", "$93.50"], Some(Field::AmountDue));
    assert_eq!(due[0].score, 1.0);
    assert_eq!(due[0].remainder, "$93.50");
    let total = find_labels(&["Total", "Due", "$93.50"], Some(Field::Total));
    assert_eq!(
      total[0].score, 0.8,
      "`Total` is a prefix of the segment, not the whole label"
    );

    assert!(find_labels(&["Terms:", "Net", "30"], Some(Field::InvoiceDate)).is_empty());
    assert_eq!(
      find_labels(&["Invoice", "Number", "INV-3337"], Some(Field::InvoiceId))[0].remainder,
      "INV-3337"
    );
  }

  #[test]
  fn document_titles_are_known() {
    assert!(is_document_title(&["INVOICE"]));
    assert!(is_document_title(&["Maintenance", "Reimbursement", "Form"]));
    assert!(is_document_title(&["FACTURE", "/", "INVOICE"]));
    assert!(!is_document_title(&["ACME", "AVIATION", "SERVICES"]));
  }
}
