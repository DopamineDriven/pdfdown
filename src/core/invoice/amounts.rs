//! Money as the corpus prints it: a value, its sign, and the code or glyph beside it. The
//! decimal separator is inferred per document from the tokens that show it unambiguously.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Separator {
  Point,
  Comma,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Amount {
  pub value: f64,
  pub negative: bool,
  pub code: Option<String>,
  pub symbol: Option<String>,
}

/// Active ISO 4217 codes: the lookup covers the standard, not the sample.
pub(crate) const CODES: &[&str] = &[
  "AED", "AFN", "ALL", "AMD", "ANG", "AOA", "ARS", "AUD", "AWG", "AZN", "BAM", "BBD", "BDT", "BGN",
  "BHD", "BIF", "BMD", "BND", "BOB", "BRL", "BSD", "BTN", "BWP", "BYN", "BZD", "CAD", "CDF", "CHF",
  "CLP", "CNY", "COP", "CRC", "CUP", "CVE", "CZK", "DJF", "DKK", "DOP", "DZD", "EGP", "ERN", "ETB",
  "EUR", "FJD", "FKP", "GBP", "GEL", "GHS", "GIP", "GMD", "GNF", "GTQ", "GYD", "HKD", "HNL", "HTG",
  "HUF", "IDR", "ILS", "INR", "IQD", "IRR", "ISK", "JMD", "JOD", "JPY", "KES", "KGS", "KHR", "KMF",
  "KPW", "KRW", "KWD", "KYD", "KZT", "LAK", "LBP", "LKR", "LRD", "LSL", "LYD", "MAD", "MDL", "MGA",
  "MKD", "MMK", "MNT", "MOP", "MRU", "MUR", "MVR", "MWK", "MXN", "MYR", "MZN", "NAD", "NGN", "NIO",
  "NOK", "NPR", "NZD", "OMR", "PAB", "PEN", "PGK", "PHP", "PKR", "PLN", "PYG", "QAR", "RON", "RSD",
  "RUB", "RWF", "SAR", "SBD", "SCR", "SDG", "SEK", "SGD", "SHP", "SLE", "SOS", "SRD", "SSP", "STN",
  "SVC", "SYP", "SZL", "THB", "TJS", "TMT", "TND", "TOP", "TRY", "TTD", "TWD", "TZS", "UAH", "UGX",
  "USD", "UYU", "UZS", "VES", "VND", "VUV", "WST", "XAF", "XCD", "XOF", "XPF", "YER", "ZAR", "ZMW",
  "ZWL",
];

/// A glyph to its code only when the glyph names exactly one currency.
pub(crate) fn symbol_code(symbol: &str) -> Option<&'static str> {
  Some(match symbol {
    "€" => "EUR",
    "£" => "GBP",
    "CHF" => "CHF",
    "C$" | "CA$" => "CAD",
    "A$" | "AU$" => "AUD",
    "NZ$" => "NZD",
    "US$" => "USD",
    "R$" => "BRL",
    "S$" => "SGD",
    "HK$" => "HKD",
    "MX$" => "MXN",
    "NT$" => "TWD",
    "RD$" => "DOP",
    "₹" => "INR",
    "₽" => "RUB",
    "₩" => "KRW",
    "₪" => "ILS",
    "₺" => "TRY",
    "฿" => "THB",
    "₫" => "VND",
    "₦" => "NGN",
    "₴" => "UAH",
    "₱" => "PHP",
    _ => return None,
  })
}

const SYMBOLS: &[&str] = &[
  "CA$", "AU$", "NZ$", "US$", "HK$", "MX$", "NT$", "RD$", "C$", "A$", "R$", "S$", "$", "€", "£",
  "¥", "₹", "₽", "₩", "₪", "₺", "฿", "₫", "₦", "₴", "₱", "¢",
];

/// Strip a currency code or glyph from either end of the token.
fn strip_currency<'a>(
  mut t: &'a str,
  code: &mut Option<String>,
  symbol: &mut Option<String>,
) -> &'a str {
  for _ in 0..3 {
    let trimmed = t.trim();
    let mut changed = false;
    for c in CODES {
      if let Some(rest) = trimmed.strip_prefix(c)
        && !rest.starts_with(|ch: char| ch.is_ascii_alphabetic())
      {
        *code = Some((*c).to_string());
        t = rest;
        changed = true;
        break;
      }
      if let Some(rest) = trimmed.strip_suffix(c)
        && !rest.ends_with(|ch: char| ch.is_ascii_alphabetic())
      {
        *code = Some((*c).to_string());
        t = rest;
        changed = true;
        break;
      }
    }
    if changed {
      continue;
    }
    for s in SYMBOLS {
      if let Some(rest) = trimmed.strip_prefix(s) {
        *symbol = Some((*s).to_string());
        t = rest;
        changed = true;
        break;
      }
      if let Some(rest) = trimmed.strip_suffix(s) {
        *symbol = Some((*s).to_string());
        t = rest;
        changed = true;
        break;
      }
    }
    if !changed {
      break;
    }
  }
  t.trim()
}

/// The number part of a token, or `None` when it is not money: digits with at most one decimal
/// separator and thousands groups, in the document's convention unless the token shows its own.
fn parse_number(digits: &str, sep: Separator) -> Option<f64> {
  if digits.is_empty()
    || !digits
      .chars()
      .all(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | ' ' | '\u{a0}' | '\''))
  {
    return None;
  }
  // `.00` and `,50` print a zero integer part as nothing; read them as `0.00` and `0,50`.
  let padded;
  let digits = if digits.starts_with(['.', ',']) {
    padded = format!("0{digits}");
    padded.as_str()
  } else {
    digits
  };
  if !digits.chars().next()?.is_ascii_digit() || !digits.chars().last()?.is_ascii_digit() {
    return None;
  }
  let has_point = digits.contains('.');
  let has_comma = digits.contains(',');
  let decimal: Option<char> = match (has_point, has_comma) {
    (true, true) => Some(
      digits
        .rfind(['.', ','])
        .and_then(|i| digits[i..].chars().next())?,
    ),
    (true, false) | (false, true) => {
      let c = if has_point { '.' } else { ',' };
      let count = digits.matches(c).count();
      let after = digits.rsplit(c).next().map(str::len).unwrap_or(0);
      let doc_decimal = match sep {
        Separator::Point => '.',
        Separator::Comma => ',',
      };
      if count == 1 && after == 2 {
        Some(c)
      } else if count == 1 && after == 3 && c != doc_decimal {
        None
      } else if count == 1 && after != 3 {
        Some(c)
      } else if count == 1 {
        (c == doc_decimal).then_some(c)
      } else {
        None
      }
    }
    (false, false) => None,
  };
  let mut int_part = String::new();
  let mut frac_part = String::new();
  let (ip, fp) = match decimal {
    Some(d) => {
      let i = digits.rfind(d)?;
      (&digits[..i], &digits[i + 1..])
    }
    None => (digits, ""),
  };
  for c in ip.chars() {
    if c.is_ascii_digit() {
      int_part.push(c);
    }
  }
  for c in fp.chars() {
    if c.is_ascii_digit() {
      frac_part.push(c);
    } else {
      return None;
    }
  }
  if int_part.is_empty() || int_part.len() > 15 {
    return None;
  }
  format!(
    "{int_part}.{}",
    if frac_part.is_empty() {
      "0"
    } else {
      &frac_part
    }
  )
  .parse()
  .ok()
}

/// Money from a token such as `$1,234.56`, `1.234,56 €`, `EUR 1 234,56`, `(123.45)`, `123.45-`.
pub(crate) fn parse_amount(text: &str, sep: Separator) -> Option<Amount> {
  let mut code = None;
  let mut symbol = None;
  let mut t = text.trim();
  let mut negative = false;
  if t.starts_with('(') && t.ends_with(')') {
    negative = true;
    t = &t[1..t.len() - 1];
  }
  let t = strip_currency(t, &mut code, &mut symbol);
  let mut t = t.trim();
  if let Some(rest) = t.strip_prefix(['-', '−']) {
    negative = true;
    t = rest.trim();
  } else if let Some(rest) = t.strip_suffix(['-', '−']) {
    negative = true;
    t = rest.trim();
  }
  let t = strip_currency(t, &mut code, &mut symbol);
  let mut t = t.trim();
  if let Some(rest) = t.strip_prefix(['-', '−']) {
    negative = true;
    t = rest.trim();
  }
  let value = parse_number(t, sep)?;
  Some(Amount {
    value,
    negative,
    code,
    symbol,
  })
}

/// Which separator the tokens that show two decimals vote for, else `fallback`.
pub(crate) fn infer_separator(tokens: &[&str], fallback: Separator) -> Separator {
  let mut point = 0usize;
  let mut comma = 0usize;
  for token in tokens {
    let mut code = None;
    let mut symbol = None;
    let digits = strip_currency(
      token.trim().trim_matches(['(', ')', '-']),
      &mut code,
      &mut symbol,
    );
    if digits.is_empty()
      || !digits
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | ' '))
    {
      continue;
    }
    if let Some(i) = digits.rfind(['.', ','])
      && digits[i + 1..].len() == 2
      && digits[i + 1..].chars().all(|c| c.is_ascii_digit())
    {
      match digits[i..].chars().next() {
        Some('.') => point += 1,
        Some(',') => comma += 1,
        _ => {}
      }
    }
  }
  match point.cmp(&comma) {
    std::cmp::Ordering::Greater => Separator::Point,
    std::cmp::Ordering::Less => Separator::Comma,
    std::cmp::Ordering::Equal => fallback,
  }
}

#[cfg(test)]
mod tests {
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
}
