//! Per-font metrics needed to place glyphs: encoding, advance widths,
//! ascent/descent, and the bold/italic hints the descriptor carries.

use super::standard14;
use lopdf::{Dictionary, Document, Encoding, Object};

use super::tounicode::ToUnicode;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WidthSource {
  /// `/Widths` or `/W` from the font dictionary.
  Metric,
  /// Built-in standard 14 metrics (no `/Widths` in the file).
  Standard14,
  /// Nothing available: 0.5 em per glyph.
  Estimate,
}

pub(crate) struct FontInfo<'a> {
  pub name: String,
  /// The font's `/ToUnicode` map, consulted before `encoding`: it is the producer's own statement
  /// of what each code means, and the one a viewer's copy-and-paste follows.
  pub to_unicode: Option<ToUnicode>,
  pub encoding: Option<Encoding<'a>>,
  /// Composite (Type0) fonts consume two bytes per code.
  pub two_byte: bool,
  /// Advance per code in text-space units (em fractions); Type3 already
  /// includes `/FontMatrix`.
  pub widths: HashMap<u32, f64>,
  pub default_width: f64,
  pub ascent: f64,
  pub descent: f64,
  pub bold: bool,
  pub italic: bool,
  /// The font name says the glyphs are bars (barcode fonts): the "words" are not letters.
  pub symbolic: bool,
  /// False for anonymous names (`CIDFont+F1`, `F2`) without `ForceBold` or `FontWeight` evidence:
  /// then `bold` and `italic` are guesses and callers should treat them as unknown.
  pub weight_known: bool,
  pub width_source: WidthSource,
}

fn deref<'a>(doc: &'a Document, obj: &'a Object) -> Option<&'a Object> {
  match obj {
    Object::Reference(id) => doc.get_object(*id).ok(),
    other => Some(other),
  }
}

fn dict_of<'a>(doc: &'a Document, dict: &'a Dictionary, key: &[u8]) -> Option<&'a Dictionary> {
  match deref(doc, dict.get(key).ok()?)? {
    Object::Dictionary(d) => Some(d),
    Object::Stream(s) => Some(&s.dict),
    _ => None,
  }
}

fn num(obj: &Object) -> Option<f64> {
  match obj {
    Object::Integer(i) => Some(*i as f64),
    Object::Real(r) => Some(f64::from(*r)),
    _ => None,
  }
}

fn num_of(doc: &Document, dict: &Dictionary, key: &[u8]) -> Option<f64> {
  dict.get(key).ok().and_then(|o| deref(doc, o)).and_then(num)
}

fn array_of<'a>(doc: &'a Document, dict: &'a Dictionary, key: &[u8]) -> Option<&'a Vec<Object>> {
  match deref(doc, dict.get(key).ok()?)? {
    Object::Array(a) => Some(a),
    _ => None,
  }
}

/// `ABCDEF+Helvetica-Bold` → `Helvetica-Bold`.
fn strip_subset_prefix(name: &str) -> &str {
  match name.split_once('+') {
    Some((prefix, rest)) if prefix.len() == 6 && prefix.chars().all(|c| c.is_ascii_uppercase()) => {
      rest
    }
    _ => name,
  }
}

/// `/W [ c [w1 w2 ...] | cfirst clast w ]` for CID fonts.
fn parse_cid_widths(doc: &Document, w: &[Object], out: &mut HashMap<u32, f64>) {
  let mut i = 0;
  while i < w.len() {
    let Some(first) = deref(doc, &w[i]).and_then(num) else {
      i += 1;
      continue;
    };
    match w.get(i + 1).and_then(|o| deref(doc, o)) {
      Some(Object::Array(list)) => {
        for (k, item) in list.iter().enumerate() {
          if let Some(width) = deref(doc, item).and_then(num) {
            out.insert(first as u32 + k as u32, width / 1000.0);
          }
        }
        i += 2;
      }
      Some(last_obj) => {
        let last = num(last_obj).unwrap_or(first);
        let width = w
          .get(i + 2)
          .and_then(|o| deref(doc, o))
          .and_then(num)
          .unwrap_or(1000.0);
        let (lo, hi) = (first as u32, last as u32);
        if hi >= lo && hi - lo < 65_536 {
          for code in lo..=hi {
            out.insert(code, width / 1000.0);
          }
        }
        i += 3;
      }
      None => break,
    }
  }
}

pub(crate) fn load_font<'a>(doc: &'a Document, dict: &'a Dictionary) -> FontInfo<'a> {
  let subtype = dict
    .get(b"Subtype")
    .ok()
    .and_then(|o| o.as_name().ok())
    .map(|n| String::from_utf8_lossy(n).to_string())
    .unwrap_or_default();
  let raw_name = dict
    .get(b"BaseFont")
    .ok()
    .and_then(|o| deref(doc, o))
    .and_then(|o| o.as_name().ok())
    .map(|n| String::from_utf8_lossy(n).to_string())
    .unwrap_or_default();
  let name = strip_subset_prefix(&raw_name).to_string();

  let is_type0 = subtype == "Type0";
  let is_type3 = subtype == "Type3";
  let descendant = if is_type0 {
    array_of(doc, dict, b"DescendantFonts")
      .and_then(|a| a.first())
      .and_then(|o| deref(doc, o))
      .and_then(|o| match o {
        Object::Dictionary(d) => Some(d),
        _ => None,
      })
  } else {
    None
  };
  let metrics_dict = descendant.unwrap_or(dict);
  let descriptor = dict_of(doc, metrics_dict, b"FontDescriptor");

  let flags = descriptor
    .and_then(|d| num_of(doc, d, b"Flags"))
    .unwrap_or(0.0) as u32;
  let weight = descriptor
    .and_then(|d| num_of(doc, d, b"FontWeight"))
    .unwrap_or(0.0);
  // Name, ForceBold flag and FontWeight only: StemV is unreliable (Microsoft
  // Print To PDF writes 833 for regular Times New Roman).
  let lower = name.to_ascii_lowercase();
  let compact: String = lower
    .chars()
    .filter(|c| !c.is_whitespace() && *c != '-' && *c != '_')
    .collect();
  // "Ultra" alone is the heaviest cut of a family; "UltraLight" and "UltraThin" are the lightest.
  let ultra =
    compact.contains("ultra") && !compact.contains("ultralight") && !compact.contains("ultrathin");
  // "fett" covers the German cuts: Halbfett (600), Dreiviertelfett (700), Fett (800), Extrafett.
  let bold = compact.contains("bold")
    || compact.contains("black")
    || compact.contains("heavy")
    || compact.contains("fett")
    || ultra
    || flags & (1 << 18) != 0
    || weight >= 600.0;
  let italic = lower.contains("italic") || lower.contains("oblique") || flags & (1 << 6) != 0;
  let symbolic = is_barcode_font(&lower);
  let weight_known = !is_anonymous_font(&lower) || flags & (1 << 18) != 0 || weight > 0.0;

  let mut widths = HashMap::new();
  let mut default_width = 0.0;
  let mut width_source = WidthSource::Estimate;
  let mut ascent = descriptor
    .and_then(|d| num_of(doc, d, b"Ascent"))
    .map(|a| a / 1000.0);
  let mut descent = descriptor
    .and_then(|d| num_of(doc, d, b"Descent"))
    .map(|a| a / 1000.0);

  if is_type0 {
    if let Some(desc) = descendant {
      default_width = num_of(doc, desc, b"DW").unwrap_or(1000.0) / 1000.0;
      if let Some(w) = array_of(doc, desc, b"W") {
        parse_cid_widths(doc, w, &mut widths);
      }
      width_source = WidthSource::Metric;
    }
  } else if is_type3 {
    let matrix: Vec<f64> = array_of(doc, dict, b"FontMatrix")
      .map(|a| a.iter().filter_map(num).collect())
      .unwrap_or_default();
    let (ma, md) = if matrix.len() >= 6 {
      (matrix[0], matrix[3])
    } else {
      (0.001, 0.001)
    };
    let first = num_of(doc, dict, b"FirstChar").unwrap_or(0.0) as u32;
    if let Some(w) = array_of(doc, dict, b"Widths") {
      for (k, item) in w.iter().enumerate() {
        if let Some(width) = deref(doc, item).and_then(num) {
          widths.insert(first + k as u32, width * ma);
        }
      }
      width_source = WidthSource::Metric;
    }
    if let Some(bbox) = array_of(doc, dict, b"FontBBox")
      && bbox.len() == 4
    {
      let top = bbox[3].as_f32().map(f64::from).unwrap_or(0.0) * md;
      let bottom = bbox[1].as_f32().map(f64::from).unwrap_or(0.0) * md;
      ascent.get_or_insert(top);
      descent.get_or_insert(bottom);
    }
    default_width = 0.0;
  } else {
    let first = num_of(doc, dict, b"FirstChar").unwrap_or(0.0) as u32;
    if let Some(w) = array_of(doc, dict, b"Widths")
      && !w.is_empty()
    {
      for (k, item) in w.iter().enumerate() {
        if let Some(width) = deref(doc, item).and_then(num) {
          widths.insert(first + k as u32, width / 1000.0);
        }
      }
      width_source = WidthSource::Metric;
      default_width = descriptor
        .and_then(|d| num_of(doc, d, b"MissingWidth"))
        .unwrap_or(0.0)
        / 1000.0;
    } else {
      let std = standard14::lookup(&name, bold);
      for code in 32u32..=126 {
        if let Some(w) = std.width(code) {
          widths.insert(code, w);
        }
      }
      width_source = WidthSource::Standard14;
      default_width = 0.5;
      ascent.get_or_insert(std.ascent);
      descent.get_or_insert(std.descent);
    }
  }

  let to_unicode = dict
    .get(b"ToUnicode")
    .ok()
    .and_then(|o| deref(doc, o))
    .and_then(|o| o.as_stream().ok())
    .and_then(|s| {
      s.decompressed_content()
        .or_else(|_| s.get_plain_content())
        .ok()
    })
    .map(|bytes| ToUnicode::parse(&bytes));

  FontInfo {
    name,
    to_unicode,
    encoding: dict.get_font_encoding(doc).ok(),
    two_byte: is_type0,
    widths,
    default_width,
    ascent: ascent.unwrap_or(0.9),
    descent: descent.unwrap_or(-0.2),
    bold,
    italic,
    symbolic,
    weight_known,
    width_source,
  }
}

/// Barcode fonts set bars, not letters; their "words" must not feed text heuristics.
fn is_barcode_font(lower: &str) -> bool {
  let compact: String = lower
    .chars()
    .filter(|c| !c.is_whitespace() && *c != '-' && *c != '_')
    .collect();
  [
    "barcode",
    "idautomation",
    "code128",
    "code39",
    "3of9",
    "postnet",
    "ean13",
    "qrcode",
    "datamatrix",
  ]
  .iter()
  .any(|needle| compact.contains(needle))
    || upc_at_word_start(lower)
}

/// The UPC-A / UPC-E families: `upc` at the start of the name or of a word in it. A word that
/// merely contains it ("Cupcake") is not a barcode font.
fn upc_at_word_start(lower: &str) -> bool {
  lower.match_indices("upc").any(|(at, _)| {
    !lower[..at]
      .chars()
      .next_back()
      .is_some_and(|c| c.is_ascii_alphabetic())
  })
}

/// `CIDFont+F1`, `F2`, … (Microsoft Print To PDF and friends) carry no weight or style in the name.
fn is_anonymous_font(lower: &str) -> bool {
  let body = lower.strip_prefix("cidfont+").unwrap_or(lower);
  let mut chars = body.chars();
  matches!(chars.next(), Some('f')) && {
    let rest: String = chars.collect();
    !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit())
  }
}

impl FontInfo<'_> {
  pub(crate) fn width(&self, code: u32) -> f64 {
    self
      .widths
      .get(&code)
      .copied()
      .unwrap_or(if self.width_source == WidthSource::Estimate {
        0.5
      } else {
        self.default_width
      })
  }

  /// Decode one code's bytes to text: the `/ToUnicode` map first, then the encoding. Falls back
  /// to the byte as Latin-1 for simple fonts, and to U+FFFD for composite fonts without a usable map.
  pub(crate) fn decode(&self, bytes: &[u8]) -> String {
    if let Some(map) = &self.to_unicode
      && let Some(text) = map.lookup(bytes)
    {
      return text;
    }
    if let Some(enc) = &self.encoding
      && let Ok(s) = enc.bytes_to_string(bytes)
      && !s.is_empty()
    {
      return s;
    }
    if !self.two_byte
      && let [b] = bytes
    {
      return (*b as char).to_string();
    }
    "\u{FFFD}".to_string()
  }
}

#[cfg(test)]
mod tests;
