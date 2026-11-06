//! Dates as the corpus prints them, to ISO `YYYY-MM-DD`, and the day/month order inferred from
//! the dates that cannot be read two ways.

// Consumed by the anchors, priors and parties modules of the next task; until then only the
// tests call in here.
#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Order {
  Mdy,
  Dmy,
}

const MONTHS: [&str; 12] = [
  "january",
  "february",
  "march",
  "april",
  "may",
  "june",
  "july",
  "august",
  "september",
  "october",
  "november",
  "december",
];

fn month_number(name: &str) -> Option<u32> {
  let lower = name.to_ascii_lowercase();
  let lower = lower.trim_end_matches('.');
  if lower.len() < 3 {
    return None;
  }
  MONTHS
    .iter()
    .position(|m| m.starts_with(lower) || (lower == "sept" && *m == "september"))
    .map(|i| i as u32 + 1)
}

fn year_of(text: &str) -> Option<i32> {
  let y: i32 = text.parse().ok()?;
  Some(match text.len() {
    2 => 2000 + y,
    4 => y,
    _ => return None,
  })
}

fn valid(y: i32, m: u32, d: u32) -> bool {
  (1900..=2099).contains(&y) && (1..=12).contains(&m) && (1..=31).contains(&d)
}

fn iso(y: i32, m: u32, d: u32) -> String {
  format!("{y:04}-{m:02}-{d:02}")
}

/// Three numeric parts in `order`: `(iso, unambiguous)`; a day over 12 decides the order itself.
fn numeric(a: &str, b: &str, c: &str, order: Order) -> Option<(String, bool)> {
  if a.len() == 4 {
    let (y, m, d) = (year_of(a)?, b.parse().ok()?, c.parse().ok()?);
    return valid(y, m, d).then(|| (iso(y, m, d), true));
  }
  let first: u32 = a.parse().ok()?;
  let second: u32 = b.parse().ok()?;
  let y = year_of(c)?;
  let (m, d, unambiguous) = if first > 12 && second <= 12 {
    (second, first, true)
  } else if second > 12 && first <= 12 {
    (first, second, true)
  } else if first > 12 && second > 12 {
    return None;
  } else {
    match order {
      Order::Dmy => (second, first, false),
      Order::Mdy => (first, second, false),
    }
  };
  valid(y, m, d).then(|| (iso(y, m, d), unambiguous))
}

/// `(ISO date, unambiguous)` for the shapes the corpus prints, or `None`.
pub(crate) fn parse_date(text: &str, order: Order) -> Option<(String, bool)> {
  let t = text.trim().trim_end_matches(['.', ',', ';']);
  if t.is_empty() {
    return None;
  }
  // Numeric with one separator kind: 06/23/2018, 31.08.2015, 2022-09-15, 2026/11/01, 01-11-26.
  for sep in ['/', '.', '-'] {
    let parts: Vec<&str> = t.split(sep).collect();
    if parts.len() == 3
      && parts
        .iter()
        .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
    {
      return numeric(parts[0], parts[1], parts[2], order);
    }
  }
  // 01-Nov-26, 1-Nov-2026
  let dashed: Vec<&str> = t.split('-').collect();
  if dashed.len() == 3
    && dashed[0].chars().all(|c| c.is_ascii_digit())
    && let (Ok(d), Some(m), Some(y)) = (
      dashed[0].parse::<u32>(),
      month_number(dashed[1]),
      year_of(dashed[2]),
    )
  {
    return valid(y, m, d).then(|| (iso(y, m, d), true));
  }
  // Compact 27MAR2026 / 27MAR26
  if t.len() >= 7 && t.len() <= 9 {
    let digits_end = t.find(|c: char| !c.is_ascii_digit()).unwrap_or(0);
    let letters_end = t[digits_end..]
      .find(|c: char| !c.is_ascii_alphabetic())
      .map(|i| i + digits_end)
      .unwrap_or(t.len());
    if digits_end > 0
      && letters_end > digits_end
      && letters_end < t.len()
      && let (Ok(d), Some(m), Some(y)) = (
        t[..digits_end].parse::<u32>(),
        month_number(&t[digits_end..letters_end]),
        year_of(&t[letters_end..]),
      )
    {
      return valid(y, m, d).then(|| (iso(y, m, d), true));
    }
  }
  // Worded: January 25, 2016 · Nov 1 2026 · 1 Nov 2026 · November 1st, 2026
  let words: Vec<String> = t
    .split(|c: char| c.is_whitespace() || c == ',')
    .filter(|w| !w.is_empty())
    .map(|w| {
      w.trim_end_matches(|c: char| !c.is_ascii_alphanumeric())
        .to_string()
    })
    .collect();
  if words.len() == 3 {
    let ordinal = |w: &str| -> Option<u32> {
      let digits: String = w.chars().take_while(|c| c.is_ascii_digit()).collect();
      let suffix = &w[digits.len()..];
      if digits.is_empty() || !matches!(suffix, "" | "st" | "nd" | "rd" | "th") {
        return None;
      }
      digits.parse().ok()
    };
    if let (Some(m), Some(d), Some(y)) = (
      month_number(&words[0]),
      ordinal(&words[1]),
      year_of(&words[2]),
    ) {
      return valid(y, m, d).then(|| (iso(y, m, d), true));
    }
    if let (Some(d), Some(m), Some(y)) = (
      ordinal(&words[0]),
      month_number(&words[1]),
      year_of(&words[2]),
    ) {
      return valid(y, m, d).then(|| (iso(y, m, d), true));
    }
  }
  None
}

/// The order the unambiguous numeric dates vote for, else `fallback`.
pub(crate) fn infer_order(texts: &[&str], fallback: Order) -> Order {
  let mut dmy = 0usize;
  let mut mdy = 0usize;
  for t in texts {
    let t = t.trim();
    for sep in ['/', '.', '-'] {
      let parts: Vec<&str> = t.split(sep).collect();
      if parts.len() == 3
        && parts
          .iter()
          .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
        && parts[0].len() != 4
      {
        let (Ok(a), Ok(b)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) else {
          break;
        };
        if a > 12 && b <= 12 {
          dmy += 1;
        } else if b > 12 && a <= 12 {
          mdy += 1;
        }
        break;
      }
    }
  }
  match dmy.cmp(&mdy) {
    std::cmp::Ordering::Greater => Order::Dmy,
    std::cmp::Ordering::Less => Order::Mdy,
    std::cmp::Ordering::Equal => fallback,
  }
}

#[cfg(test)]
mod tests {
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
}
