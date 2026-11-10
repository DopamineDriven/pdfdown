//! Content-derived priors, read off the first page in one pass: the currency, the day/month
//! order and the decimal separator, each with the tier that decided it. Nothing here knows a
//! vendor; the page's own evidence wins, then the caller's option, then a plain default.

use super::amounts::{CODES, Separator, infer_separator, parse_amount, symbol_code};
use super::dates::{Order, infer_order, parse_date};
use super::lexicon::{Field, Hit, language_specific};
use super::segments::Segment;
use crate::types::{CurrencySource, DateOrder, InvoiceOptions};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Priors {
  pub currency: Option<String>,
  pub currency_source: CurrencySource,
  pub order: Order,
  pub separator: Separator,
}

fn is_code(token: &str) -> Option<&'static str> {
  let t = token.trim_matches(|c: char| !c.is_ascii_alphabetic());
  CODES.iter().copied().find(|c| *c == t)
}

/// The day/month order the page's label language implies, when no option and no unambiguous
/// date decides: labels in Spanish, French, German, Italian, Portuguese or Dutch vote day-first,
/// English labels month-first; a tie or no labels falls to month-first. Only labels whose text
/// belongs to one language vote (`Total` and `Date` do not), and each field votes once per
/// language, so ten `TOTAL …` lines are not ten votes.
fn label_language_order(hits: &[Vec<Hit>]) -> Order {
  let mut voters: std::collections::HashSet<(&str, Field)> = std::collections::HashSet::new();
  for h in hits.iter().flatten() {
    if h.start == 0 && h.score >= 0.8 && language_specific(h.label) {
      voters.insert((h.label.lang, h.label.field));
    }
  }
  let day_first = voters
    .iter()
    .filter(|(lang, _)| matches!(*lang, "es" | "fr" | "de" | "it" | "pt" | "nl"))
    .count();
  let month_first = voters.iter().filter(|(lang, _)| *lang == "en").count();
  if day_first > month_first {
    Order::Dmy
  } else {
    Order::Mdy
  }
}

/// A currency the page states: an ISO code beside an amount, in parentheses, on a totals label
/// or in a `Currency` label, else an unambiguous glyph on or beside an amount, where "beside"
/// includes a glyph standing in its own column left of an amount column (`US$ | 400.00`).
fn page_currency(segments: &[Segment], hits: &[Vec<Hit>], separator: Separator) -> Option<String> {
  let mut glyph_code: Option<String> = None;
  let line_has_amount = |s: &Segment| {
    segments.iter().any(|t| {
      t.page_index == s.page_index
        && t.line == s.line
        && !std::ptr::eq(t, s)
        && t
          .tokens
          .iter()
          .any(|tok| parse_amount(tok, separator).is_some())
    })
  };
  for (s, segment_hits) in segments.iter().zip(hits) {
    let n = s.tokens.len();
    let totals_label = segment_hits.iter().any(|h| {
      h.start == 0
        && matches!(
          h.label.field,
          Field::Total | Field::SubTotal | Field::AmountDue | Field::Tax
        )
    });
    let labelled = s.text.to_ascii_lowercase().contains("currency");
    let neighbour_is_amount = |i: usize| {
      [i.wrapping_sub(1), i + 1]
        .iter()
        .filter(|&&j| j < n)
        .any(|&j| parse_amount(&s.tokens[j], separator).is_some())
    };
    for (i, token) in s.tokens.iter().enumerate() {
      if let Some(code) = is_code(token) {
        let parenthesised = token.starts_with('(') && token.ends_with(')');
        if neighbour_is_amount(i) || parenthesised || labelled || totals_label {
          return Some(code.to_string());
        }
      }
      if glyph_code.is_none() {
        if let Some(amount) = parse_amount(token, separator) {
          if let Some(code) = amount.code {
            return Some(code);
          }
          if let Some(code) = amount.symbol.as_deref().and_then(symbol_code) {
            glyph_code = Some(code.to_string());
          }
        } else if let Some(code) = symbol_code(token)
          && (neighbour_is_amount(i) || (n == 1 && line_has_amount(s)))
        {
          glyph_code = Some(code.to_string());
        }
      }
    }
  }
  glyph_code
}

/// Priors for a document from its first page's segments and the caller's options.
pub(crate) fn infer(
  segments: &[Segment],
  hits: &[Vec<Hit>],
  opts: Option<&InvoiceOptions>,
) -> Priors {
  let tokens: Vec<&str> = segments
    .iter()
    .flat_map(|s| s.tokens.iter().map(String::as_str))
    .collect();
  let sep_fallback = match opts.and_then(|o| o.decimal_separator.as_deref()) {
    Some(",") => Separator::Comma,
    _ => Separator::Point,
  };
  let separator = infer_separator(&tokens, sep_fallback);
  let order_fallback = match opts.and_then(|o| o.date_order) {
    Some(DateOrder::DMY) => Order::Dmy,
    Some(_) => Order::Mdy,
    None => label_language_order(hits),
  };
  let date_tokens: Vec<&str> = tokens
    .iter()
    .copied()
    .filter(|t| parse_date(t, order_fallback).is_some())
    .collect();
  let order = infer_order(&date_tokens, order_fallback);
  let (currency, currency_source) = match page_currency(segments, hits, separator) {
    Some(code) => (Some(code), CurrencySource::Page),
    None => match opts.and_then(|o| o.currency.as_deref()) {
      Some(code) if !code.trim().is_empty() => (
        Some(code.trim().to_ascii_uppercase()),
        CurrencySource::Option,
      ),
      _ => (None, CurrencySource::Unknown),
    },
  };
  Priors {
    currency,
    currency_source,
    order,
    separator,
  }
}

#[cfg(test)]
mod tests;
