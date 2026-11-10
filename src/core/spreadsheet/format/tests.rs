use super::*;

#[test]
fn renders_the_corpus_codes_and_the_standard_ones() {
  let r = |code: &str, v: f64| NumberFormat::parse(code).unwrap().render(v, false).unwrap();
  assert_eq!(r("#,##0.00\"/hr\"", 142.5), "142.50/hr");
  assert_eq!(r("0.0", 3.0), "3.0");
  assert_eq!(r("#,##0.00", 1234.5), "1,234.50");
  assert_eq!(r("0.00%", 0.075), "7.50%");
  assert_eq!(r("$#,##0.00;($#,##0.00)", -5.0), "($5.00)");
  assert_eq!(
    r(
      "_($* #,##0.00_);_($* (#,##0.00);_($* \"-\"??_);_(@_)",
      1234.5
    ),
    "$1,234.50"
  );
  assert_eq!(r("[Red]0.00", -1.5), "-1.50");
  assert_eq!(r("00000", 123.0), "00123");
  assert_eq!(r("mm/dd/yy", 42247.0), "08/31/15");
  assert_eq!(r("m/d/yyyy", 42247.0), "8/31/2015");
  assert_eq!(r("yyyy-mm-dd hh:mm:ss", 45161.5), "2023-08-23 12:00:00");
  assert_eq!(r("h:mm AM/PM", 45161.75), "6:00 PM");
  assert_eq!(r("d-mmm-yy", 42247.0), "31-Aug-15");
  assert_eq!(r("@", 7.0), "7");
  assert!(
    NumberFormat::parse("[h]:mm:ss").is_none(),
    "elapsed time is unsupported"
  );
  assert!(NumberFormat::parse("0.00E+00").is_none());
  assert!(
    NumberFormat::parse("# ?/?").is_none(),
    "fractions are unsupported"
  );
  assert!(
    NumberFormat::parse("# ??/16").is_none(),
    "fixed-denominator fractions are unsupported"
  );
  assert!(
    NumberFormat::parse("m/d/yyyy").is_some(),
    "a slash between date tokens is a separator"
  );
  assert!(
    NumberFormat::parse("0.00\"/hr\"").is_some(),
    "a quoted slash is a literal"
  );
  assert!(
    NumberFormat::parse("[$-409]mmmm d, yyyy").is_none(),
    "locale directives are unsupported"
  );
  assert!(NumberFormat::parse("mm/dd/yy").unwrap().is_date());
  assert!(
    !NumberFormat::parse("#,##0.00\"/hr\"").unwrap().is_date(),
    "a quoted h is a literal"
  );
}

#[test]
fn review_findings_hostile_and_unusual_codes() {
  let r = |code: &str, v: f64| NumberFormat::parse(code).unwrap().render(v, false).unwrap();
  // A bare `a` followed by a multi-byte character is a literal, never a byte-sliced panic.
  assert_eq!(r("0 a€", 5.0), "5 a€");
  assert_eq!(r("0 aaé", 5.0), "5 aaé");
  assert_eq!(r("0 A中", 5.0), "5 A中");
  // Digits inside colour tags and after padding markers are not placeholders.
  assert_eq!(r("[Color10]0.00", 1.5), "1.50");
  assert_eq!(r("[Color50]#,##0", 1234.0), "1,234");
  assert_eq!(r("_00.00", 1.5), "1.50");
  assert_eq!(r("*00.00", 1.5), "1.50");
  // Scaling commas after the decimals divide by a thousand each.
  assert_eq!(r("0.0,,", 1234567.0), "1.2");
  assert_eq!(r("#,##0.0,", 1234567.0), "1,234.6");
  assert_eq!(r("#,##0,,\"M\"", 2500000.0), "3M");
  // Points and commas inside a date code are separators.
  assert_eq!(r("dd.mm.yyyy", 42247.0), "31.08.2015");
  assert_eq!(r("mmmm d, yyyy", 42247.0), "August 31, 2015");
  assert_eq!(r("yyyy.mm.dd hh:mm", 45161.5), "2023.08.23 12:00");
  assert!(NumberFormat::parse("dd.mm.yyyy").unwrap().is_date());
  // Excel caps a format code at 255 characters; longer codes are unsupported, not slow.
  assert!(NumberFormat::parse(&"0".repeat(255)).is_some());
  assert!(NumberFormat::parse(&"0".repeat(256)).is_none());
  // Random codes over the whole token alphabet, multi-byte characters included, never panic.
  let alphabet: Vec<char> = "0#?.,%;\"\\[]_*@/:-+ ()$€中😀aAmMdDhHsSyYeEgGbBpP"
    .chars()
    .collect();
  let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
  let mut next = || {
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;
    state
  };
  for _ in 0..4000 {
    let len = (next() % 24) as usize;
    let code: String = (0..len)
      .map(|_| alphabet[(next() % alphabet.len() as u64) as usize])
      .collect();
    if let Some(parsed) = NumberFormat::parse(&code) {
      for v in [0.0, -1.5, 0.075, 42247.0, 1e300, -1e-300] {
        let _ = parsed.render(v, false);
        let _ = parsed.render(v, true);
      }
    }
  }
}

#[test]
fn currency_symbols_and_time_only_codes() {
  let f = |code: &str| NumberFormat::parse(code).unwrap();
  // A bracketed currency with a locale id is that currency; the id names a language we ignore.
  assert_eq!(
    f("[$€-407]#,##0.00").render(1234.5, false).as_deref(),
    Some("€1,234.50")
  );
  assert_eq!(
    f("[$€-407]#,##0.00").currency_symbol().as_deref(),
    Some("€")
  );
  assert_eq!(f("[$USD] #,##0").currency_symbol().as_deref(), Some("USD"));
  assert_eq!(
    f("\"USD \"#,##0.00").currency_symbol().as_deref(),
    Some("USD")
  );
  assert_eq!(
    f("$#,##0.00;($#,##0.00)").currency_symbol().as_deref(),
    Some("$")
  );
  assert_eq!(f("£#,##0").currency_symbol().as_deref(), Some("£"));
  assert_eq!(f("#,##0.00\"/hr\"").currency_symbol(), None);
  assert_eq!(f("0.00%").currency_symbol(), None);
  assert_eq!(
    f("_($* #,##0.00_);_($* (#,##0.00);_($* \"-\"??_);_(@_)")
      .currency_symbol()
      .as_deref(),
    Some("$")
  );
  assert!(
    NumberFormat::parse("[$-409]mmmm d, yyyy").is_none(),
    "a bare locale id is still unsupported"
  );
  // Time-only codes carry a clock and no date.
  assert!(f("h:mm").is_date());
  assert!(!f("h:mm").has_date_part());
  assert!(!f("h:mm AM/PM").has_date_part());
  assert!(!f("mm:ss").has_date_part());
  assert!(f("m/d/yyyy h:mm").has_date_part());
  assert_eq!(f("h:mm").render(0.75, false).as_deref(), Some("18:00"));
  assert_eq!(
    f("h:mm AM/PM").render(60.5, false).as_deref(),
    Some("12:00 PM"),
    "the clock needs no valid date"
  );
  assert_eq!(iso_time(0.5), "12:00:00");
  assert_eq!(iso_time(45161.75), "18:00:00");
  assert_eq!(
    iso_time(0.999_999),
    "00:00:00",
    "rounds up to the next day like Excel"
  );
}

#[test]
fn general_and_serials() {
  assert_eq!(render_general(3.0), "3");
  assert_eq!(render_general(1234.567), "1234.567");
  assert_eq!(render_general(0.1 + 0.2), "0.3");
  assert_eq!(render_general(-0.0), "0");
  assert_eq!(render_general(999_999_999_999_999.0), "999999999999999");
  assert_eq!(render_general(1e15), "1E+15");
  assert_eq!(render_general(123_456_789_012_345_680_000.0), "1.23457E+20");
  assert_eq!(render_general(-2.5e20), "-2.5E+20");
  assert_eq!(render_general(1e-10), "0.0000000001");
  assert_eq!(render_general(1.5e-12), "1.5E-12");
  assert_eq!(render_general(9.9999999e-12), "1E-11");
  assert_eq!(
    iso_date(42247.0, false, false).as_deref(),
    Some("2015-08-31")
  );
  assert_eq!(
    iso_date(45161.5, false, true).as_deref(),
    Some("2023-08-23T12:00:00")
  );
  assert_eq!(iso_date(59.0, false, false).as_deref(), Some("1900-02-28"));
  assert_eq!(iso_date(60.0, false, false), None);
  assert_eq!(iso_date(61.0, false, false).as_deref(), Some("1900-03-01"));
  assert_eq!(iso_date(0.0, true, false).as_deref(), Some("1904-01-01"));
  assert_eq!(iso_date(-1.0, false, false), None);
}
