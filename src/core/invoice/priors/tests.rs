use super::super::amounts::Separator;
use super::super::dates::Order;
use super::super::segments::label_hits;
use super::super::testing::{page, segments_at};
use super::*;
use crate::types::{CurrencySource, InvoiceOptions};

fn infer_of(segs: &[Segment], opts: Option<&InvoiceOptions>) -> Priors {
  infer(segs, &label_hits(segs), opts)
}

#[test]
fn currency_comes_from_the_page_then_the_option_then_nothing() {
  let usd = segments_at(&page(&[(
    400.0,
    700.0,
    10.0,
    false,
    "Sub Total USD $50.00",
  )]));
  let p = infer_of(&usd, None);
  assert_eq!(p.currency.as_deref(), Some("USD"));
  assert_eq!(p.currency_source, CurrencySource::Page);

  let glyph = segments_at(&page(&[(400.0, 700.0, 10.0, false, "Total $93.50")]));
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

  let euro = segments_at(&page(&[(
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
  let es = segments_at(&page(&[
    (400.0, 700.0, 10.0, false, "FECHA: 08/07/2014"),
    (400.0, 686.0, 10.0, false, "CLIENTE: Skyline Charter"),
    (400.0, 300.0, 10.0, false, "TOTAL: 400.00"),
  ]));
  let p = infer(&es, &label_hits(&es), None);
  assert_eq!(
    p.order,
    Order::Dmy,
    "Spanish labels read an ambiguous date day-first"
  );
  let shared = segments_at(&page(&[
    (400.0, 700.0, 10.0, false, "FECHA: 08/07/2014"),
    (400.0, 686.0, 10.0, false, "CLIENTE: Skyline Charter"),
    (400.0, 400.0, 10.0, false, "TOTAL MANO DE OBRA 400.00"),
    (400.0, 386.0, 10.0, false, "TOTAL HORAS 0.00"),
    (400.0, 372.0, 10.0, false, "TOTAL OTROS 0.00"),
    (400.0, 358.0, 10.0, false, "TOTAL 400.00"),
  ]));
  assert_eq!(
    infer(&shared, &label_hits(&shared), None).order,
    Order::Dmy,
    "`Total` is the same word in several languages and does not vote"
  );
  let en = segments_at(&page(&[
    (400.0, 700.0, 10.0, false, "Date: 08/07/2014"),
    (400.0, 686.0, 10.0, false, "Bill To: Skyline Charter"),
  ]));
  assert_eq!(infer(&en, &label_hits(&en), None).order, Order::Mdy);
  let de = segments_at(&page(&[
    (400.0, 700.0, 10.0, false, "Zwischensumme 1.234,56"),
    (400.0, 686.0, 10.0, false, "MwSt. 234,57"),
    (400.0, 672.0, 10.0, false, "Datum: 31.08.2015"),
  ]));
  let p = infer_of(&de, None);
  assert_eq!(p.separator, Separator::Comma);
  assert_eq!(p.order, Order::Dmy);
  let us = segments_at(&page(&[(
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
