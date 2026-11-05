//! Number formats: a cell's format code parsed into sections and tokens, and the display text it
//! produces for a value, in the en-US spellings the design settles on.
//!
//! Supported: General, fixed decimals with and without grouping, zero padding, percent, quoted and
//! escaped literals, padding (`_x`, one space) and fill (`*x`, dropped) tokens, colour tags
//! (dropped), currency and accounting codes with negative sections, the date and time tokens
//! `y m d h s AM/PM` (where `m` is minutes next to `h` or `s`), the text section `@`. Unsupported,
//! returning `None` from `parse`: elapsed durations, scientific notation, fractions (a bare slash
//! among digit placeholders), conditions, locale directives, era and Buddhist-year tokens.

#[derive(Debug, Clone, PartialEq)]
enum DateToken {
  Year2,
  Year4,
  Month,
  Month2,
  MonthAbbr,
  MonthName,
  Day,
  Day2,
  DayAbbr,
  DayName,
  Hour,
  Hour2,
  Minute,
  Minute2,
  Second,
  Second2,
  AmPm,
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
  Literal(String),
  /// `0`, `#` or `?`.
  Digit(char),
  Decimal,
  Comma,
  Percent,
  General,
  Text,
  Date(DateToken),
  /// A bare `/`: a separator between date tokens, a fraction bar between digit placeholders.
  Slash,
  /// A run of `m` whose meaning (month or minute) is settled once the whole section is read.
  Ambiguous(usize),
}

#[derive(Debug, Clone, PartialEq)]
struct Section {
  tokens: Vec<Token>,
  decimals: usize,
  /// `#` placeholders after the point: shown only when non-zero.
  optional_decimals: usize,
  int_min: usize,
  grouping: bool,
  scale: u32,
  percent: bool,
  has_number: bool,
  is_date: bool,
  has_time: bool,
  is_text: bool,
}

/// A parsed format code: one to four sections for positive, negative, zero and text values.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NumberFormat {
  sections: Vec<Section>,
}

/// Excel refuses format codes longer than this; so do we, before any work on them.
const MAX_CODE_CHARS: usize = 255;
/// Excel serial of 1970-01-01 in the 1900 system.
const SERIAL_AT_UNIX_EPOCH_1900: i64 = 25_569;
/// Days from 1904-01-01 to 1970-01-01.
const DAYS_1904_TO_UNIX_EPOCH: i64 = 24_107;
const MONTHS: [&str; 12] = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
];
const DAYS: [&str; 7] = [
  "Sunday",
  "Monday",
  "Tuesday",
  "Wednesday",
  "Thursday",
  "Friday",
  "Saturday",
];
const COLOURS: [&str; 8] = [
  "black", "blue", "cyan", "green", "magenta", "red", "white", "yellow",
];

/// Split a code on `;` outside quotes and brackets.
fn split_sections(code: &str) -> Vec<String> {
  let mut sections = vec![String::new()];
  let mut quoted = false;
  let mut bracket = false;
  let mut escape = false;
  for c in code.chars() {
    if escape {
      sections.last_mut().expect("section").push(c);
      escape = false;
      continue;
    }
    match c {
      '\\' if !quoted => {
        escape = true;
        sections.last_mut().expect("section").push(c);
      }
      '"' if !bracket => {
        quoted = !quoted;
        sections.last_mut().expect("section").push(c);
      }
      '[' if !quoted => {
        bracket = true;
        sections.last_mut().expect("section").push(c);
      }
      ']' if !quoted => {
        bracket = false;
        sections.last_mut().expect("section").push(c);
      }
      ';' if !quoted && !bracket => sections.push(String::new()),
      _ => sections.last_mut().expect("section").push(c),
    }
  }
  sections
}

fn parse_section(text: &str) -> Option<Section> {
  let chars: Vec<char> = text.chars().collect();
  let mut tokens = Vec::new();
  let mut i = 0;
  while i < chars.len() {
    let c = chars[i];
    match c {
      '"' => {
        let end = chars[i + 1..].iter().position(|&x| x == '"')? + i + 1;
        tokens.push(Token::Literal(chars[i + 1..end].iter().collect()));
        i = end + 1;
      }
      '\\' => {
        tokens.push(Token::Literal(chars.get(i + 1)?.to_string()));
        i += 2;
      }
      '_' => {
        chars.get(i + 1)?;
        tokens.push(Token::Literal(" ".to_string()));
        i += 2;
      }
      '*' => {
        chars.get(i + 1)?;
        i += 2;
      }
      '[' => {
        let end = chars[i + 1..].iter().position(|&x| x == ']')? + i + 1;
        let inner: String = chars[i + 1..end].iter().collect();
        let lower = inner.to_ascii_lowercase();
        if let Some(rest) = inner.strip_prefix('$') {
          // `[$€]` is a currency literal; `[$€-407]` and `[$-409]` carry a locale: unsupported.
          if rest.contains('-') {
            return None;
          }
          if !rest.is_empty() {
            tokens.push(Token::Literal(rest.to_string()));
          }
        } else if COLOURS.contains(&lower.as_str()) || lower.starts_with("color") {
          // colour tags carry no text
        } else {
          // elapsed time, conditions and anything else bracketed
          return None;
        }
        i = end + 1;
      }
      '0' | '#' | '?' => {
        tokens.push(Token::Digit(c));
        i += 1;
      }
      '.' => {
        tokens.push(Token::Decimal);
        i += 1;
      }
      ',' => {
        tokens.push(Token::Comma);
        i += 1;
      }
      '%' => {
        tokens.push(Token::Percent);
        i += 1;
      }
      '@' => {
        tokens.push(Token::Text);
        i += 1;
      }
      '/' => {
        tokens.push(Token::Slash);
        i += 1;
      }
      'E' | 'e' if matches!(chars.get(i + 1), Some('+' | '-')) => return None,
      'G' | 'g'
        if chars[i..]
          .iter()
          .take(7)
          .collect::<String>()
          .eq_ignore_ascii_case("general") =>
      {
        tokens.push(Token::General);
        i += 7;
      }
      'A' | 'a' => {
        let rest: Vec<char> = chars[i..]
          .iter()
          .take(5)
          .map(|c| c.to_ascii_lowercase())
          .collect();
        if rest == ['a', 'm', '/', 'p', 'm'] {
          tokens.push(Token::Date(DateToken::AmPm));
          i += 5;
        } else if rest.len() >= 3 && rest[..3] == ['a', '/', 'p'] {
          tokens.push(Token::Date(DateToken::AmPm));
          i += 3;
        } else {
          tokens.push(Token::Literal(c.to_string()));
          i += 1;
        }
      }
      'y' | 'Y' | 'm' | 'M' | 'd' | 'D' | 'h' | 'H' | 's' | 'S' => {
        let lower = c.to_ascii_lowercase();
        let run = chars[i..]
          .iter()
          .take_while(|x| x.to_ascii_lowercase() == lower)
          .count();
        tokens.push(match (lower, run) {
          ('y', 1..=2) => Token::Date(DateToken::Year2),
          ('y', _) => Token::Date(DateToken::Year4),
          ('m', n) => Token::Ambiguous(n),
          ('d', 1) => Token::Date(DateToken::Day),
          ('d', 2) => Token::Date(DateToken::Day2),
          ('d', 3) => Token::Date(DateToken::DayAbbr),
          ('d', _) => Token::Date(DateToken::DayName),
          ('h', 1) => Token::Date(DateToken::Hour),
          ('h', _) => Token::Date(DateToken::Hour2),
          ('s', 1) => Token::Date(DateToken::Second),
          ('s', _) => Token::Date(DateToken::Second2),
          _ => return None,
        });
        i += run;
      }
      'b' | 'B' | 'g' | 'G' | 'e' => return None,
      _ => {
        tokens.push(Token::Literal(c.to_string()));
        i += 1;
      }
    }
  }
  // An `m` run is minutes next to an hour or a second token, else a month.
  let dates: Vec<usize> = tokens
    .iter()
    .enumerate()
    .filter(|(_, t)| matches!(t, Token::Date(_) | Token::Ambiguous(_)))
    .map(|(i, _)| i)
    .collect();
  for (k, &idx) in dates.iter().enumerate() {
    if let Token::Ambiguous(n) = tokens[idx] {
      let prev_hour = k > 0
        && matches!(
          tokens[dates[k - 1]],
          Token::Date(DateToken::Hour | DateToken::Hour2)
        );
      let next_second = dates.get(k + 1).is_some_and(|&j| {
        matches!(
          tokens[j],
          Token::Date(DateToken::Second | DateToken::Second2)
        )
      });
      tokens[idx] = Token::Date(if prev_hour || next_second {
        if n >= 2 {
          DateToken::Minute2
        } else {
          DateToken::Minute
        }
      } else {
        match n {
          1 => DateToken::Month,
          2 => DateToken::Month2,
          3 => DateToken::MonthAbbr,
          _ => DateToken::MonthName,
        }
      });
    }
  }

  let has_number = tokens.iter().any(|t| matches!(t, Token::Digit(_)));
  let is_date = tokens.iter().any(|t| matches!(t, Token::Date(_)));
  let has_time = tokens.iter().any(|t| {
    matches!(
      t,
      Token::Date(
        DateToken::Hour
          | DateToken::Hour2
          | DateToken::Minute
          | DateToken::Minute2
          | DateToken::Second
          | DateToken::Second2
          | DateToken::AmPm
      )
    )
  });
  if is_date && has_number {
    return None;
  }
  if is_date {
    for t in tokens.iter_mut() {
      match t {
        Token::Decimal => *t = Token::Literal(".".to_string()),
        Token::Comma => *t = Token::Literal(",".to_string()),
        _ => {}
      }
    }
  }
  // A bare slash among digit placeholders is a fraction, which this release does not render.
  if has_number && tokens.iter().any(|t| matches!(t, Token::Slash)) {
    return None;
  }
  let is_text = tokens.iter().any(|t| matches!(t, Token::Text)) && !has_number && !is_date;

  // Number shape: digits before and after the point, grouping commas, scaling commas. Every
  // count comes from the tokens, so digits inside tags, quotes and padding markers do not count.
  let decimal_at = tokens.iter().position(|t| matches!(t, Token::Decimal));
  if tokens
    .iter()
    .filter(|t| matches!(t, Token::Decimal))
    .count()
    > 1
  {
    return None;
  }
  let int_tokens = &tokens[..decimal_at.unwrap_or(tokens.len())];
  let frac_tokens = decimal_at.map(|at| &tokens[at + 1..]).unwrap_or(&[]);
  let last_int_digit = int_tokens
    .iter()
    .rposition(|t| matches!(t, Token::Digit(_)));
  let int_min = int_tokens
    .iter()
    .filter(|t| matches!(t, Token::Digit('0')))
    .count();
  let grouping = last_int_digit.is_some_and(|last| {
    int_tokens[..last].iter().enumerate().any(|(i, t)| {
      matches!(t, Token::Comma) && int_tokens[..i].iter().any(|x| matches!(x, Token::Digit(_)))
    })
  });
  let scale = match tokens.iter().rposition(|t| matches!(t, Token::Digit(_))) {
    Some(last) => tokens[last + 1..]
      .iter()
      .take_while(|t| matches!(t, Token::Comma))
      .count() as u32,
    None => 0,
  };
  let decimals = frac_tokens
    .iter()
    .filter(|t| matches!(t, Token::Digit(_)))
    .count();
  let optional_decimals = frac_tokens
    .iter()
    .filter(|t| matches!(t, Token::Digit('#' | '?')))
    .count();
  let percent = tokens.iter().any(|t| matches!(t, Token::Percent));
  Some(Section {
    tokens,
    decimals,
    optional_decimals,
    int_min,
    grouping,
    scale,
    percent,
    has_number,
    is_date,
    has_time,
    is_text,
  })
}

impl NumberFormat {
  /// Parse a format code; `None` when any section uses something this release does not render.
  pub(crate) fn parse(code: &str) -> Option<NumberFormat> {
    if code.chars().count() > MAX_CODE_CHARS {
      return None;
    }
    let sections = split_sections(code);
    if sections.is_empty() || sections.len() > 4 {
      return None;
    }
    let sections = sections
      .iter()
      .map(|s| parse_section(s))
      .collect::<Option<Vec<_>>>()?;
    Some(NumberFormat { sections })
  }

  /// Whether a serial under this code reads as a date or time.
  pub(crate) fn is_date(&self) -> bool {
    self.sections[0].is_date
  }

  /// Whether the date rendering carries a clock time.
  pub(crate) fn has_time(&self) -> bool {
    self.sections[0].has_time
  }

  /// The display text for `value`; `None` when a date code meets a serial that is not a date.
  pub(crate) fn render(&self, value: f64, date1904: bool) -> Option<String> {
    let (section, negative) = match self.sections.len() {
      n if n >= 2 && value < 0.0 => (&self.sections[1], false),
      n if n >= 3 && value == 0.0 => (&self.sections[2], false),
      _ => (&self.sections[0], value < 0.0),
    };
    let magnitude = value.abs();
    let rendered = if section.is_text {
      render_general(value)
    } else if section.is_date {
      render_date(section, value, date1904)?
    } else {
      render_number(section, magnitude)
    };
    let rendered = rendered.trim().to_string();
    Some(if negative && !section.is_date && !section.is_text {
      format!("-{rendered}")
    } else {
      rendered
    })
  }
}

fn render_number(section: &Section, magnitude: f64) -> String {
  let mut v = magnitude;
  if section.percent {
    v *= 100.0;
  }
  for _ in 0..section.scale {
    v /= 1000.0;
  }
  let number = if section.has_number {
    let factor = 10f64.powi(section.decimals as i32);
    let rounded = (v * factor).round() / factor;
    let text = format!("{:.*}", section.decimals, rounded);
    let (int_part, frac_part) = text.split_once('.').unwrap_or((text.as_str(), ""));
    let mut int_digits = int_part.to_string();
    if int_digits == "0" && section.int_min == 0 {
      int_digits.clear();
    }
    if int_digits.len() < section.int_min {
      int_digits = format!(
        "{}{int_digits}",
        "0".repeat(section.int_min - int_digits.len())
      );
    }
    let grouped = if section.grouping {
      group_thousands(&int_digits)
    } else {
      int_digits
    };
    let mut frac = frac_part.to_string();
    if section.optional_decimals > 0 {
      let required = section.decimals - section.optional_decimals;
      while frac.len() > required && frac.ends_with('0') {
        frac.pop();
      }
    }
    if frac.is_empty() {
      grouped
    } else {
      format!("{grouped}.{frac}")
    }
  } else {
    String::new()
  };
  let mut out = String::new();
  let mut number_emitted = false;
  for token in &section.tokens {
    match token {
      Token::Literal(text) => out.push_str(text),
      Token::Slash => out.push('/'),
      Token::General => out.push_str(&render_general(magnitude)),
      Token::Percent => out.push('%'),
      Token::Digit(_) | Token::Decimal | Token::Comma => {
        if !number_emitted {
          out.push_str(&number);
          number_emitted = true;
        }
      }
      Token::Text | Token::Date(_) | Token::Ambiguous(_) => {}
    }
  }
  out
}

fn group_thousands(digits: &str) -> String {
  let mut out = String::new();
  for (i, c) in digits.chars().enumerate() {
    if i > 0 && (digits.len() - i).is_multiple_of(3) {
      out.push(',');
    }
    out.push(c);
  }
  out
}

fn render_date(section: &Section, serial: f64, date1904: bool) -> Option<String> {
  let (year, month, day, hour, minute, second) = serial_to_datetime(serial, date1904)?;
  let twelve_hour = section
    .tokens
    .iter()
    .any(|t| matches!(t, Token::Date(DateToken::AmPm)));
  let weekday = weekday_of(serial, date1904)?;
  let mut out = String::new();
  for token in &section.tokens {
    match token {
      Token::Literal(text) => out.push_str(text),
      Token::Slash => out.push('/'),
      Token::Date(t) => out.push_str(&match t {
        DateToken::Year2 => format!("{:02}", year.rem_euclid(100)),
        DateToken::Year4 => format!("{year:04}"),
        DateToken::Month => month.to_string(),
        DateToken::Month2 => format!("{month:02}"),
        DateToken::MonthAbbr => MONTHS[month as usize - 1][..3].to_string(),
        DateToken::MonthName => MONTHS[month as usize - 1].to_string(),
        DateToken::Day => day.to_string(),
        DateToken::Day2 => format!("{day:02}"),
        DateToken::DayAbbr => DAYS[weekday][..3].to_string(),
        DateToken::DayName => DAYS[weekday].to_string(),
        DateToken::Hour | DateToken::Hour2 => {
          let h = if twelve_hour {
            match hour % 12 {
              0 => 12,
              h => h,
            }
          } else {
            hour
          };
          if matches!(t, DateToken::Hour2) {
            format!("{h:02}")
          } else {
            h.to_string()
          }
        }
        DateToken::Minute => minute.to_string(),
        DateToken::Minute2 => format!("{minute:02}"),
        DateToken::Second => second.to_string(),
        DateToken::Second2 => format!("{second:02}"),
        DateToken::AmPm => if hour < 12 { "AM" } else { "PM" }.to_string(),
      }),
      _ => {}
    }
  }
  Some(out)
}

/// General: integers without a point, otherwise up to 11 significant digits with trailing zeros
/// trimmed, which is how Excel shows an unformatted number in a wide enough cell.
pub(crate) fn render_general(value: f64) -> String {
  if !value.is_finite() {
    return value.to_string();
  }
  if value == 0.0 {
    return "0".to_string();
  }
  let exponent = decimal_exponent(value.abs());
  if !(-10..15).contains(&exponent) {
    return render_scientific(value, exponent);
  }
  if value.fract() == 0.0 {
    return format!("{}", value as i64);
  }
  let decimals = (10 - exponent).clamp(0, 20) as usize;
  let text = format!("{:.*}", decimals, value);
  let trimmed = text.trim_end_matches('0').trim_end_matches('.');
  if trimmed.is_empty() || trimmed == "-" {
    "0".to_string()
  } else {
    trimmed.to_string()
  }
}

/// The power of ten at or below `magnitude`, corrected for the rounding `log10` can introduce
/// just below a power of ten.
fn decimal_exponent(magnitude: f64) -> i32 {
  let mut exponent = magnitude.log10().floor() as i32;
  if 10f64.powi(exponent) > magnitude {
    exponent -= 1;
  } else if 10f64.powi(exponent + 1) <= magnitude {
    exponent += 1;
  }
  exponent
}

/// `1.23457E+20`: a mantissa of up to six significant digits and a signed two-digit exponent.
fn render_scientific(value: f64, exponent: i32) -> String {
  let mut exponent = exponent;
  let mut mantissa = value / 10f64.powi(exponent);
  let rounded = (mantissa * 1e5).round() / 1e5;
  if rounded.abs() >= 10.0 {
    exponent += 1;
    mantissa /= 10.0;
  }
  let text = format!("{:.5}", mantissa);
  let mantissa = text.trim_end_matches('0').trim_end_matches('.');
  let sign = if exponent < 0 { '-' } else { '+' };
  format!("{mantissa}E{sign}{:02}", exponent.abs())
}

/// Days since 1970-01-01 for a serial, or `None` where the serial is not a date.
fn unix_days(serial: f64, date1904: bool) -> Option<(i64, i64)> {
  if !serial.is_finite() || !(0.0..=5_000_000.0).contains(&serial) {
    return None;
  }
  let mut days = serial.floor() as i64;
  let mut seconds = ((serial - serial.floor()) * 86_400.0).round() as i64;
  if seconds >= 86_400 {
    days += 1;
    seconds = 0;
  }
  let unix = if date1904 {
    days - DAYS_1904_TO_UNIX_EPOCH
  } else if days == 60 {
    // The 1900 system's day 60 is the leap day 1900 never had.
    return None;
  } else if days < 60 {
    days - SERIAL_AT_UNIX_EPOCH_1900 + 1
  } else {
    days - SERIAL_AT_UNIX_EPOCH_1900
  };
  Some((unix, seconds))
}

/// A serial to civil date and clock time, or `None` for a negative serial, the phantom 1900
/// leap day, or a date past year 9999.
pub(crate) fn serial_to_datetime(
  serial: f64,
  date1904: bool,
) -> Option<(i32, u32, u32, u32, u32, u32)> {
  let (days, seconds) = unix_days(serial, date1904)?;
  let (year, month, day) = civil_from_days(days);
  if !(1..=9999).contains(&year) {
    return None;
  }
  Some((
    year,
    month,
    day,
    (seconds / 3600) as u32,
    ((seconds % 3600) / 60) as u32,
    (seconds % 60) as u32,
  ))
}

/// `YYYY-MM-DD`, or `YYYY-MM-DDTHH:MM:SS` with `with_time`.
pub(crate) fn iso_date(serial: f64, date1904: bool, with_time: bool) -> Option<String> {
  let (y, mo, d, h, mi, s) = serial_to_datetime(serial, date1904)?;
  Some(if with_time {
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}")
  } else {
    format!("{y:04}-{mo:02}-{d:02}")
  })
}

fn weekday_of(serial: f64, date1904: bool) -> Option<usize> {
  let (days, _) = unix_days(serial, date1904)?;
  // 1970-01-01 was a Thursday.
  Some(((days % 7 + 7 + 4) % 7) as usize)
}

/// Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i32, u32, u32) {
  let shifted = days + 719_468;
  let era = shifted.div_euclid(146_097);
  let day_of_era = shifted.rem_euclid(146_097);
  let year_of_era =
    (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
  let year = year_of_era + era * 400;
  let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
  let month_prime = (5 * day_of_year + 2) / 153;
  let day = (day_of_year - (153 * month_prime + 2) / 5 + 1) as u32;
  let month = (if month_prime < 10 {
    month_prime + 3
  } else {
    month_prime - 9
  }) as u32;
  (
    (if month <= 2 { year + 1 } else { year }) as i32,
    month,
    day,
  )
}

#[cfg(test)]
mod tests {
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
}
