//! `/ToUnicode` CMaps: the producer's own statement of what each code means, which viewers
//! consult before any `/Encoding`. lopdf only reads it when a font has no `/Encoding`, and one
//! glyph name it does not know makes it discard a `/Differences` encoding wholesale, so the
//! layout engine parses the CMap itself and decodes through it first.

use std::collections::HashMap;

/// A parsed `/ToUnicode` CMap: a code, with its byte length, to the text it stands for.
#[derive(Debug, Default)]
pub(crate) struct ToUnicode {
  map: HashMap<(u8, u32), String>,
}

/// The largest `bfrange` honoured; a wild range would otherwise fill the map for every code.
const MAX_RANGE: u32 = 65_536;

#[derive(Debug, Clone, PartialEq)]
enum Token {
  Hex(Vec<u8>),
  ArrayOpen,
  ArrayClose,
  Word(String),
}

fn hex_digit(b: u8) -> Option<u8> {
  (b as char).to_digit(16).map(|d| d as u8)
}

/// `<...>` contents to bytes; an odd final digit is padded with 0 as the spec says.
fn decode_hex(hex: &[u8]) -> Vec<u8> {
  let mut out = Vec::with_capacity(hex.len() / 2 + 1);
  let mut k = 0;
  while k < hex.len() {
    let Some(hi) = hex_digit(hex[k]) else { break };
    let lo = match hex.get(k + 1) {
      Some(&b) => match hex_digit(b) {
        Some(d) => d,
        None => break,
      },
      None => 0,
    };
    out.push(hi << 4 | lo);
    k += 2;
  }
  out
}

fn tokens(src: &[u8]) -> Vec<Token> {
  let mut out = Vec::new();
  let mut i = 0;
  while i < src.len() {
    let c = src[i];
    match c {
      b'%' => {
        while i < src.len() && src[i] != b'\n' && src[i] != b'\r' {
          i += 1;
        }
      }
      b'<' => {
        if src.get(i + 1) == Some(&b'<') {
          i += 2;
          continue;
        }
        let start = i + 1;
        let mut j = start;
        while j < src.len() && src[j] != b'>' {
          j += 1;
        }
        let hex: Vec<u8> = src[start..j]
          .iter()
          .copied()
          .filter(|b| !b.is_ascii_whitespace())
          .collect();
        out.push(Token::Hex(decode_hex(&hex)));
        i = j + 1;
      }
      b'(' => {
        let mut depth = 0usize;
        while i < src.len() {
          match src[i] {
            b'\\' => i += 1,
            b'(' => depth += 1,
            b')' => {
              depth = depth.saturating_sub(1);
              if depth == 0 {
                break;
              }
            }
            _ => {}
          }
          i += 1;
        }
        i += 1;
      }
      b'[' => {
        out.push(Token::ArrayOpen);
        i += 1;
      }
      b']' => {
        out.push(Token::ArrayClose);
        i += 1;
      }
      b'>' | b'{' | b'}' => i += 1,
      c if c.is_ascii_whitespace() => i += 1,
      _ => {
        let start = i;
        while i < src.len() && !src[i].is_ascii_whitespace() && !b"<>[](){}%".contains(&src[i]) {
          i += 1;
        }
        out.push(Token::Word(
          String::from_utf8_lossy(&src[start..i]).into_owned(),
        ));
      }
    }
  }
  out
}

/// A source code's byte length and value; CMaps use one to four bytes.
fn code_of(bytes: &[u8]) -> Option<(u8, u32)> {
  if bytes.is_empty() || bytes.len() > 4 {
    return None;
  }
  let code = bytes.iter().fold(0u32, |acc, b| acc << 8 | u32::from(*b));
  Some((bytes.len() as u8, code))
}

/// Destination bytes are UTF-16BE: one or more code units, surrogate pairs allowed.
fn units_of(bytes: &[u8]) -> Vec<u16> {
  bytes
    .as_chunks::<2>()
    .0
    .iter()
    .map(|pair| u16::from_be_bytes(*pair))
    .collect()
}

fn text_of(units: &[u16]) -> Option<String> {
  if units.is_empty() {
    return None;
  }
  Some(
    char::decode_utf16(units.iter().copied())
      .map(|r| r.unwrap_or('\u{FFFD}'))
      .collect(),
  )
}

fn max_code(len: u8) -> u32 {
  if len >= 4 {
    u32::MAX
  } else {
    (1u32 << (8 * u32::from(len))) - 1
  }
}

impl ToUnicode {
  /// Parse a CMap stream's bytes. Anything it cannot read is skipped; nothing panics.
  pub(crate) fn parse(src: &[u8]) -> ToUnicode {
    let toks = tokens(src);
    let mut map = HashMap::new();
    let mut i = 0;
    while i < toks.len() {
      match &toks[i] {
        Token::Word(w) if w == "beginbfchar" => {
          i += 1;
          while let (Some(Token::Hex(code)), Some(Token::Hex(dst))) = (toks.get(i), toks.get(i + 1))
          {
            if let Some(key) = code_of(code)
              && let Some(text) = text_of(&units_of(dst))
            {
              map.insert(key, text);
            }
            i += 2;
          }
        }
        Token::Word(w) if w == "beginbfrange" => {
          i += 1;
          while let (Some(Token::Hex(lo)), Some(Token::Hex(hi))) = (toks.get(i), toks.get(i + 1)) {
            let Some((len, lo)) = code_of(lo) else { break };
            let hi = code_of(hi).map(|(_, h)| h).unwrap_or(lo).min(max_code(len));
            let hi = if hi < lo {
              lo
            } else {
              hi.min(lo.saturating_add(MAX_RANGE))
            };
            match toks.get(i + 2) {
              Some(Token::Hex(dst)) => {
                let units = units_of(dst);
                if let Some(last) = units.last().copied() {
                  for (k, code) in (lo..=hi).enumerate() {
                    let Some(unit) = last.checked_add(k as u16) else {
                      break;
                    };
                    let mut stepped = units.clone();
                    *stepped.last_mut().expect("non-empty") = unit;
                    if let Some(text) = text_of(&stepped) {
                      map.insert((len, code), text);
                    }
                  }
                }
                i += 3;
              }
              Some(Token::ArrayOpen) => {
                i += 3;
                let mut code = lo;
                while let Some(Token::Hex(dst)) = toks.get(i) {
                  if code <= hi
                    && let Some(text) = text_of(&units_of(dst))
                  {
                    map.insert((len, code), text);
                  }
                  code = code.saturating_add(1);
                  i += 1;
                }
                if toks.get(i) == Some(&Token::ArrayClose) {
                  i += 1;
                }
              }
              _ => break,
            }
          }
        }
        _ => i += 1,
      }
    }
    ToUnicode { map }
  }

  /// The text for one code's bytes, when the map states it.
  pub(crate) fn lookup(&self, bytes: &[u8]) -> Option<String> {
    let key = code_of(bytes)?;
    self.map.get(&key).cloned()
  }
}

#[cfg(test)]
mod tests;
