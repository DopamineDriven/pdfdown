use crate::types::{BoxType, PageBox, PdfMeta};
use lopdf::{Document, Object, ObjectId};
use std::collections::{BTreeMap, HashMap};

fn extract_info_string(dict: &lopdf::Dictionary, key: &[u8]) -> Option<String> {
  match dict.get(key).ok()? {
    Object::String(bytes, _) => {
      if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        // UTF-16 BOM
        let utf16: Vec<u16> = bytes[2..]
          .chunks_exact(2)
          .map(|c| u16::from_be_bytes([c[0], c[1]]))
          .collect();
        Some(String::from_utf16_lossy(&utf16))
      } else {
        Some(String::from_utf8_lossy(bytes).to_string())
      }
    }
    _ => None,
  }
}

/// Convert a PDF date string to ISO 8601.
/// Input format:  `D:YYYYMMDDHHmmSS+HH'mm'` (D: prefix optional, timezone optional)
/// Output format: `YYYY-MM-DDTHH:mm:SS+HH:mm` or `…Z`
fn pdf_date_to_iso8601(raw: &str) -> String {
  let s = raw.strip_prefix("D:").unwrap_or(raw);

  // Need at least YYYY (4 chars)
  if s.len() < 4 {
    return raw.to_string();
  }

  let yyyy = &s[..4];
  let mm = s.get(4..6).unwrap_or("01");
  let dd = s.get(6..8).unwrap_or("01");
  let hh = s.get(8..10).unwrap_or("00");
  let min = s.get(10..12).unwrap_or("00");
  let sec = s.get(12..14).unwrap_or("00");

  let tz_part = &s[14.min(s.len())..];
  let tz = if tz_part.is_empty() {
    String::new()
  } else if tz_part.starts_with('Z') {
    "Z".to_string()
  } else {
    // e.g. +05'30' or -06'00' → +05:30 or -06:00
    let cleaned = tz_part.replace('\'', "");
    if cleaned.len() >= 3 {
      let sign = &cleaned[..1];
      let tzh = &cleaned[1..3];
      let tzm = if cleaned.len() >= 5 {
        &cleaned[3..5]
      } else {
        "00"
      };
      format!("{sign}{tzh}:{tzm}")
    } else {
      String::new()
    }
  };

  format!("{yyyy}-{mm}-{dd}T{hh}:{min}:{sec}{tz}")
}

fn parse_page_box(obj: &Object) -> Option<[f64; 4]> {
  let arr = match obj {
    Object::Array(a) => a,
    _ => return None,
  };
  if arr.len() < 4 {
    return None;
  }
  let mut out = [0.0f64; 4];
  for (idx, slot) in out.iter_mut().enumerate().take(4) {
    *slot = match arr[idx] {
      Object::Integer(v) => v as f64,
      Object::Real(v) => v as f64,
      _ => return None,
    };
  }
  Some(out)
}

/// Walk the page tree to find an inheritable page box (e.g., /MediaBox, /CropBox).
/// Resolves indirect references — some PDFs store the box array via `Object::Reference`.
fn get_inherited_page_box(doc: &Document, page_id: ObjectId, key: &[u8]) -> Option<[f64; 4]> {
  let mut current_id = Some(page_id);
  while let Some(id) = current_id {
    let dict = doc.get_dictionary(id).ok()?;
    if let Ok(obj) = dict.get(key) {
      // Resolve indirect reference if the box value is stored as one
      let resolved = match obj {
        Object::Reference(ref_id) => doc.get_object(*ref_id).ok().cloned(),
        other => Some(other.clone()),
      };
      if let Some(ref val) = resolved
        && let Some(rect) = parse_page_box(val)
      {
        return Some(rect);
      }
    }
    // Walk up to /Parent
    current_id = dict.get(b"Parent").ok().and_then(|p| match p {
      Object::Reference(ref_id) => Some(*ref_id),
      _ => None,
    });
  }
  None
}

/// Key type for grouping page boxes by geometry.
/// Uses `to_bits()` so NaN/negative-zero edge cases hash correctly.
#[derive(Clone, PartialEq, Eq, Hash)]
struct PageBoxKey {
  left: u64,
  bottom: u64,
  right: u64,
  top: u64,
  box_type: u8, // 0=CropBox, 1=MediaBox, 2=Unknown
}

/// Intermediate representation before we decide which group is dominant.
struct PageBoxGroup {
  left: f64,
  bottom: f64,
  right: f64,
  top: f64,
  box_type: BoxType,
  page_nums: Vec<u32>,
}

pub(crate) fn extract_page_boxes(doc: &Document, pages: &BTreeMap<u32, ObjectId>) -> Vec<PageBox> {
  let mut page_entries: Vec<(u32, ObjectId)> = pages.iter().map(|(&k, &v)| (k, v)).collect();
  page_entries.sort_unstable_by_key(|(page, _)| *page);

  // Maintain insertion order via Vec + HashMap index
  let mut groups: Vec<PageBoxGroup> = Vec::new();
  let mut key_to_idx: HashMap<PageBoxKey, usize> = HashMap::new();

  for (page_num, page_id) in page_entries {
    let (box_type, rect) = if let Some(rect) = get_inherited_page_box(doc, page_id, b"CropBox") {
      (BoxType::CropBox, rect)
    } else if let Some(rect) = get_inherited_page_box(doc, page_id, b"MediaBox") {
      (BoxType::MediaBox, rect)
    } else {
      (BoxType::Unknown, [0.0, 0.0, 0.0, 0.0])
    };

    let (left, right) = if rect[0] <= rect[2] {
      (rect[0], rect[2])
    } else {
      (rect[2], rect[0])
    };
    let (bottom, top) = if rect[1] <= rect[3] {
      (rect[1], rect[3])
    } else {
      (rect[3], rect[1])
    };

    let key = PageBoxKey {
      left: left.to_bits(),
      bottom: bottom.to_bits(),
      right: right.to_bits(),
      top: top.to_bits(),
      box_type: match box_type {
        BoxType::CropBox => 0,
        BoxType::MediaBox => 1,
        BoxType::Unknown => 2,
      },
    };

    if let Some(&idx) = key_to_idx.get(&key) {
      groups[idx].page_nums.push(page_num);
    } else {
      let idx = groups.len();
      key_to_idx.insert(key, idx);
      groups.push(PageBoxGroup {
        left,
        bottom,
        right,
        top,
        box_type,
        page_nums: vec![page_num],
      });
    }
  }

  // Find the dominant group (most pages)
  let dominant_idx = groups
    .iter()
    .enumerate()
    .max_by_key(|(_, g)| g.page_nums.len())
    .map(|(i, _)| i)
    .unwrap_or(0);

  groups
    .into_iter()
    .enumerate()
    .map(|(i, g)| {
      let count = g.page_nums.len() as u32;
      let pages = if i == dominant_idx {
        None
      } else {
        Some(g.page_nums)
      };
      PageBox {
        page_count: count,
        left: g.left,
        bottom: g.bottom,
        right: g.right,
        top: g.top,
        width: g.right - g.left,
        height: g.top - g.bottom,
        box_type: g.box_type,
        pages,
      }
    })
    .collect()
}

pub(crate) fn extract_metadata(doc: &Document) -> PdfMeta {
  let pages = doc.get_pages();
  let page_count = pages.len() as u32;
  let version = doc.version.clone();
  let is_linearized = doc.trailer.get(b"Linearized").is_ok();

  let info_dict = doc.trailer.get(b"Info").ok().and_then(|obj| match obj {
    Object::Reference(id) => doc.get_dictionary(*id).ok(),
    _ => None,
  });

  let (creator, producer, creation_date, modification_date) = match info_dict {
    Some(d) => (
      extract_info_string(d, b"Creator"),
      extract_info_string(d, b"Producer"),
      extract_info_string(d, b"CreationDate").map(|s| pdf_date_to_iso8601(&s)),
      extract_info_string(d, b"ModDate").map(|s| pdf_date_to_iso8601(&s)),
    ),
    None => (None, None, None, None),
  };

  let page_boxes = extract_page_boxes(doc, &pages);

  PdfMeta {
    page_count,
    version,
    is_linearized,
    creator,
    producer,
    creation_date,
    modification_date,
    page_boxes,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_page_box_valid_integers() {
    let obj = Object::Array(vec![
      Object::Integer(0),
      Object::Integer(0),
      Object::Integer(612),
      Object::Integer(792),
    ]);
    let result = parse_page_box(&obj);
    assert_eq!(result, Some([0.0, 0.0, 612.0, 792.0]));
  }

  #[test]
  fn parse_page_box_valid_reals() {
    let obj = Object::Array(vec![
      Object::Real(0.0),
      Object::Real(0.0),
      Object::Real(595.0),
      Object::Real(842.0),
    ]);
    let result = parse_page_box(&obj);
    assert_eq!(result, Some([0.0, 0.0, 595.0, 842.0]));
  }

  #[test]
  fn parse_page_box_mixed_types() {
    let obj = Object::Array(vec![
      Object::Integer(0),
      Object::Real(0.5),
      Object::Integer(612),
      Object::Real(792.0),
    ]);
    let result = parse_page_box(&obj);
    assert_eq!(result, Some([0.0, 0.5, 612.0, 792.0]));
  }

  #[test]
  fn parse_page_box_too_short() {
    let obj = Object::Array(vec![Object::Integer(0), Object::Integer(0)]);
    assert_eq!(parse_page_box(&obj), None);
  }

  #[test]
  fn parse_page_box_non_numeric() {
    let obj = Object::Array(vec![
      Object::Integer(0),
      Object::Integer(0),
      Object::Name(b"bad".to_vec()),
      Object::Integer(792),
    ]);
    assert_eq!(parse_page_box(&obj), None);
  }

  #[test]
  fn parse_page_box_not_array() {
    let obj = Object::Integer(42);
    assert_eq!(parse_page_box(&obj), None);
  }

  #[test]
  fn parse_page_box_extra_elements_ignored() {
    let obj = Object::Array(vec![
      Object::Integer(0),
      Object::Integer(0),
      Object::Integer(612),
      Object::Integer(792),
      Object::Integer(999),
    ]);
    // Only first 4 used
    assert_eq!(parse_page_box(&obj), Some([0.0, 0.0, 612.0, 792.0]));
  }
}
