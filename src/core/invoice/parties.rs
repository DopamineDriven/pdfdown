//! Parties: the vendor from the top of the first page, the customer and the other addresses
//! from the block under their label. A block is the aligned lines under a label or a name until
//! a gap, another label or an amount; an email or phone inside it moves to its own field.

use super::amounts::{Separator, parse_amount};
use super::dates::{Order, parse_date};
use super::lexicon::{Field, Hit, is_document_title};
use super::segments::Segment;
use crate::types::{AddressField, FieldSpan, StringField};

const LINE_EM: f64 = 1.2;
const ALIGN_EM: f64 = 1.5;
const MAX_ADDRESS_LINES: usize = 5;
/// The vendor sits in this top share of the first page.
const TOP_BAND: f64 = 0.75;

#[derive(Debug, Clone)]
pub(crate) struct Party {
  pub name: StringField,
  pub address: Option<AddressField>,
  pub email: Option<StringField>,
  pub phone: Option<StringField>,
}

fn span(s: &Segment) -> FieldSpan {
  FieldSpan {
    page: s.page,
    x0: s.x0,
    y0: s.y0,
    x1: s.x1,
    y1: s.y1,
  }
}

fn union(spans: &[FieldSpan]) -> Option<FieldSpan> {
  let first = spans.first()?;
  Some(spans.iter().fold(first.clone(), |a, b| FieldSpan {
    page: a.page,
    x0: a.x0.min(b.x0),
    y0: a.y0.min(b.y0),
    x1: a.x1.max(b.x1),
    y1: a.y1.max(b.y1),
  }))
}

fn is_label(hits: &[Hit]) -> bool {
  hits
    .iter()
    .any(|h| h.start == 0 && h.score >= 0.8 && h.label.field != Field::DocumentTitle)
}

fn is_title(s: &Segment) -> bool {
  let tokens: Vec<&str> = s.tokens.iter().map(String::as_str).collect();
  is_document_title(&tokens)
}

fn is_email(text: &str) -> bool {
  let t = text.trim();
  t.contains('@') && !t.contains(' ') && t.rsplit('@').next().is_some_and(|d| d.contains('.'))
}

fn is_phone(text: &str) -> bool {
  let lower = text.to_ascii_lowercase();
  let digits = text.chars().filter(|c| c.is_ascii_digit()).count();
  lower.starts_with("phone")
    || lower.starts_with("tel")
    || lower.starts_with("fax")
    || (digits >= 7
      && text
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, ' ' | '+' | '(' | ')' | '-' | '.')))
}

fn is_web(text: &str) -> bool {
  let lower = text.to_ascii_lowercase();
  lower.starts_with("www.") || lower.starts_with("http")
}

/// A line that cannot be a name or an address line.
fn is_noise(s: &Segment, hits: &[Hit]) -> bool {
  is_label(hits)
    || is_title(s)
    || parse_date(&s.text, Order::Mdy).is_some()
    || parse_amount(&s.text, Separator::Point).is_some()
}

fn text_field(s: &Segment, confidence: f64) -> StringField {
  StringField {
    value: s.text.clone(),
    text: s.text.clone(),
    confidence,
    span: Some(span(s)),
  }
}

/// The aligned lines directly under `anchor` until a gap, a label, a title or an amount.
fn block_under<'a>(
  segments: &'a [Segment],
  hits: &[Vec<Hit>],
  anchor: &Segment,
) -> Vec<&'a Segment> {
  let mut lines: Vec<&Segment> = Vec::new();
  let mut last_y0 = anchor.y0;
  let mut below: Vec<(&Segment, &Vec<Hit>)> = segments
    .iter()
    .zip(hits)
    .filter(|(t, _)| t.page == anchor.page && t.y1 <= anchor.y0 + 1e-6)
    .filter(|(t, _)| (t.x0 - anchor.x0).abs() <= ALIGN_EM * anchor.font_size)
    .collect();
  below.sort_by(|a, b| b.0.y1.total_cmp(&a.0.y1));
  for (t, t_hits) in below {
    if last_y0 - t.y1 > 1.5 * LINE_EM * anchor.font_size.max(t.font_size) {
      break;
    }
    if is_label(t_hits) || is_title(t) || parse_amount(&t.text, Separator::Point).is_some() {
      break;
    }
    lines.push(t);
    last_y0 = t.y0;
    if lines.len() == MAX_ADDRESS_LINES + 2 {
      break;
    }
  }
  lines
}

/// A party from a name line and the lines under it.
fn party_from(name: &Segment, lines: &[&Segment], confidence: f64) -> Party {
  let mut address: Vec<&Segment> = Vec::new();
  let mut email = None;
  let mut phone = None;
  for l in lines {
    if is_email(&l.text) {
      email.get_or_insert_with(|| text_field(l, confidence));
    } else if is_phone(&l.text) {
      phone.get_or_insert_with(|| {
        let mut f = text_field(l, confidence);
        f.value = l
          .text
          .trim_start_matches(|c: char| c.is_ascii_alphabetic() || c == ':' || c == '.')
          .trim()
          .to_string();
        f
      });
    } else if is_web(&l.text) {
      // a website line is not an address line; the field itself is slice 2
    } else if address.len() < MAX_ADDRESS_LINES {
      address.push(l);
    }
  }
  let address_field = (!address.is_empty()).then(|| {
    let lines: Vec<String> = address.iter().map(|s| s.text.clone()).collect();
    AddressField {
      value: lines.join(", "),
      text: lines.join("\n"),
      confidence,
      span: union(&address.iter().map(|s| span(s)).collect::<Vec<_>>()),
      lines,
    }
  });
  Party {
    name: text_field(name, confidence),
    address: address_field,
    email,
    phone,
  }
}

/// The vendor: the largest-font line in the top band of the first page that is not a label, a
/// title, a date or an amount (bold breaks ties, then the topmost), with its address under it.
/// A name stands out: a fifth larger than the page's median font, bold, or two words or more;
/// and it is mostly letters, never a number with a unit.
fn looks_like_a_name(s: &Segment, median: f64) -> bool {
  let letters = s.text.chars().filter(|c| c.is_alphabetic()).count();
  let digits_or_punct = s
    .text
    .chars()
    .filter(|c| c.is_ascii_digit() || c.is_ascii_punctuation())
    .count();
  letters > 0
    && letters >= digits_or_punct
    && (s.font_size >= 1.2 * median || s.bold || s.tokens.len() >= 2)
}

pub(crate) fn vendor(segments: &[Segment], hits: &[Vec<Hit>], page_height: f64) -> Option<Party> {
  let first_page = segments.iter().map(|s| s.page).min()?;
  let mut sizes: Vec<f64> = segments
    .iter()
    .filter(|s| s.page == first_page)
    .map(|s| s.font_size)
    .collect();
  sizes.sort_by(f64::total_cmp);
  let median = sizes.get(sizes.len() / 2).copied().unwrap_or(0.0);
  let (name, _) = segments
    .iter()
    .zip(hits)
    .filter(|(s, _)| s.page == first_page && s.y1 >= TOP_BAND * page_height)
    .filter(|(s, h)| {
      !is_noise(s, h) && !is_email(&s.text) && !is_phone(&s.text) && !is_web(&s.text)
    })
    .filter(|(s, _)| looks_like_a_name(s, median))
    .max_by(|(a, _), (b, _)| {
      a.font_size
        .total_cmp(&b.font_size)
        .then(a.bold.cmp(&b.bold))
        .then(a.y1.total_cmp(&b.y1))
    })?;
  let lines = block_under(segments, hits, name);
  Some(party_from(name, &lines, 0.7))
}

/// The party under a `Bill To` / `Ship To` / `Remit To` / `From` label: the best-scoring label
/// hit on the page (topmost on a tie); the label's own remainder is the name when it has one,
/// else the first aligned line below.
pub(crate) fn party_block(segments: &[Segment], hits: &[Vec<Hit>], field: Field) -> Option<Party> {
  let mut found: Vec<(f64, usize, &Hit)> = Vec::new();
  for (i, segment_hits) in hits.iter().enumerate() {
    if let Some(hit) = segment_hits
      .iter()
      .find(|h| h.start == 0 && h.score >= 0.8 && h.label.field == field)
    {
      found.push((hit.score, i, hit));
    }
  }
  found.sort_by(|a, b| {
    b.0
      .total_cmp(&a.0)
      .then(segments[b.1].y1.total_cmp(&segments[a.1].y1))
  });
  for (_, i, hit) in found {
    let s = &segments[i];
    let lines = block_under(segments, hits, s);
    if !hit.remainder.is_empty() {
      let name = Segment {
        text: hit.remainder.clone(),
        tokens: hit
          .remainder
          .split_whitespace()
          .map(str::to_string)
          .collect(),
        words: s.words[hit.end.min(s.words.len())..].to_vec(),
        ..s.clone()
      };
      return Some(party_from(&name, &lines, 0.9));
    }
    let Some((name, rest)) = lines.split_first() else {
      continue;
    };
    let name_index = segments.iter().position(|t| std::ptr::eq(t, *name));
    let name_hits = name_index.map(|j| hits[j].as_slice()).unwrap_or(&[]);
    if is_noise(name, name_hits) {
      continue;
    }
    return Some(party_from(name, rest, 0.9));
  }
  None
}

#[cfg(test)]
mod tests {
  use super::super::lexicon::Field;
  use super::super::segments::{label_hits, segments};
  use super::super::testing::{acme_page, page};
  use super::*;

  #[test]
  fn the_vendor_is_the_largest_non_title_line_in_the_top_band_with_its_address() {
    let p = acme_page();
    let segs = segments(&p);
    let v = vendor(&segs, &label_hits(&segs), p.height).unwrap();
    assert_eq!(v.name.value, "ACME AVIATION SERVICES");
    assert_eq!(v.name.span.unwrap().page, 1);
    assert_eq!(
      v.address.unwrap().lines,
      ["1200 Hangar Road", "Wichita, KS 67209"]
    );

    let bare = page(&[
      (450.0, 760.0, 16.0, true, "INVOICE"),
      (380.0, 730.0, 10.0, false, "Invoice No: 1"),
    ]);
    let bare_segs = segments(&bare);
    assert!(vendor(&bare_segs, &label_hits(&bare_segs), bare.height).is_none());
  }

  #[test]
  fn a_party_block_is_the_aligned_lines_under_its_label() {
    let p = acme_page();
    let segs = segments(&p);
    let c = party_block(&segs, &label_hits(&segs), Field::BillTo).unwrap();
    assert_eq!(c.name.value, "Skyline Charter LLC");
    assert_eq!(
      c.address.unwrap().lines,
      ["88 Terminal Drive", "Denver, CO 80249"]
    );
    assert!(c.email.is_none() && c.phone.is_none());
  }
}
