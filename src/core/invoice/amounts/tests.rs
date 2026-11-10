use super::*;
fn amt(text: &str, sep: Separator) -> Option<(f64, bool, Option<String>, Option<String>)> {
  parse_amount(text, sep).map(|a| (a.value, a.negative, a.code, a.symbol))
}

#[test]
fn a_leading_decimal_separator_has_a_zero_integer_part() {
  assert_eq!(parse_amount("$.00", Separator::Point).unwrap().value, 0.0);
  assert_eq!(parse_amount("$.50", Separator::Point).unwrap().value, 0.5);
  assert_eq!(parse_amount(",75", Separator::Comma).unwrap().value, 0.75);
  let credit = parse_amount("$.00-", Separator::Point).unwrap();
  assert_eq!(credit.value, 0.0);
  assert!(credit.negative);
  assert!(parse_amount(".", Separator::Point).is_none());
  assert!(parse_amount("$.", Separator::Point).is_none());
}

#[test]
fn every_shape_the_corpus_prints() {
  assert_eq!(
    amt("$1,234.56", Separator::Point),
    Some((1234.56, false, None, Some("$".into())))
  );
  assert_eq!(
    amt("USD $50.00", Separator::Point),
    Some((50.0, false, Some("USD".into()), Some("$".into())))
  );
  assert_eq!(
    amt("1.234,56 €", Separator::Comma),
    Some((1234.56, false, None, Some("€".into())))
  );
  assert_eq!(
    amt("EUR 1 234,56", Separator::Comma),
    Some((1234.56, false, Some("EUR".into()), None))
  );
  assert_eq!(
    amt("(123.45)", Separator::Point),
    Some((123.45, true, None, None))
  );
  assert_eq!(
    amt("123.45-", Separator::Point),
    Some((123.45, true, None, None))
  );
  assert_eq!(
    amt("$-12.50", Separator::Point),
    Some((12.5, true, None, Some("$".into())))
  );
  assert_eq!(
    amt("-$1,200.00", Separator::Point),
    Some((1200.0, true, None, Some("$".into())))
  );
  assert_eq!(
    amt("4 500,00", Separator::Comma),
    Some((4500.0, false, None, None))
  );
  assert_eq!(
    amt("CAD$ 5,173.88", Separator::Point),
    Some((5173.88, false, Some("CAD".into()), Some("$".into())))
  );
  assert_eq!(amt("12", Separator::Point), Some((12.0, false, None, None)));
  assert_eq!(
    amt("1,469.13", Separator::Comma).map(|a| a.0),
    Some(1469.13),
    "the token's own shape wins over the document convention"
  );
  assert_eq!(amt("INV-3337", Separator::Point), None);
  assert_eq!(amt("2015-0831", Separator::Point), None);
  assert_eq!(amt("06/23/2018", Separator::Point), None);
}

#[test]
fn the_separator_is_a_majority_vote_and_glyphs_map_only_when_unambiguous() {
  assert_eq!(
    infer_separator(&["1.234,56", "234,57", "1,469.13"], Separator::Point),
    Separator::Comma
  );
  assert_eq!(
    infer_separator(&["$85.00", "$8.50"], Separator::Comma),
    Separator::Point
  );
  assert_eq!(
    infer_separator(&["12", "INV-1"], Separator::Comma),
    Separator::Comma
  );
  assert_eq!(symbol_code("€"), Some("EUR"));
  assert_eq!(symbol_code("£"), Some("GBP"));
  assert_eq!(symbol_code("C$"), Some("CAD"));
  assert_eq!(symbol_code("US$"), Some("USD"));
  assert_eq!(symbol_code("R$"), Some("BRL"));
  assert_eq!(symbol_code("$"), None);
  let latin = parse_amount("US$ 400.00", Separator::Point).unwrap();
  assert_eq!(latin.value, 400.0);
  assert_eq!(latin.symbol.as_deref(), Some("US$"));
  assert_eq!(symbol_code("¥"), None);
  assert!(CODES.contains(&"USD") && CODES.contains(&"CHF") && CODES.contains(&"AED"));
}
