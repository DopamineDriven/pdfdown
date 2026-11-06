//! Identifiers as printed: invoice numbers, purchase orders, work orders, aircraft registrations
//! and serial numbers, with the label remnants a value drags along removed and nothing else
//! changed, so the value compares with what the issuer's own records hold.

const LABEL_REMNANTS: &[&str] = &["no.", "no", "nr.", "nr", "n°", "#", ":", "number"];

/// An alphanumeric identifier of 3–30 characters holding a digit; leading `#`, `No.` or `:` and
/// trailing punctuation removed, inner whitespace collapsed, case and inner punctuation kept.
pub(crate) fn parse_identifier(text: &str) -> Option<String> {
  let mut t = text.trim().to_string();
  loop {
    let lower = t.to_lowercase();
    let mut stripped = false;
    for r in LABEL_REMNANTS {
      if lower.starts_with(r) && (r.len() < lower.len()) {
        t = t[r.len()..].trim_start().to_string();
        stripped = true;
        break;
      }
    }
    if !stripped {
      break;
    }
  }
  let t = t.trim_end_matches([',', '.', ';', ':', ')', ']']).trim();
  let collapsed: String = t.split_whitespace().collect::<Vec<_>>().join(" ");
  let n = collapsed.chars().count();
  if !(3..=30).contains(&n) || !collapsed.chars().any(|c| c.is_ascii_digit()) {
    return None;
  }
  if !collapsed
    .chars()
    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '/' | '.' | '_' | '#'))
  {
    return None;
  }
  // A date is never an identifier, whatever the label beside it says.
  if super::dates::parse_date(&collapsed, super::dates::Order::Mdy).is_some() {
    return None;
  }
  Some(collapsed)
}

/// An aircraft registration: a US N-number, or a country prefix, a hyphen and three to five
/// characters (`C-GXYZ`, `G-ABCD`, `HB-XYZ`), upper-cased.
pub(crate) fn parse_registration(text: &str) -> Option<String> {
  let t = text
    .trim()
    .trim_end_matches([',', '.', ';', ':'])
    .to_ascii_uppercase();
  let chars: Vec<char> = t.chars().collect();
  if chars.is_empty() || chars.len() > 8 {
    return None;
  }
  if chars[0] == 'N' && chars.len() >= 2 && chars.len() <= 6 {
    let digits = chars[1..].iter().take_while(|c| c.is_ascii_digit()).count();
    let letters = chars[1 + digits..]
      .iter()
      .filter(|c| c.is_ascii_alphabetic())
      .count();
    if (1..=5).contains(&digits)
      && letters == chars.len() - 1 - digits
      && letters <= 2
      && chars[1] != '0'
    {
      return Some(t);
    }
  }
  if let Some(dash) = t.find('-') {
    let (prefix, suffix) = (&t[..dash], &t[dash + 1..]);
    if (1..=2).contains(&prefix.len())
      && prefix.chars().all(|c| c.is_ascii_alphabetic())
      && (3..=5).contains(&suffix.len())
      && suffix.chars().all(|c| c.is_ascii_alphanumeric())
    {
      return Some(t);
    }
  }
  None
}

/// A serial number: two to twenty alphanumerics or hyphens with at least one digit.
pub(crate) fn parse_serial(text: &str) -> Option<String> {
  let t = text
    .trim()
    .trim_end_matches([',', '.', ';', ':'])
    .to_string();
  let n = t.chars().count();
  if !(2..=20).contains(&n)
    || !t.chars().any(|c| c.is_ascii_digit())
    || !t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
  {
    return None;
  }
  Some(t)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn identifiers_keep_their_printed_form_without_label_remnants() {
    assert_eq!(parse_identifier("#13594027").as_deref(), Some("13594027"));
    assert_eq!(parse_identifier("INV-3337").as_deref(), Some("INV-3337"));
    assert_eq!(parse_identifier("A-10045,").as_deref(), Some("A-10045"));
    assert_eq!(
      parse_identifier("E240317201-CN").as_deref(),
      Some("E240317201-CN")
    );
    assert_eq!(
      parse_identifier("No. 2015-0831").as_deref(),
      Some("2015-0831")
    );
    assert_eq!(parse_identifier(": F-2209").as_deref(), Some("F-2209"));
    assert_eq!(parse_identifier("Invoice"), None, "no digit");
    assert_eq!(parse_identifier("12"), None, "too short");
    assert_eq!(
      parse_identifier("January 25, 2016"),
      None,
      "a date is not an identifier"
    );
  }

  #[test]
  fn a_date_shape_is_not_an_identifier() {
    assert_eq!(parse_identifier("03/14/2024"), None);
    assert_eq!(parse_identifier("31.08.2015"), None);
    assert_eq!(parse_identifier("2022-09-15"), None);
    assert_eq!(
      parse_identifier("2015-0831").as_deref(),
      Some("2015-0831"),
      "two parts are an id"
    );
  }

  #[test]
  fn registrations_and_serials() {
    assert_eq!(parse_registration("N417SK").as_deref(), Some("N417SK"));
    assert_eq!(parse_registration("c-gxyz").as_deref(), Some("C-GXYZ"));
    assert_eq!(parse_registration("G-ABCD").as_deref(), Some("G-ABCD"));
    assert_eq!(parse_registration("12345"), None);
    assert_eq!(parse_registration("Nordique"), None);
    assert_eq!(parse_serial("20311").as_deref(), Some("20311"));
    assert_eq!(parse_serial("MSN-5521").as_deref(), Some("MSN-5521"));
    assert_eq!(parse_serial("S/N"), None);
    assert_eq!(parse_serial("A"), None);
  }
}
