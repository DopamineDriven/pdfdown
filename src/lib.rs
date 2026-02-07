#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi::{Env, Task};
use napi_derive::napi;

use image::{DynamicImage, ImageBuffer, ImageFormat};
use lopdf::{Document, Object, ObjectId};
use rayon::prelude::*;
use std::collections::HashSet;
use std::io::Cursor;
use std::sync::Arc;

// ── Step 1: Compile-time assertion that lopdf::Document is Send + Sync ──
const _: () = {
  fn assert_send_sync<T: Send + Sync>() {}
  fn check() {
    assert_send_sync::<lopdf::Document>();
  }
};

// ── Napi object types (JS boundary) ─────────────────────────────

#[napi(object)]
pub struct PageText {
  pub page: u32,
  pub text: String,
}

#[napi(object)]
pub struct StructuredPageText {
  pub page: u32,
  pub header: String,
  pub body: String,
  pub footer: String,
}

#[cfg(feature = "ocr")]
#[napi(string_enum)]
pub enum TextSource {
  Native,
  Ocr,
}

#[cfg(feature = "ocr")]
#[napi(object)]
pub struct OcrPageText {
  pub page: u32,
  pub text: String,
  pub source: TextSource,
}

#[cfg(feature = "ocr")]
#[napi(object)]
pub struct OcrOptions {
  pub lang: Option<String>,
  pub min_text_length: Option<u32>,
  pub max_threads: Option<u32>,
}

#[napi(object)]
pub struct PdfMeta {
  pub page_count: u32,
  pub version: String,
  pub is_linearized: bool,
  pub creator: Option<String>,
  pub producer: Option<String>,
  pub creation_date: Option<String>,
  pub modification_date: Option<String>,
  pub page_boxes: Vec<PageBox>,
}

#[napi(string_enum)]
pub enum BoxType {
  CropBox,
  MediaBox,
  Unknown,
}

#[napi(object)]
pub struct PageBox {
  /// Number of pages that share these dimensions.
  pub page_count: u32,
  pub left: f64,
  pub bottom: f64,
  pub right: f64,
  pub top: f64,
  pub width: f64,
  pub height: f64,
  pub box_type: BoxType,
  /// Present only on non-dominant boxes — lists the specific pages with these
  /// dimensions. `None` on the first (most frequent) entry means "all pages
  /// not listed in any other entry's `pages` array."
  pub pages: Option<Vec<u32>>,
}

#[napi(object)]
pub struct PageImage {
  pub page: u32,
  pub image_index: u32,
  pub width: u32,
  pub height: u32,
  pub data: Buffer,
  pub color_space: String,
  pub bits_per_component: u32,
  pub filter: String,
  pub xobject_name: String,
  pub object_id: String,
}

#[napi(object)]
pub struct PageAnnotation {
  pub page: u32,
  pub subtype: String,
  pub rect: Vec<f64>,
  pub uri: Option<String>,
  pub dest: Option<String>,
  pub content: Option<String>,
}

#[napi(object)]
pub struct PdfDocument {
  pub version: String,
  pub is_linearized: bool,
  pub page_count: u32,
  pub creator: Option<String>,
  pub producer: Option<String>,
  pub creation_date: Option<String>,
  pub modification_date: Option<String>,
  pub page_boxes: Vec<PageBox>,
  pub total_images: u32,
  pub total_annotations: u32,
  pub image_pages: Vec<u32>,
  pub annotation_pages: Vec<u32>,
  pub text: Vec<PageText>,
  pub structured_text: Vec<StructuredPageText>,
  pub images: Vec<PageImage>,
  pub annotations: Vec<PageAnnotation>,
}

#[cfg(feature = "ocr")]
#[napi(object)]
pub struct OcrStructuredPageText {
  pub page: u32,
  pub header: String,
  pub body: String,
  pub footer: String,
  pub source: TextSource,
}

#[cfg(feature = "ocr")]
#[napi(object)]
pub struct PdfDocumentOcr {
  pub version: String,
  pub is_linearized: bool,
  pub page_count: u32,
  pub creator: Option<String>,
  pub producer: Option<String>,
  pub creation_date: Option<String>,
  pub modification_date: Option<String>,
  pub page_boxes: Vec<PageBox>,
  pub total_images: u32,
  pub total_annotations: u32,
  pub image_pages: Vec<u32>,
  pub annotation_pages: Vec<u32>,
  pub text: Vec<OcrPageText>,
  pub structured_text: Vec<OcrStructuredPageText>,
  pub images: Vec<PageImage>,
  pub annotations: Vec<PageAnnotation>,
}

// ── Step 2: Internal type — no napi types, safe for any thread ──

pub struct RawPageImage {
  pub page: u32,
  pub image_index: u32,
  pub width: u32,
  pub height: u32,
  pub data: Vec<u8>,
  pub color_space: String,
  pub bits_per_component: u32,
  pub filter: String,
  pub xobject_name: String,
  pub object_id: String,
}

impl From<RawPageImage> for PageImage {
  fn from(r: RawPageImage) -> Self {
    PageImage {
      page: r.page,
      image_index: r.image_index,
      width: r.width,
      height: r.height,
      data: r.data.into(),
      color_space: r.color_space,
      bits_per_component: r.bits_per_component,
      filter: r.filter,
      xobject_name: r.xobject_name,
      object_id: r.object_id,
    }
  }
}

pub struct RawPdfDocument {
  pub meta: PdfMeta,
  pub text: Vec<PageText>,
  pub structured_text: Vec<StructuredPageText>,
  pub images: Vec<RawPageImage>,
  pub annotations: Vec<PageAnnotation>,
  pub image_pages: Vec<u32>,
  pub annotation_pages: Vec<u32>,
}

impl From<RawPdfDocument> for PdfDocument {
  fn from(r: RawPdfDocument) -> Self {
    let total_images = r.images.len() as u32;
    let total_annotations = r.annotations.len() as u32;
    PdfDocument {
      version: r.meta.version,
      is_linearized: r.meta.is_linearized,
      page_count: r.meta.page_count,
      creator: r.meta.creator,
      producer: r.meta.producer,
      creation_date: r.meta.creation_date,
      modification_date: r.meta.modification_date,
      page_boxes: r.meta.page_boxes,
      total_images,
      total_annotations,
      image_pages: r.image_pages,
      annotation_pages: r.annotation_pages,
      text: r.text,
      structured_text: r.structured_text,
      images: r.images.into_iter().map(PageImage::from).collect(),
      annotations: r.annotations,
    }
  }
}

#[cfg(feature = "ocr")]
pub struct RawPdfDocumentOcr {
  pub meta: PdfMeta,
  pub text: Vec<OcrPageText>,
  pub structured_text: Vec<OcrStructuredPageText>,
  pub images: Vec<RawPageImage>,
  pub annotations: Vec<PageAnnotation>,
  pub image_pages: Vec<u32>,
  pub annotation_pages: Vec<u32>,
}

#[cfg(feature = "ocr")]
impl From<RawPdfDocumentOcr> for PdfDocumentOcr {
  fn from(r: RawPdfDocumentOcr) -> Self {
    let total_images = r.images.len() as u32;
    let total_annotations = r.annotations.len() as u32;
    PdfDocumentOcr {
      version: r.meta.version,
      is_linearized: r.meta.is_linearized,
      page_count: r.meta.page_count,
      creator: r.meta.creator,
      producer: r.meta.producer,
      creation_date: r.meta.creation_date,
      modification_date: r.meta.modification_date,
      page_boxes: r.meta.page_boxes,
      total_images,
      total_annotations,
      image_pages: r.image_pages,
      annotation_pages: r.annotation_pages,
      text: r.text,
      structured_text: r.structured_text,
      images: r.images.into_iter().map(PageImage::from).collect(),
      annotations: r.annotations,
    }
  }
}

// ── Shared helpers ──────────────────────────────────────────────

fn load_doc(buf: &[u8]) -> Result<Document> {
  Document::load_mem(buf).map_err(|e| Error::from_reason(format!("Failed to load PDF: {e}")))
}

fn extract_text(doc: &Document) -> Result<Vec<PageText>> {
  let pages = doc.get_pages();
  let page_count_str = pages.len().to_string();
  let page_nums: Vec<u32> = pages.keys().copied().collect();
  let mut results: Vec<PageText> = page_nums
    .par_iter()
    .map(|&page_num| {
      let raw = doc.extract_text(&[page_num]).unwrap_or_default();
      let text = strip_footer_artifacts(&raw, &page_count_str);
      PageText {
        page: page_num,
        text,
      }
    })
    .collect();
  results.sort_unstable_by_key(|p| p.page);
  Ok(results)
}

/// Normalize a line for header/footer comparison: trim whitespace and replace
/// contiguous digit sequences with `<NUM>` so "Page 1" matches "Page 42".
fn normalize_header_footer_line(line: &str) -> String {
  let trimmed = line.trim();
  let mut out = String::with_capacity(trimmed.len());
  let mut in_digits = false;
  for ch in trimmed.chars() {
    if ch.is_ascii_digit() {
      if !in_digits {
        out.push_str("<NUM>");
        in_digits = true;
      }
    } else {
      in_digits = false;
      out.push(ch);
    }
  }
  out
}

/// Strip Chromium footer artifacts from extracted text.
///
/// Chromium's Skia PDF renderer writes page footers (e.g., `1 / 38`) as 2-3
/// separate text operations. `lopdf::extract_text` concatenates these in
/// content-stream order, causing orphaned fragments like ` / \n38\n` to appear
/// mid-text on every page. This function removes the known pattern: a line
/// containing just `/` followed by a line containing just the total page count.
fn strip_footer_artifacts(text: &str, page_count_str: &str) -> String {
  let lines: Vec<&str> = text.lines().collect();
  if lines.len() < 2 {
    return text.to_string();
  }
  let mut skip = vec![false; lines.len()];
  for i in 0..lines.len() - 1 {
    if lines[i].trim() == "/" && lines[i + 1].trim() == page_count_str {
      skip[i] = true;
      skip[i + 1] = true;
    }
  }
  if !skip.iter().any(|&s| s) {
    return text.to_string();
  }
  lines
    .iter()
    .zip(skip.iter())
    .filter(|&(_, &s)| !s)
    .map(|(&line, _)| line)
    .collect::<Vec<_>>()
    .join("\n")
}

/// Detect repeated header/footer lines across pages and split each page's text
/// into header, body, and footer sections.
fn detect_headers_footers(pages: &[PageText]) -> Vec<StructuredPageText> {
  // For fewer than 3 pages, no meaningful detection — return everything as body
  if pages.len() < 3 {
    return pages
      .iter()
      .map(|p| StructuredPageText {
        page: p.page,
        header: String::new(),
        body: p.text.clone(),
        footer: String::new(),
      })
      .collect();
  }

  let threshold = (pages.len() as f64 * 0.6).ceil() as usize;
  let max_check = 3usize; // check up to 3 lines from top/bottom

  // Split each page into lines
  let page_lines: Vec<Vec<&str>> = pages.iter().map(|p| p.text.lines().collect()).collect();

  // Detect header line count: for each position 0..max_check, check if the
  // normalized line at that position appears on >= threshold pages
  let mut header_count = 0usize;
  for pos in 0..max_check {
    let mut freq = std::collections::HashMap::<String, usize>::new();
    for lines in &page_lines {
      if let Some(&line) = lines.get(pos) {
        let norm = normalize_header_footer_line(line);
        if !norm.is_empty() {
          *freq.entry(norm).or_insert(0) += 1;
        }
      }
    }
    if freq.values().any(|&c| c >= threshold) {
      header_count = pos + 1;
    } else {
      break;
    }
  }

  // Detect footer line count (from the bottom)
  let mut footer_count = 0usize;
  for pos in 0..max_check {
    let mut freq = std::collections::HashMap::<String, usize>::new();
    for lines in &page_lines {
      if lines.len() > pos {
        let idx = lines.len() - 1 - pos;
        // Don't overlap with headers
        if idx >= header_count {
          let norm = normalize_header_footer_line(lines[idx]);
          if !norm.is_empty() {
            *freq.entry(norm).or_insert(0) += 1;
          }
        }
      }
    }
    if freq.values().any(|&c| c >= threshold) {
      footer_count = pos + 1;
    } else {
      break;
    }
  }

  pages
    .iter()
    .zip(page_lines.iter())
    .map(|(p, lines)| {
      let total = lines.len();
      let h_end = header_count.min(total);
      let f_start = if footer_count > 0 {
        total.saturating_sub(footer_count).max(h_end)
      } else {
        total
      };

      let header = lines[..h_end].join("\n");
      let body = lines[h_end..f_start].join("\n");
      let footer = lines[f_start..].join("\n");

      StructuredPageText {
        page: p.page,
        header,
        body,
        footer,
      }
    })
    .collect()
}

fn extract_structured_text(doc: &Document) -> Result<Vec<StructuredPageText>> {
  let pages = extract_text(doc)?;
  Ok(detect_headers_footers(&pages))
}

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
      if let Some(ref val) = resolved {
        if let Some(rect) = parse_page_box(val) {
          return Some(rect);
        }
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

fn extract_page_boxes(
  doc: &Document,
  pages: &std::collections::BTreeMap<u32, ObjectId>,
) -> Vec<PageBox> {
  let mut page_entries: Vec<(u32, ObjectId)> = pages.iter().map(|(&k, &v)| (k, v)).collect();
  page_entries.sort_unstable_by_key(|(page, _)| *page);

  // Maintain insertion order via Vec + HashMap index
  let mut groups: Vec<PageBoxGroup> = Vec::new();
  let mut key_to_idx: std::collections::HashMap<PageBoxKey, usize> =
    std::collections::HashMap::new();

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

fn extract_metadata(doc: &Document) -> PdfMeta {
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

fn extract_images_raw(doc: &Document) -> Vec<RawPageImage> {
  let pages = doc.get_pages();
  let page_entries: Vec<(u32, ObjectId)> = pages.iter().map(|(&k, &v)| (k, v)).collect();
  let mut results: Vec<RawPageImage> = page_entries
    .par_iter()
    .flat_map(|&(page_num, page_id)| collect_page_images_raw(doc, page_id, page_num))
    .collect();
  results.sort_unstable_by_key(|r| (r.page, r.image_index));
  results
}

fn collect_page_annotations(
  doc: &Document,
  page_id: ObjectId,
  page_num: u32,
) -> Vec<PageAnnotation> {
  let annots = match doc.get_page_annotations(page_id) {
    Ok(a) => a,
    Err(_) => return Vec::new(),
  };

  let mut results = Vec::new();
  for annot in annots {
    let subtype = annot
      .get(b"Subtype")
      .ok()
      .and_then(|v| {
        if let Object::Name(n) = v {
          Some(String::from_utf8_lossy(n).to_string())
        } else {
          None
        }
      })
      .unwrap_or_default();

    let rect = annot
      .get(b"Rect")
      .ok()
      .and_then(|v| {
        if let Object::Array(arr) = v {
          Some(
            arr
              .iter()
              .filter_map(|o| match o {
                Object::Real(f) => Some(*f as f64),
                Object::Integer(i) => Some(*i as f64),
                _ => None,
              })
              .collect::<Vec<f64>>(),
          )
        } else {
          None
        }
      })
      .unwrap_or_default();

    // Extract URI from /A action dictionary
    let uri = annot.get(b"A").ok().and_then(|action| {
      let action_dict = match action {
        Object::Dictionary(d) => Some(d),
        Object::Reference(id) => doc.get_dictionary(*id).ok(),
        _ => None,
      }?;
      let uri_obj = action_dict.get(b"URI").ok()?;
      match uri_obj {
        Object::String(bytes, _) => Some(String::from_utf8_lossy(bytes).to_string()),
        _ => None,
      }
    });

    // Extract /Dest (named or direct destination)
    let dest = annot.get(b"Dest").ok().and_then(|d| match d {
      Object::String(bytes, _) => Some(String::from_utf8_lossy(bytes).to_string()),
      Object::Name(n) => Some(String::from_utf8_lossy(n).to_string()),
      _ => None,
    });

    // Extract /Contents (tooltip / alt text)
    let content = annot.get(b"Contents").ok().and_then(|c| match c {
      Object::String(bytes, _) => Some(String::from_utf8_lossy(bytes).to_string()),
      _ => None,
    });

    results.push(PageAnnotation {
      page: page_num,
      subtype,
      rect,
      uri,
      dest,
      content,
    });
  }

  results
}

fn extract_annotations(doc: &Document) -> Vec<PageAnnotation> {
  let pages = doc.get_pages();
  let page_entries: Vec<(u32, ObjectId)> = pages.iter().map(|(&k, &v)| (k, v)).collect();
  let mut results: Vec<PageAnnotation> = page_entries
    .par_iter()
    .flat_map(|&(page_num, page_id)| collect_page_annotations(doc, page_id, page_num))
    .collect();
  results.sort_unstable_by_key(|a| a.page);
  results
}

fn extract_all(doc: &Document) -> Result<RawPdfDocument> {
  let meta = extract_metadata(doc);
  let ((text, images), annotations) = rayon::join(
    || rayon::join(|| extract_text(doc), || extract_images_raw(doc)),
    || extract_annotations(doc),
  );
  let text = text?;
  let structured_text = detect_headers_footers(&text);

  let mut image_pages: Vec<u32> = images
    .iter()
    .map(|i| i.page)
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();
  image_pages.sort_unstable();

  let mut annotation_pages: Vec<u32> = annotations
    .iter()
    .map(|a| a.page)
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();
  annotation_pages.sort_unstable();

  Ok(RawPdfDocument {
    meta,
    text,
    structured_text,
    images,
    annotations,
    image_pages,
    annotation_pages,
  })
}

// ── OCR fallback (feature-gated) ────────────────────────────────

#[cfg(feature = "ocr")]
fn normalize_max_threads(v: Option<u32>) -> u32 {
  let default = 4u32;
  let max = std::thread::available_parallelism()
    .map(|n| n.get() as u32)
    .unwrap_or(default);
  v.unwrap_or(default).clamp(1, max)
}

#[cfg(feature = "ocr")]
fn get_ocr_pool(threads: usize) -> Arc<rayon::ThreadPool> {
  use std::collections::HashMap;
  use std::sync::{Mutex, OnceLock};

  static POOLS: OnceLock<Mutex<HashMap<usize, Arc<rayon::ThreadPool>>>> = OnceLock::new();
  let map = POOLS.get_or_init(|| Mutex::new(HashMap::new()));
  let mut guard = map.lock().unwrap();
  Arc::clone(guard.entry(threads).or_insert_with(|| {
    Arc::new(
      rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("failed to build OCR thread pool"),
    )
  }))
}

/// Decode all image XObjects on a page to DynamicImages (no PNG encoding).
/// Used by OCR to avoid the PNG encode→decode roundtrip.
#[cfg(feature = "ocr")]
fn collect_page_decoded_images(doc: &Document, page_id: ObjectId) -> Vec<DynamicImage> {
  let mut decoded = Vec::new();

  let xobjects = match get_page_xobjects(doc, page_id) {
    Some(x) => x,
    None => return decoded,
  };

  let referenced_names = get_referenced_xobject_names(doc, page_id);

  for (name, obj_ref) in xobjects.iter() {
    if !referenced_names.is_empty() && !referenced_names.contains(name) {
      continue;
    }

    let obj_id = match obj_ref {
      Object::Reference(id) => *id,
      _ => continue,
    };

    let stream = match doc.get_object(obj_id) {
      Ok(Object::Stream(s)) => s,
      _ => continue,
    };

    let subtype = stream.dict.get(b"Subtype").ok().and_then(|v| {
      if let Object::Name(n) = v {
        Some(n.as_slice())
      } else {
        None
      }
    });
    if subtype != Some(b"Image") {
      continue;
    }

    let width = get_dict_int(&stream.dict, b"Width").unwrap_or(0) as u32;
    let height = get_dict_int(&stream.dict, b"Height").unwrap_or(0) as u32;
    let bpc = get_dict_int(&stream.dict, b"BitsPerComponent").unwrap_or(8) as u32;
    if width == 0 || height == 0 {
      continue;
    }

    let color_space = resolve_color_space(&stream.dict, doc);
    let filter = resolve_filter(&stream.dict);

    let content = match filter.as_str() {
      "DCTDecode" | "JPXDecode" => stream.content.clone(),
      _ => {
        let mut s = stream.clone();
        if s.decompress().is_ok() {
          s.content
        } else {
          stream.content.clone()
        }
      }
    };

    // Skip SMask for OCR — to_rgb8() drops alpha anyway
    if let Some(img) =
      decode_xobject_to_dynamic_image(&content, width, height, bpc, &color_space, &filter, None)
    {
      decoded.push(img);
    }
  }

  decoded
}

/// Auto-detect tessdata path, cached for the lifetime of the process.
/// Checks `TESSDATA_PREFIX` env var first (user override), then falls back to
/// parsing the output of `tesseract --list-langs` (e.g.
/// `List of available languages in "/usr/share/tesseract-ocr/5/tessdata/" (161):`).
/// Returns `None` if neither source yields a path, letting tesseract use its
/// compiled-in default.
#[cfg(feature = "ocr")]
fn get_tessdata_prefix() -> Option<&'static str> {
  use std::sync::OnceLock;
  static TESSDATA_PATH: OnceLock<Option<String>> = OnceLock::new();

  TESSDATA_PATH
    .get_or_init(|| {
      // User-provided override takes priority
      if let Ok(path) = std::env::var("TESSDATA_PREFIX") {
        return Some(path);
      }

      // Auto-detect from tesseract --list-langs
      let output = std::process::Command::new("tesseract")
        .arg("--list-langs")
        .output()
        .ok()?;

      // tesseract writes the path header to stderr
      let stderr = String::from_utf8_lossy(&output.stderr);
      let text = if stderr.contains('"') {
        stderr
      } else {
        String::from_utf8_lossy(&output.stdout)
      };

      // Parse: `List of available languages in "/path/to/tessdata/" (N):`
      let start = text.find('"')?;
      let end = text[start + 1..].find('"')?;
      Some(text[start + 1..start + 1 + end].to_string())
    })
    .as_deref()
}

#[cfg(feature = "ocr")]
fn ocr_page_images(doc: &Document, page_id: ObjectId, lang: &str) -> String {
  let datapath = get_tessdata_prefix().unwrap_or("");
  let images = collect_page_decoded_images(doc, page_id);
  let mut texts = Vec::new();

  for dyn_img in &images {
    let rgb = dyn_img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let pixels = rgb.as_raw();

    let tess = tesseract_rs::TesseractAPI::new();
    if tess.init(datapath, lang).is_err() {
      continue;
    }
    if tess
      .set_image(pixels, w as i32, h as i32, 3, (w * 3) as i32)
      .is_err()
    {
      continue;
    }
    if let Ok(text) = tess.get_utf8_text() {
      let trimmed = text.trim();
      if !trimmed.is_empty() {
        texts.push(trimmed.to_string());
      }
    }
  }

  texts.join("\n")
}

#[cfg(feature = "ocr")]
fn extract_text_with_ocr(
  doc: &Document,
  lang: &str,
  min_len: u32,
  max_threads: u32,
) -> Result<Vec<OcrPageText>> {
  let pages = doc.get_pages();
  let page_count_str = pages.len().to_string();
  let page_entries: Vec<(u32, ObjectId)> = pages.iter().map(|(&k, &v)| (k, v)).collect();

  let pool = get_ocr_pool(max_threads as usize);

  let mut results: Vec<OcrPageText> = pool.install(|| {
    page_entries
      .par_iter()
      .map(|&(page_num, page_id)| {
        let raw = doc.extract_text(&[page_num]).unwrap_or_default();
        let native = strip_footer_artifacts(&raw, &page_count_str);
        let non_ws: usize = native.chars().filter(|c| !c.is_whitespace()).count();
        if non_ws >= min_len as usize {
          OcrPageText {
            page: page_num,
            text: native,
            source: TextSource::Native,
          }
        } else {
          let ocr_text = ocr_page_images(doc, page_id, lang);
          OcrPageText {
            page: page_num,
            text: ocr_text,
            source: TextSource::Ocr,
          }
        }
      })
      .collect()
  });
  results.sort_unstable_by_key(|r| r.page);
  Ok(results)
}

#[cfg(feature = "ocr")]
fn detect_headers_footers_ocr(pages: &[OcrPageText]) -> Vec<OcrStructuredPageText> {
  // Convert to PageText for header/footer detection
  let as_page_text: Vec<PageText> = pages
    .iter()
    .map(|p| PageText {
      page: p.page,
      text: p.text.clone(),
    })
    .collect();
  let structured = detect_headers_footers(&as_page_text);
  // Zip back with source info
  structured
    .into_iter()
    .zip(pages.iter())
    .map(|(s, ocr)| OcrStructuredPageText {
      page: s.page,
      header: s.header,
      body: s.body,
      footer: s.footer,
      source: match ocr.source {
        TextSource::Native => TextSource::Native,
        TextSource::Ocr => TextSource::Ocr,
      },
    })
    .collect()
}

#[cfg(feature = "ocr")]
fn extract_all_with_ocr(
  doc: &Document,
  lang: &str,
  min_len: u32,
  max_threads: u32,
) -> Result<RawPdfDocumentOcr> {
  let meta = extract_metadata(doc);
  let (text, (images, annotations)) = rayon::join(
    || extract_text_with_ocr(doc, lang, min_len, max_threads),
    || rayon::join(|| extract_images_raw(doc), || extract_annotations(doc)),
  );
  let text = text?;
  let structured_text = detect_headers_footers_ocr(&text);

  let mut image_pages: Vec<u32> = images
    .iter()
    .map(|i| i.page)
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();
  image_pages.sort_unstable();

  let mut annotation_pages: Vec<u32> = annotations
    .iter()
    .map(|a| a.page)
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();
  annotation_pages.sort_unstable();

  Ok(RawPdfDocumentOcr {
    meta,
    text,
    structured_text,
    images,
    annotations,
    image_pages,
    annotation_pages,
  })
}

// ── Standalone sync functions ───────────────────────────────────

#[napi]
pub fn extract_text_per_page(buffer: Buffer) -> Result<Vec<PageText>> {
  let doc = load_doc(buffer.as_ref())?;
  extract_text(&doc)
}

#[napi]
pub fn pdf_metadata(buffer: Buffer) -> Result<PdfMeta> {
  let doc = load_doc(buffer.as_ref())?;
  Ok(extract_metadata(&doc))
}

#[napi]
pub fn extract_annotations_per_page(buffer: Buffer) -> Result<Vec<PageAnnotation>> {
  let doc = load_doc(buffer.as_ref())?;
  Ok(extract_annotations(&doc))
}

#[napi]
pub fn extract_images_per_page(buffer: Buffer) -> Result<Vec<PageImage>> {
  let doc = load_doc(buffer.as_ref())?;
  Ok(
    extract_images_raw(&doc)
      .into_iter()
      .map(PageImage::from)
      .collect(),
  )
}

#[napi]
pub fn pdf_document(buffer: Buffer) -> Result<PdfDocument> {
  let doc = load_doc(buffer.as_ref())?;
  Ok(PdfDocument::from(extract_all(&doc)?))
}

#[napi]
pub fn extract_structured_text_per_page(buffer: Buffer) -> Result<Vec<StructuredPageText>> {
  let doc = load_doc(buffer.as_ref())?;
  extract_structured_text(&doc)
}

#[cfg(feature = "ocr")]
#[napi]
pub fn extract_text_with_ocr_per_page(
  buffer: Buffer,
  opts: Option<OcrOptions>,
) -> Result<Vec<OcrPageText>> {
  let doc = load_doc(buffer.as_ref())?;
  let lang = opts
    .as_ref()
    .and_then(|o| o.lang.as_deref())
    .unwrap_or("eng");
  let min_len = opts.as_ref().and_then(|o| o.min_text_length).unwrap_or(1);
  let max_threads = normalize_max_threads(opts.as_ref().and_then(|o| o.max_threads));
  extract_text_with_ocr(&doc, lang, min_len, max_threads)
}

#[cfg(feature = "ocr")]
#[napi]
pub fn pdf_document_ocr(buffer: Buffer, opts: Option<OcrOptions>) -> Result<PdfDocumentOcr> {
  let doc = load_doc(buffer.as_ref())?;
  let lang = opts
    .as_ref()
    .and_then(|o| o.lang.as_deref())
    .unwrap_or("eng");
  let min_len = opts.as_ref().and_then(|o| o.min_text_length).unwrap_or(1);
  let max_threads = normalize_max_threads(opts.as_ref().and_then(|o| o.max_threads));
  Ok(PdfDocumentOcr::from(extract_all_with_ocr(
    &doc,
    lang,
    min_len,
    max_threads,
  )?))
}

// ── Image extraction internals ──────────────────────────────────

fn collect_page_images_raw(doc: &Document, page_id: ObjectId, page_num: u32) -> Vec<RawPageImage> {
  let mut images = Vec::new();

  // Get XObjects from page resources (with parent inheritance)
  let xobjects = match get_page_xobjects(doc, page_id) {
    Some(x) => x,
    None => return images,
  };

  // Get the set of XObject names actually referenced by Do operators in the content stream
  let referenced_names = get_referenced_xobject_names(doc, page_id);

  let mut img_index = 0u32;

  for (name, obj_ref) in xobjects.iter() {
    // Only process XObjects actually painted on the page via Do operators
    if !referenced_names.is_empty() && !referenced_names.contains(name) {
      continue;
    }

    let obj_id = match obj_ref {
      Object::Reference(id) => *id,
      _ => continue,
    };

    let stream = match doc.get_object(obj_id) {
      Ok(Object::Stream(s)) => s,
      _ => continue,
    };

    // Only process Image XObjects
    let subtype = stream.dict.get(b"Subtype").ok().and_then(|v| {
      if let Object::Name(n) = v {
        Some(n.as_slice())
      } else {
        None
      }
    });

    if subtype != Some(b"Image") {
      continue;
    }

    let width = get_dict_int(&stream.dict, b"Width").unwrap_or(0) as u32;
    let height = get_dict_int(&stream.dict, b"Height").unwrap_or(0) as u32;
    let bpc = get_dict_int(&stream.dict, b"BitsPerComponent").unwrap_or(8) as u32;

    if width == 0 || height == 0 {
      continue;
    }

    let color_space = resolve_color_space(&stream.dict, doc);
    let filter = resolve_filter(&stream.dict);

    let channels: u32 = match color_space.as_str() {
      "DeviceRGB" | "ICCBased3" | "CalRGB" => 3,
      "DeviceGray" | "ICCBased1" | "CalGray" => 1,
      "DeviceCMYK" | "ICCBased4" => 4,
      _ => 3,
    };

    // Step 4: Skip the full stream clone for DCT/JPX — they're already in their
    // target encoded format and don't need lopdf decompression.
    let content = match filter.as_str() {
      "DCTDecode" | "JPXDecode" => stream.content.clone(),
      _ => decompress_stream_content(doc, stream, width, height, channels, bpc),
    };

    // Check for SMask (alpha channel)
    let smask_data = get_smask_data(doc, &stream.dict);

    let png_data = match encode_to_png(
      &content,
      width,
      height,
      bpc,
      &color_space,
      &filter,
      smask_data.as_deref(),
    ) {
      Some(data) => data,
      None => continue,
    };

    let xobject_name = String::from_utf8_lossy(name).to_string();
    let object_id_str = format!("{} {} obj", obj_id.0, obj_id.1);

    images.push(RawPageImage {
      page: page_num,
      image_index: img_index,
      width,
      height,
      data: png_data,
      color_space,
      bits_per_component: bpc,
      filter,
      xobject_name,
      object_id: object_id_str,
    });

    img_index += 1;
  }

  images
}

/// Walk the page tree to find /Resources (handles inheritance from /Parent)
fn get_page_xobjects(doc: &Document, page_id: ObjectId) -> Option<lopdf::Dictionary> {
  let resources = get_inherited_resources(doc, page_id)?;
  let xobject_obj = resources.get(b"XObject").ok()?;
  resolve_to_dict(doc, xobject_obj)
}

fn get_inherited_resources(doc: &Document, page_id: ObjectId) -> Option<lopdf::Dictionary> {
  let mut current_id = Some(page_id);
  while let Some(id) = current_id {
    let dict = doc.get_dictionary(id).ok()?;
    if let Ok(resources_obj) = dict.get(b"Resources") {
      return resolve_to_dict(doc, resources_obj);
    }
    // Walk up to /Parent
    current_id = dict.get(b"Parent").ok().and_then(|p| match p {
      Object::Reference(ref_id) => Some(*ref_id),
      _ => None,
    });
  }
  None
}

/// Parse the page content stream to find XObject names referenced by `Do` operators.
/// This filters out XObjects that are defined in Resources but never actually painted.
fn get_referenced_xobject_names(doc: &Document, page_id: ObjectId) -> HashSet<Vec<u8>> {
  let mut names = HashSet::new();

  let page_dict = match doc.get_dictionary(page_id) {
    Ok(d) => d,
    Err(_) => return names,
  };

  let contents = match page_dict.get(b"Contents") {
    Ok(c) => c,
    Err(_) => return names,
  };

  let stream_ids: Vec<ObjectId> = match contents {
    Object::Reference(id) => vec![*id],
    Object::Array(arr) => arr
      .iter()
      .filter_map(|o| {
        if let Object::Reference(id) = o {
          Some(*id)
        } else {
          None
        }
      })
      .collect(),
    _ => return names,
  };

  let mut all_bytes = Vec::new();
  for stream_id in stream_ids {
    if let Ok(Object::Stream(s)) = doc.get_object(stream_id) {
      let mut s = s.clone();
      let _ = s.decompress();
      all_bytes.extend_from_slice(&s.content);
    }
  }

  if let Ok(content) = lopdf::content::Content::decode(&all_bytes) {
    for op in &content.operations {
      if op.operator == "Do"
        && let Some(Object::Name(name)) = op.operands.first()
      {
        names.insert(name.clone());
      }
    }
  }

  names
}

/// Resolve /DecodeParms from a stream dictionary, following indirect references.
fn resolve_decode_parms(doc: &Document, dict: &lopdf::Dictionary) -> Option<lopdf::Dictionary> {
  let dp = dict.get(b"DecodeParms").ok()?;
  match dp {
    Object::Dictionary(d) => Some(d.clone()),
    Object::Reference(id) => match doc.get_object(*id) {
      Ok(Object::Dictionary(d)) => Some(d.clone()),
      _ => None,
    },
    Object::Array(arr) => {
      // Filter chain: DecodeParms is an array parallel to Filter array.
      // Use the first dictionary entry found.
      for item in arr {
        match item {
          Object::Dictionary(d) => return Some(d.clone()),
          Object::Reference(id) => {
            if let Ok(Object::Dictionary(d)) = doc.get_object(*id) {
              return Some(d.clone());
            }
          }
          _ => {}
        }
      }
      None
    }
    _ => None,
  }
}

/// Apply PNG predictor unfiltering to raw decompressed data.
/// Each row has a 1-byte filter type prefix followed by `row_bytes` of filtered data.
/// `bytes_per_pixel` is the number of bytes per pixel (channels * ceil(bpc/8)).
fn apply_png_predictor(data: &[u8], bytes_per_pixel: usize, row_bytes: usize) -> Option<Vec<u8>> {
  let src_row_len = row_bytes + 1; // +1 for filter type byte
  if !data.len().is_multiple_of(src_row_len) {
    return None;
  }
  let num_rows = data.len() / src_row_len;
  let mut output = Vec::with_capacity(num_rows * row_bytes);
  let mut prev_row = vec![0u8; row_bytes];

  for row_idx in 0..num_rows {
    let row_start = row_idx * src_row_len;
    let filter_byte = data[row_start];
    let mut current_row = data[row_start + 1..row_start + src_row_len].to_vec();

    match filter_byte {
      0 => { /* None */ }
      1 => {
        // Sub
        for i in bytes_per_pixel..row_bytes {
          current_row[i] = current_row[i].wrapping_add(current_row[i - bytes_per_pixel]);
        }
      }
      2 => {
        // Up
        for i in 0..row_bytes {
          current_row[i] = current_row[i].wrapping_add(prev_row[i]);
        }
      }
      3 => {
        // Average
        for i in 0..bytes_per_pixel {
          current_row[i] = current_row[i].wrapping_add(prev_row[i] / 2);
        }
        for i in bytes_per_pixel..row_bytes {
          current_row[i] = current_row[i].wrapping_add(
            ((current_row[i - bytes_per_pixel] as u16 + prev_row[i] as u16) / 2) as u8,
          );
        }
      }
      4 => {
        // Paeth
        for i in 0..bytes_per_pixel {
          current_row[i] = current_row[i].wrapping_add(paeth_predictor(0, prev_row[i], 0));
        }
        for i in bytes_per_pixel..row_bytes {
          current_row[i] = current_row[i].wrapping_add(paeth_predictor(
            current_row[i - bytes_per_pixel],
            prev_row[i],
            prev_row[i - bytes_per_pixel],
          ));
        }
      }
      _ => return None, // Unknown filter type
    }

    output.extend_from_slice(&current_row);
    prev_row = current_row;
  }

  Some(output)
}

fn paeth_predictor(a: u8, b: u8, c: u8) -> u8 {
  let pa = (b as i16 - c as i16).abs();
  let pb = (a as i16 - c as i16).abs();
  let pc = (a as i16 + b as i16 - 2 * c as i16).abs();
  if pa <= pb && pa <= pc {
    a
  } else if pb <= pc {
    b
  } else {
    c
  }
}

/// Decompress a stream's content with correct predictor handling.
///
/// lopdf's built-in `decompress()` attempts PNG predictor unfiltering internally
/// but produces corrupted output for some streams (e.g. xdvipdfmx/pandoc images).
/// We bypass it entirely: raw zlib inflate via `flate2`, then apply our own
/// predictor reversal.
fn decompress_stream_content(
  doc: &Document,
  stream: &lopdf::Stream,
  width: u32,
  height: u32,
  channels: u32,
  bpc: u32,
) -> Vec<u8> {
  let bytes_per_sample = if bpc > 8 { 2u32 } else { 1u32 };
  let row_bytes = (width * channels * bpc / 8) as usize;
  let expected = (width * height * channels * bytes_per_sample) as usize;
  let predicted_len = height as usize * (row_bytes + 1);

  // Check if the stream uses FlateDecode
  let uses_flate = match stream.dict.get(b"Filter") {
    Ok(Object::Name(n)) => n == b"FlateDecode",
    Ok(Object::Array(arr)) => arr
      .iter()
      .any(|o| matches!(o, Object::Name(n) if n == b"FlateDecode")),
    _ => false,
  };

  // Step 1: Raw inflate — bypass lopdf's decompress to avoid its buggy predictor handling
  let content = if uses_flate {
    raw_inflate(&stream.content).unwrap_or_else(|| {
      // Fallback: let lopdf try (handles edge cases like chained filters)
      let mut s = stream.clone();
      if s.decompress().is_ok() {
        s.content
      } else {
        stream.content.clone()
      }
    })
  } else {
    stream.content.clone()
  };

  // Step 2: Apply predictor reversal if DecodeParms specifies one
  if let Some(dp) = resolve_decode_parms(doc, &stream.dict) {
    let predictor = get_dict_int(&dp, b"Predictor").unwrap_or(1);

    // TIFF Predictor 2: horizontal differencing (same size as raw pixels)
    if predictor == 2 && content.len() == expected {
      let bpp = (channels * bpc / 8).max(1) as usize;
      let mut data = content;
      apply_tiff_predictor2(&mut data, bpp, row_bytes);
      return data;
    }

    // PNG Predictors 10-15: each row has a leading filter type byte
    if (10..=15).contains(&predictor) && content.len() == predicted_len {
      let bpp = (channels * bpc / 8).max(1) as usize;
      if let Some(unfiltered) = apply_png_predictor(&content, bpp, row_bytes) {
        return unfiltered;
      }
    }
  }

  content
}

/// Raw zlib inflate without any predictor handling.
fn raw_inflate(data: &[u8]) -> Option<Vec<u8>> {
  use std::io::Read;
  // Try zlib wrapper first (most common in PDF)
  let mut output = Vec::new();
  if flate2::read::ZlibDecoder::new(data)
    .read_to_end(&mut output)
    .is_ok()
  {
    return Some(output);
  }
  // Fallback to raw deflate (no zlib header)
  output.clear();
  if flate2::read::DeflateDecoder::new(data)
    .read_to_end(&mut output)
    .is_ok()
  {
    return Some(output);
  }
  None
}

/// Reverse TIFF Predictor 2 (horizontal differencing) in-place.
/// Each byte after the first `bpp` bytes in each row is a delta from the previous byte.
fn apply_tiff_predictor2(data: &mut [u8], bpp: usize, row_bytes: usize) {
  if row_bytes == 0 {
    return;
  }
  let num_rows = data.len() / row_bytes;
  for row in 0..num_rows {
    let start = row * row_bytes;
    for i in (start + bpp)..(start + row_bytes) {
      data[i] = data[i].wrapping_add(data[i - bpp]);
    }
  }
}

/// Retrieve and decompress the SMask (soft mask / alpha channel) image data if present
fn get_smask_data(doc: &Document, dict: &lopdf::Dictionary) -> Option<Vec<u8>> {
  let smask_ref = dict.get(b"SMask").ok()?;
  let smask_id = match smask_ref {
    Object::Reference(id) => *id,
    _ => return None,
  };

  let smask_stream = match doc.get_object(smask_id) {
    Ok(Object::Stream(s)) => s,
    _ => return None,
  };

  // Verify it's an Image subtype
  let subtype = smask_stream.dict.get(b"Subtype").ok().and_then(|v| {
    if let Object::Name(n) = v {
      Some(n.as_slice())
    } else {
      None
    }
  });
  if subtype != Some(b"Image") {
    return None;
  }

  // SMask is always DeviceGray with 1 channel
  let smask_width = get_dict_int(&smask_stream.dict, b"Width").unwrap_or(0) as u32;
  let smask_height = get_dict_int(&smask_stream.dict, b"Height").unwrap_or(0) as u32;
  let smask_bpc = get_dict_int(&smask_stream.dict, b"BitsPerComponent").unwrap_or(8) as u32;
  Some(decompress_stream_content(
    doc,
    smask_stream,
    smask_width,
    smask_height,
    1,
    smask_bpc,
  ))
}

fn resolve_to_dict(doc: &Document, obj: &Object) -> Option<lopdf::Dictionary> {
  match obj {
    Object::Dictionary(d) => Some(d.clone()),
    Object::Reference(id) => match doc.get_object(*id).ok()? {
      Object::Dictionary(d) => Some(d.clone()),
      _ => None,
    },
    _ => None,
  }
}

fn get_dict_int(dict: &lopdf::Dictionary, key: &[u8]) -> Option<i64> {
  match dict.get(key).ok()? {
    Object::Integer(i) => Some(*i),
    _ => None,
  }
}

fn resolve_color_space(dict: &lopdf::Dictionary, doc: &Document) -> String {
  let cs = match dict.get(b"ColorSpace") {
    Ok(obj) => obj,
    Err(_) => return "DeviceRGB".to_string(),
  };

  match cs {
    Object::Name(name) => String::from_utf8_lossy(name).to_string(),
    Object::Reference(id) => match doc.get_object(*id) {
      Ok(Object::Name(name)) => String::from_utf8_lossy(name).to_string(),
      // ICCBased is typically [/ICCBased <stream ref>]
      Ok(Object::Array(arr)) => parse_color_space_array(arr, doc),
      _ => "DeviceRGB".to_string(),
    },
    Object::Array(arr) => parse_color_space_array(arr, doc),
    _ => "DeviceRGB".to_string(),
  }
}

fn parse_color_space_array(arr: &[Object], doc: &Document) -> String {
  if arr.is_empty() {
    return "DeviceRGB".to_string();
  }

  let cs_name = match &arr[0] {
    Object::Name(n) => String::from_utf8_lossy(n).to_string(),
    _ => return "DeviceRGB".to_string(),
  };

  if cs_name == "ICCBased" && arr.len() > 1 {
    // Get /N from the ICCBased stream to determine channel count
    let stream_id = match &arr[1] {
      Object::Reference(id) => *id,
      _ => return "ICCBased".to_string(),
    };

    if let Ok(Object::Stream(s)) = doc.get_object(stream_id) {
      let n = get_dict_int(&s.dict, b"N").unwrap_or(3);
      return format!("ICCBased{n}");
    }
  }

  cs_name
}

fn resolve_filter(dict: &lopdf::Dictionary) -> String {
  match dict.get(b"Filter") {
    Ok(Object::Name(name)) => String::from_utf8_lossy(name).to_string(),
    Ok(Object::Array(arr)) => {
      // Filter chain — return the last (innermost) filter for image type detection
      if let Some(Object::Name(name)) = arr.last() {
        String::from_utf8_lossy(name).to_string()
      } else {
        "None".to_string()
      }
    }
    _ => "None".to_string(),
  }
}

/// Decode an XObject stream into a DynamicImage (shared by PNG export and OCR).
fn decode_xobject_to_dynamic_image(
  content: &[u8],
  width: u32,
  height: u32,
  bpc: u32,
  color_space: &str,
  filter: &str,
  smask: Option<&[u8]>,
) -> Option<DynamicImage> {
  let dynamic_img = if filter == "DCTDecode" {
    image::load_from_memory_with_format(content, ImageFormat::Jpeg).ok()?
  } else if filter == "JPXDecode" {
    decode_jpx(content)?
  } else {
    decode_raw_pixels(content, width, height, bpc, color_space)?
  };

  Some(if let Some(mask_data) = smask {
    apply_smask(dynamic_img, mask_data, width, height)
  } else {
    dynamic_img
  })
}

fn encode_to_png(
  content: &[u8],
  width: u32,
  height: u32,
  bpc: u32,
  color_space: &str,
  filter: &str,
  smask: Option<&[u8]>,
) -> Option<Vec<u8>> {
  let final_img =
    decode_xobject_to_dynamic_image(content, width, height, bpc, color_space, filter, smask)?;
  let mut png_buf = Cursor::new(Vec::new());
  final_img.write_to(&mut png_buf, ImageFormat::Png).ok()?;
  Some(png_buf.into_inner())
}

/// Decode a JPEG 2000 (JPXDecode) stream using hayro-jpeg2000 (pure Rust)
fn decode_jpx(content: &[u8]) -> Option<DynamicImage> {
  let jp2_img =
    hayro_jpeg2000::Image::new(content, &hayro_jpeg2000::DecodeSettings::default()).ok()?;
  DynamicImage::from_decoder(jp2_img).ok()
}

/// Decode raw pixel data (FlateDecode / uncompressed) into a DynamicImage
fn decode_raw_pixels(
  content: &[u8],
  width: u32,
  height: u32,
  bpc: u32,
  color_space: &str,
) -> Option<DynamicImage> {
  let channels: u32 = match color_space {
    "DeviceRGB" | "ICCBased3" | "CalRGB" => 3,
    "DeviceGray" | "ICCBased1" | "CalGray" => 1,
    "DeviceCMYK" | "ICCBased4" => 4,
    _ => 3,
  };
  let bytes_per_sample = if bpc > 8 { 2u32 } else { 1u32 };
  let expected = (width * height * channels * bytes_per_sample) as usize;

  // Validate buffer size before constructing image
  if content.len() < expected {
    return None;
  }
  // Use exactly the expected number of bytes
  let pixel_data = &content[..expected];

  // Downscale 16-bit to 8-bit if needed
  let pixel_data_8bit: Vec<u8> = if bytes_per_sample == 2 {
    pixel_data
      .chunks_exact(2)
      .map(|pair| (u16::from_be_bytes([pair[0], pair[1]]) >> 8) as u8)
      .collect()
  } else {
    pixel_data.to_vec()
  };

  match color_space {
    "DeviceRGB" | "ICCBased3" | "CalRGB" => {
      let img: ImageBuffer<image::Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width, height, pixel_data_8bit)?;
      Some(DynamicImage::ImageRgb8(img))
    }
    "DeviceGray" | "ICCBased1" | "CalGray" => {
      let img: ImageBuffer<image::Luma<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width, height, pixel_data_8bit)?;
      Some(DynamicImage::ImageLuma8(img))
    }
    "DeviceCMYK" | "ICCBased4" => {
      let rgb_bytes = cmyk_to_rgb(&pixel_data_8bit);
      let img: ImageBuffer<image::Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width, height, rgb_bytes)?;
      Some(DynamicImage::ImageRgb8(img))
    }
    _ => {
      let expected_rgb = (width * height * 3) as usize;
      if pixel_data_8bit.len() == expected_rgb {
        let img: ImageBuffer<image::Rgb<u8>, Vec<u8>> =
          ImageBuffer::from_raw(width, height, pixel_data_8bit)?;
        Some(DynamicImage::ImageRgb8(img))
      } else {
        None
      }
    }
  }
}

/// Combine a base RGB image with a grayscale SMask to produce an RGBA PNG
fn apply_smask(base: DynamicImage, mask_data: &[u8], width: u32, height: u32) -> DynamicImage {
  let rgb = base.to_rgb8();
  let expected_mask_len = (width * height) as usize;

  if mask_data.len() < expected_mask_len {
    return DynamicImage::ImageRgb8(rgb);
  }

  let mut rgba_pixels = Vec::with_capacity((width * height * 4) as usize);
  for (rgb_pixel, &alpha) in rgb.pixels().zip(mask_data.iter()) {
    rgba_pixels.push(rgb_pixel[0]);
    rgba_pixels.push(rgb_pixel[1]);
    rgba_pixels.push(rgb_pixel[2]);
    rgba_pixels.push(alpha);
  }

  match ImageBuffer::from_raw(width, height, rgba_pixels) {
    Some(img) => DynamicImage::ImageRgba8(img),
    None => DynamicImage::ImageRgb8(rgb),
  }
}

fn cmyk_to_rgb(cmyk: &[u8]) -> Vec<u8> {
  let pixel_count = cmyk.len() / 4;
  let mut rgb = Vec::with_capacity(pixel_count * 3);

  for i in 0..pixel_count {
    let c = cmyk[i * 4] as f32 / 255.0;
    let m = cmyk[i * 4 + 1] as f32 / 255.0;
    let y = cmyk[i * 4 + 2] as f32 / 255.0;
    let k = cmyk[i * 4 + 3] as f32 / 255.0;

    let r = 255.0 * (1.0 - c) * (1.0 - k);
    let g = 255.0 * (1.0 - m) * (1.0 - k);
    let b = 255.0 * (1.0 - y) * (1.0 - k);

    rgb.push(r as u8);
    rgb.push(g as u8);
    rgb.push(b as u8);
  }

  rgb
}

// ── Standalone async functions (libuv thread pool via AsyncTask) ─

pub struct ExtractTextTask(Vec<u8>);

#[napi]
impl Task for ExtractTextTask {
  type Output = Vec<PageText>;
  type JsValue = Vec<PageText>;

  fn compute(&mut self) -> Result<Self::Output> {
    let doc = load_doc(&self.0)?;
    extract_text(&doc)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

#[napi]
pub fn extract_text_per_page_async(buffer: Buffer) -> AsyncTask<ExtractTextTask> {
  AsyncTask::new(ExtractTextTask(buffer.to_vec()))
}

pub struct ExtractImagesTask(Vec<u8>);

#[napi]
impl Task for ExtractImagesTask {
  type Output = Vec<RawPageImage>;
  type JsValue = Vec<PageImage>;

  fn compute(&mut self) -> Result<Self::Output> {
    let doc = load_doc(&self.0)?;
    Ok(extract_images_raw(&doc))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into_iter().map(PageImage::from).collect())
  }
}

#[napi]
pub fn extract_images_per_page_async(buffer: Buffer) -> AsyncTask<ExtractImagesTask> {
  AsyncTask::new(ExtractImagesTask(buffer.to_vec()))
}

pub struct PdfMetaTask(Vec<u8>);

#[napi]
impl Task for PdfMetaTask {
  type Output = PdfMeta;
  type JsValue = PdfMeta;

  fn compute(&mut self) -> Result<Self::Output> {
    let doc = load_doc(&self.0)?;
    Ok(extract_metadata(&doc))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

pub struct ExtractAnnotationsTask(Vec<u8>);

#[napi]
impl Task for ExtractAnnotationsTask {
  type Output = Vec<PageAnnotation>;
  type JsValue = Vec<PageAnnotation>;

  fn compute(&mut self) -> Result<Self::Output> {
    let doc = load_doc(&self.0)?;
    Ok(extract_annotations(&doc))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

#[napi]
pub fn extract_annotations_per_page_async(buffer: Buffer) -> AsyncTask<ExtractAnnotationsTask> {
  AsyncTask::new(ExtractAnnotationsTask(buffer.to_vec()))
}

#[napi]
pub fn pdf_metadata_async(buffer: Buffer) -> AsyncTask<PdfMetaTask> {
  AsyncTask::new(PdfMetaTask(buffer.to_vec()))
}

pub struct PdfDocumentTask(Vec<u8>);

#[napi]
impl Task for PdfDocumentTask {
  type Output = RawPdfDocument;
  type JsValue = PdfDocument;

  fn compute(&mut self) -> Result<Self::Output> {
    let doc = load_doc(&self.0)?;
    extract_all(&doc)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(PdfDocument::from(output))
  }
}

#[napi]
pub fn pdf_document_async(buffer: Buffer) -> AsyncTask<PdfDocumentTask> {
  AsyncTask::new(PdfDocumentTask(buffer.to_vec()))
}

pub struct ExtractStructuredTextTask(Vec<u8>);

#[napi]
impl Task for ExtractStructuredTextTask {
  type Output = Vec<StructuredPageText>;
  type JsValue = Vec<StructuredPageText>;

  fn compute(&mut self) -> Result<Self::Output> {
    let doc = load_doc(&self.0)?;
    extract_structured_text(&doc)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

#[napi]
pub fn extract_structured_text_per_page_async(
  buffer: Buffer,
) -> AsyncTask<ExtractStructuredTextTask> {
  AsyncTask::new(ExtractStructuredTextTask(buffer.to_vec()))
}

#[cfg(feature = "ocr")]
pub struct ExtractTextOcrTask {
  data: Vec<u8>,
  lang: String,
  min_len: u32,
  max_threads: u32,
}

#[cfg(feature = "ocr")]
#[napi]
impl Task for ExtractTextOcrTask {
  type Output = Vec<OcrPageText>;
  type JsValue = Vec<OcrPageText>;

  fn compute(&mut self) -> Result<Self::Output> {
    let doc = load_doc(&self.data)?;
    extract_text_with_ocr(&doc, &self.lang, self.min_len, self.max_threads)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

#[cfg(feature = "ocr")]
#[napi]
pub fn extract_text_with_ocr_per_page_async(
  buffer: Buffer,
  opts: Option<OcrOptions>,
) -> AsyncTask<ExtractTextOcrTask> {
  let lang = opts
    .as_ref()
    .and_then(|o| o.lang.clone())
    .unwrap_or_else(|| "eng".to_string());
  let min_len = opts.as_ref().and_then(|o| o.min_text_length).unwrap_or(1);
  let max_threads = normalize_max_threads(opts.as_ref().and_then(|o| o.max_threads));
  AsyncTask::new(ExtractTextOcrTask {
    data: buffer.to_vec(),
    lang,
    min_len,
    max_threads,
  })
}

#[cfg(feature = "ocr")]
pub struct PdfDocumentOcrTask {
  data: Vec<u8>,
  lang: String,
  min_len: u32,
  max_threads: u32,
}

#[cfg(feature = "ocr")]
#[napi]
impl Task for PdfDocumentOcrTask {
  type Output = RawPdfDocumentOcr;
  type JsValue = PdfDocumentOcr;

  fn compute(&mut self) -> Result<Self::Output> {
    let doc = load_doc(&self.data)?;
    extract_all_with_ocr(&doc, &self.lang, self.min_len, self.max_threads)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(PdfDocumentOcr::from(output))
  }
}

#[cfg(feature = "ocr")]
#[napi]
pub fn pdf_document_ocr_async(
  buffer: Buffer,
  opts: Option<OcrOptions>,
) -> AsyncTask<PdfDocumentOcrTask> {
  let lang = opts
    .as_ref()
    .and_then(|o| o.lang.clone())
    .unwrap_or_else(|| "eng".to_string());
  let min_len = opts.as_ref().and_then(|o| o.min_text_length).unwrap_or(1);
  let max_threads = normalize_max_threads(opts.as_ref().and_then(|o| o.max_threads));
  AsyncTask::new(PdfDocumentOcrTask {
    data: buffer.to_vec(),
    lang,
    min_len,
    max_threads,
  })
}

// ── Step 3: Class-based API with Arc<Document> ──────────────────

/// Shared-document task types for class async methods.
/// These use Arc<Document> instead of raw bytes, avoiding re-parsing.
pub struct SharedExtractTextTask(Arc<Document>);

#[napi]
impl Task for SharedExtractTextTask {
  type Output = Vec<PageText>;
  type JsValue = Vec<PageText>;

  fn compute(&mut self) -> Result<Self::Output> {
    extract_text(&self.0)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

pub struct SharedExtractImagesTask(Arc<Document>);

#[napi]
impl Task for SharedExtractImagesTask {
  type Output = Vec<RawPageImage>;
  type JsValue = Vec<PageImage>;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(extract_images_raw(&self.0))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into_iter().map(PageImage::from).collect())
  }
}

pub struct SharedExtractAnnotationsTask(Arc<Document>);

#[napi]
impl Task for SharedExtractAnnotationsTask {
  type Output = Vec<PageAnnotation>;
  type JsValue = Vec<PageAnnotation>;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(extract_annotations(&self.0))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

pub struct SharedPdfMetaTask(Arc<Document>);

#[napi]
impl Task for SharedPdfMetaTask {
  type Output = PdfMeta;
  type JsValue = PdfMeta;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(extract_metadata(&self.0))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

pub struct SharedPdfDocumentTask(Arc<Document>);

#[napi]
impl Task for SharedPdfDocumentTask {
  type Output = RawPdfDocument;
  type JsValue = PdfDocument;

  fn compute(&mut self) -> Result<Self::Output> {
    extract_all(&self.0)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(PdfDocument::from(output))
  }
}

pub struct SharedStructuredTextTask(Arc<Document>);

#[napi]
impl Task for SharedStructuredTextTask {
  type Output = Vec<StructuredPageText>;
  type JsValue = Vec<StructuredPageText>;

  fn compute(&mut self) -> Result<Self::Output> {
    extract_structured_text(&self.0)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

#[cfg(feature = "ocr")]
pub struct SharedExtractTextOcrTask {
  doc: Arc<Document>,
  lang: String,
  min_len: u32,
  max_threads: u32,
}

#[cfg(feature = "ocr")]
#[napi]
impl Task for SharedExtractTextOcrTask {
  type Output = Vec<OcrPageText>;
  type JsValue = Vec<OcrPageText>;

  fn compute(&mut self) -> Result<Self::Output> {
    extract_text_with_ocr(&self.doc, &self.lang, self.min_len, self.max_threads)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

#[cfg(feature = "ocr")]
pub struct SharedPdfDocumentOcrTask {
  doc: Arc<Document>,
  lang: String,
  min_len: u32,
  max_threads: u32,
}

#[cfg(feature = "ocr")]
#[napi]
impl Task for SharedPdfDocumentOcrTask {
  type Output = RawPdfDocumentOcr;
  type JsValue = PdfDocumentOcr;

  fn compute(&mut self) -> Result<Self::Output> {
    extract_all_with_ocr(&self.doc, &self.lang, self.min_len, self.max_threads)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(PdfDocumentOcr::from(output))
  }
}

#[napi]
pub struct PdfDown {
  doc: Arc<Document>,
}

#[napi]
impl PdfDown {
  #[napi(constructor)]
  pub fn new(buffer: Buffer) -> Result<Self> {
    let doc = Document::load_mem(buffer.as_ref())
      .map_err(|e| Error::from_reason(format!("Failed to load PDF: {e}")))?;
    Ok(PdfDown { doc: Arc::new(doc) })
  }

  /// Sync: extract text per page (reuses the already-parsed document)
  #[napi]
  pub fn text_per_page(&self) -> Result<Vec<PageText>> {
    extract_text(&self.doc)
  }

  /// Sync: extract images per page (reuses the already-parsed document)
  #[napi]
  pub fn images_per_page(&self) -> Result<Vec<PageImage>> {
    Ok(
      extract_images_raw(&self.doc)
        .into_iter()
        .map(PageImage::from)
        .collect(),
    )
  }

  /// Sync: extract annotations per page (reuses the already-parsed document)
  #[napi]
  pub fn annotations_per_page(&self) -> Vec<PageAnnotation> {
    extract_annotations(&self.doc)
  }

  /// Sync: get PDF metadata (reuses the already-parsed document)
  #[napi]
  pub fn metadata(&self) -> PdfMeta {
    extract_metadata(&self.doc)
  }

  /// Async: extract text per page on the libuv thread pool (shares parsed document via Arc)
  #[napi]
  pub fn text_per_page_async(&self) -> AsyncTask<SharedExtractTextTask> {
    AsyncTask::new(SharedExtractTextTask(Arc::clone(&self.doc)))
  }

  /// Async: extract images per page on the libuv thread pool (shares parsed document via Arc)
  #[napi]
  pub fn images_per_page_async(&self) -> AsyncTask<SharedExtractImagesTask> {
    AsyncTask::new(SharedExtractImagesTask(Arc::clone(&self.doc)))
  }

  /// Async: extract annotations per page on the libuv thread pool (shares parsed document via Arc)
  #[napi]
  pub fn annotations_per_page_async(&self) -> AsyncTask<SharedExtractAnnotationsTask> {
    AsyncTask::new(SharedExtractAnnotationsTask(Arc::clone(&self.doc)))
  }

  /// Async: get PDF metadata on the libuv thread pool (shares parsed document via Arc)
  #[napi]
  pub fn metadata_async(&self) -> AsyncTask<SharedPdfMetaTask> {
    AsyncTask::new(SharedPdfMetaTask(Arc::clone(&self.doc)))
  }

  /// Sync: extract everything from the PDF in one call (reuses the already-parsed document)
  #[napi]
  pub fn document(&self) -> Result<PdfDocument> {
    Ok(PdfDocument::from(extract_all(&self.doc)?))
  }

  /// Async: extract everything from the PDF on the libuv thread pool (shares parsed document via Arc)
  #[napi]
  pub fn document_async(&self) -> AsyncTask<SharedPdfDocumentTask> {
    AsyncTask::new(SharedPdfDocumentTask(Arc::clone(&self.doc)))
  }

  /// Sync: extract structured text with header/footer detection
  #[napi]
  pub fn structured_text(&self) -> Result<Vec<StructuredPageText>> {
    extract_structured_text(&self.doc)
  }

  /// Async: extract structured text with header/footer detection
  #[napi]
  pub fn structured_text_async(&self) -> AsyncTask<SharedStructuredTextTask> {
    AsyncTask::new(SharedStructuredTextTask(Arc::clone(&self.doc)))
  }
}

#[cfg(feature = "ocr")]
#[napi]
impl PdfDown {
  /// Sync: extract text with OCR fallback for image-only pages
  #[napi]
  pub fn text_with_ocr_per_page(&self, opts: Option<OcrOptions>) -> Result<Vec<OcrPageText>> {
    let lang = opts
      .as_ref()
      .and_then(|o| o.lang.as_deref())
      .unwrap_or("eng");
    let min_len = opts.as_ref().and_then(|o| o.min_text_length).unwrap_or(1);
    let max_threads = normalize_max_threads(opts.as_ref().and_then(|o| o.max_threads));
    extract_text_with_ocr(&self.doc, lang, min_len, max_threads)
  }

  /// Async: extract text with OCR fallback for image-only pages
  #[napi]
  pub fn text_with_ocr_per_page_async(
    &self,
    opts: Option<OcrOptions>,
  ) -> AsyncTask<SharedExtractTextOcrTask> {
    let lang = opts
      .as_ref()
      .and_then(|o| o.lang.clone())
      .unwrap_or_else(|| "eng".to_string());
    let min_len = opts.as_ref().and_then(|o| o.min_text_length).unwrap_or(1);
    let max_threads = normalize_max_threads(opts.as_ref().and_then(|o| o.max_threads));
    AsyncTask::new(SharedExtractTextOcrTask {
      doc: Arc::clone(&self.doc),
      lang,
      min_len,
      max_threads,
    })
  }

  /// Sync: extract everything from the PDF with OCR text fallback
  #[napi]
  pub fn document_ocr(&self, opts: Option<OcrOptions>) -> Result<PdfDocumentOcr> {
    let lang = opts
      .as_ref()
      .and_then(|o| o.lang.as_deref())
      .unwrap_or("eng");
    let min_len = opts.as_ref().and_then(|o| o.min_text_length).unwrap_or(1);
    let max_threads = normalize_max_threads(opts.as_ref().and_then(|o| o.max_threads));
    Ok(PdfDocumentOcr::from(extract_all_with_ocr(
      &self.doc,
      lang,
      min_len,
      max_threads,
    )?))
  }

  /// Async: extract everything from the PDF with OCR text fallback
  #[napi]
  pub fn document_ocr_async(
    &self,
    opts: Option<OcrOptions>,
  ) -> AsyncTask<SharedPdfDocumentOcrTask> {
    let lang = opts
      .as_ref()
      .and_then(|o| o.lang.clone())
      .unwrap_or_else(|| "eng".to_string());
    let min_len = opts.as_ref().and_then(|o| o.min_text_length).unwrap_or(1);
    let max_threads = normalize_max_threads(opts.as_ref().and_then(|o| o.max_threads));
    AsyncTask::new(SharedPdfDocumentOcrTask {
      doc: Arc::clone(&self.doc),
      lang,
      min_len,
      max_threads,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn strip_basic_footer_artifact() {
    let text = "Some content\n/\n38\nMore content";
    let result = strip_footer_artifacts(text, "38");
    assert_eq!(result, "Some content\nMore content");
  }

  #[test]
  fn strip_footer_artifact_with_whitespace() {
    let text = "Some content\n  /  \n  38  \nMore content";
    let result = strip_footer_artifacts(text, "38");
    assert_eq!(result, "Some content\nMore content");
  }

  #[test]
  fn no_match_passthrough() {
    let text = "Some content\nNo footer here\nMore content";
    let result = strip_footer_artifacts(text, "38");
    assert_eq!(result, text);
  }

  #[test]
  fn multiple_occurrences() {
    let text = "Page one\n/\n38\nPage two\n/\n38\nPage three";
    let result = strip_footer_artifacts(text, "38");
    assert_eq!(result, "Page one\nPage two\nPage three");
  }

  #[test]
  fn at_start_of_text() {
    let text = "/\n38\nContent after";
    let result = strip_footer_artifacts(text, "38");
    assert_eq!(result, "Content after");
  }

  #[test]
  fn at_end_of_text() {
    let text = "Content before\n/\n38";
    let result = strip_footer_artifacts(text, "38");
    assert_eq!(result, "Content before");
  }

  #[test]
  fn empty_input() {
    let result = strip_footer_artifacts("", "38");
    assert_eq!(result, "");
  }

  #[test]
  fn single_line_input() {
    let result = strip_footer_artifacts("just one line", "38");
    assert_eq!(result, "just one line");
  }

  #[test]
  fn consecutive_pairs() {
    // Two pairs back to back: `/\n38\n/\n38`
    let text = "start\n/\n38\n/\n38\nend";
    let result = strip_footer_artifacts(text, "38");
    assert_eq!(result, "start\nend");
  }

  #[test]
  fn slash_not_followed_by_count() {
    let text = "Some content\n/\n99\nMore content";
    let result = strip_footer_artifacts(text, "38");
    assert_eq!(result, text);
  }

  #[test]
  fn slash_with_extra_text_not_stripped() {
    let text = "Some content\n/ extra\n38\nMore content";
    let result = strip_footer_artifacts(text, "38");
    assert_eq!(result, text);
  }

  #[test]
  fn count_with_extra_text_not_stripped() {
    let text = "Some content\n/\n38 pages\nMore content";
    let result = strip_footer_artifacts(text, "38");
    assert_eq!(result, text);
  }

  // ── parse_page_box tests ──────────────────────────────────────

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
