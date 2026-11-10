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
  // A date is never an identifier, whatever the label beside it says; nor is anything shaped
  // like one (`08/07/7014` is a misread date, not a number).
  if super::dates::parse_date(&collapsed, super::dates::Order::Mdy).is_some()
    || is_date_shaped(&collapsed)
  {
    return None;
  }
  Some(collapsed)
}

/// Three all-digit groups split by `/`, `.` or `-` in a day-month-year or year-month-day
/// arrangement of lengths, whatever the digits say.
fn is_date_shaped(text: &str) -> bool {
  let groups: Vec<&str> = text.split(['/', '.', '-']).collect();
  if groups.len() != 3
    || !groups
      .iter()
      .all(|g| !g.is_empty() && g.chars().all(|c| c.is_ascii_digit()))
  {
    return false;
  }
  let lens: Vec<usize> = groups.iter().map(|g| g.len()).collect();
  let short = |n: usize| (1..=2).contains(&n);
  let year = |n: usize| n == 2 || n == 4;
  (short(lens[0]) && short(lens[1]) && year(lens[2]))
    || (lens[0] == 4 && short(lens[1]) && short(lens[2]))
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
mod tests;
