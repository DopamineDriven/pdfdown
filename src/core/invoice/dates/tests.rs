use super::*;

fn iso(text: &str, order: Order) -> Option<(String, bool)> {
  parse_date(text, order)
}

#[test]
fn every_shape_the_corpus_prints() {
  assert_eq!(
    iso("January 25, 2016", Order::Mdy),
    Some(("2016-01-25".into(), true))
  );
  assert_eq!(
    iso("06/23/2018", Order::Mdy),
    Some(("2018-06-23".into(), true))
  );
  assert_eq!(
    iso("31.08.2015", Order::Mdy),
    Some(("2015-08-31".into(), true)),
    "a day over 12 overrides the order"
  );
  assert_eq!(
    iso("03/04/2024", Order::Mdy),
    Some(("2024-03-04".into(), false))
  );
  assert_eq!(
    iso("03/04/2024", Order::Dmy),
    Some(("2024-04-03".into(), false))
  );
  assert_eq!(
    iso("2022-09-15", Order::Dmy),
    Some(("2022-09-15".into(), true))
  );
  assert_eq!(
    iso("27MAR2026", Order::Mdy),
    Some(("2026-03-27".into(), true))
  );
  assert_eq!(
    iso("01-Nov-26", Order::Mdy),
    Some(("2026-11-01".into(), true))
  );
  assert_eq!(
    iso("1 Nov 2026", Order::Mdy),
    Some(("2026-11-01".into(), true))
  );
  assert_eq!(
    iso("November 1st, 2026", Order::Mdy),
    Some(("2026-11-01".into(), true))
  );
  assert_eq!(
    iso("2026/11/01", Order::Mdy),
    Some(("2026-11-01".into(), true))
  );
  assert_eq!(iso("Net 30", Order::Mdy), None);
  assert_eq!(iso("13/13/2024", Order::Mdy), None);
  assert_eq!(iso("12345", Order::Mdy), None);
}

#[test]
fn the_order_is_inferred_from_unambiguous_dates() {
  assert_eq!(
    infer_order(&["03/04/2024", "31/08/2015"], Order::Mdy),
    Order::Dmy
  );
  assert_eq!(infer_order(&["03/04/2024"], Order::Mdy), Order::Mdy);
  assert_eq!(infer_order(&["06/23/2018"], Order::Dmy), Order::Mdy);
}
