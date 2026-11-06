//! Content-derived priors, read off the first page in one pass: the currency, the day/month
//! order and the decimal separator, each with the tier that decided it. Nothing here knows a
//! vendor; the page's own evidence wins, then the caller's option, then a plain default.

use super::amounts::{CODES, Separator, infer_separator, parse_amount, symbol_code};
use super::dates::{Order, infer_order, parse_date};
use super::lexicon::{Field, Hit};
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

/// A currency the page states: an ISO code beside an amount, in parentheses, on a totals label
/// or in a `Currency` label, else an unambiguous glyph on or beside an amount.
fn page_currency(segments: &[Segment], hits: &[Vec<Hit>], separator: Separator) -> Option<String> {
  let mut glyph_code: Option<String> = None;
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
          && neighbour_is_amount(i)
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
    _ => Order::Mdy,
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
mod tests {
  use super::super::amounts::Separator;
  use super::super::dates::Order;
  use super::super::segments::{label_hits, segments};
  use super::super::testing::page;
  use super::*;
  use crate::types::{CurrencySource, InvoiceOptions};

  fn infer_of(segs: &[Segment], opts: Option<&InvoiceOptions>) -> Priors {
    infer(segs, &label_hits(segs), opts)
  }

  #[test]
  fn currency_comes_from_the_page_then_the_option_then_nothing() {
    let usd = segments(&page(&[(
      400.0,
      700.0,
      10.0,
      false,
      "Sub Total USD $50.00",
    )]));
    let p = infer_of(&usd, None);
    assert_eq!(p.currency.as_deref(), Some("USD"));
    assert_eq!(p.currency_source, CurrencySource::Page);

    let glyph = segments(&page(&[(400.0, 700.0, 10.0, false, "Total $93.50")]));
    let p = infer_of(&glyph, None);
    assert_eq!(p.currency, None);
    assert_eq!(p.currency_source, CurrencySource::Unknown);
    let opts = InvoiceOptions {
      currency: Some("USD".into()),
      ..Default::default()
    };
    let p = infer_of(&glyph, Some(&opts));
    assert_eq!(p.currency.as_deref(), Some("USD"));
    assert_eq!(p.currency_source, CurrencySource::Option);

    let euro = segments(&page(&[(
      400.0,
      700.0,
      10.0,
      false,
      "Gesamtbetrag 1.234,56 €",
    )]));
    let p = infer_of(&euro, None);
    assert_eq!(p.currency.as_deref(), Some("EUR"));
    assert_eq!(p.currency_source, CurrencySource::Page);
  }

  #[test]
  fn separator_and_order_come_from_unambiguous_tokens() {
    let de = segments(&page(&[
      (400.0, 700.0, 10.0, false, "Zwischensumme 1.234,56"),
      (400.0, 686.0, 10.0, false, "MwSt. 234,57"),
      (400.0, 672.0, 10.0, false, "Datum: 31.08.2015"),
    ]));
    let p = infer_of(&de, None);
    assert_eq!(p.separator, Separator::Comma);
    assert_eq!(p.order, Order::Dmy);
    let us = segments(&page(&[(
      400.0,
      700.0,
      10.0,
      false,
      "Date: 06/23/2018 Total $93.50",
    )]));
    let p = infer_of(&us, None);
    assert_eq!(p.separator, Separator::Point);
    assert_eq!(p.order, Order::Mdy);
  }
}
