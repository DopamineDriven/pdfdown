//! Positioned text: words and lines with boxes, plus where images were painted.
//! The foundation for layout-aware extraction (tables, key/value anchors).

pub(crate) mod fonts;
pub(crate) mod interpreter;
pub(crate) mod standard14;
pub(crate) mod tounicode;
pub(crate) mod words;

use crate::core::images::get_inherited_resources;
use crate::core::meta::get_inherited_page_box;
use crate::types::{PageLayout, TextSource};
use interpreter::{FontMeta, Interpreter, Matrix};
use lopdf::{Dictionary, Document, Object, ObjectId};
use rayon::prelude::*;
use words::Word;

/// Per-page bound on decoded content-stream bytes.
const MAX_CONTENT_BYTES: usize = 64 * 1024 * 1024;

fn inherited_int(doc: &Document, page_id: ObjectId, key: &[u8]) -> Option<i64> {
  let mut current = Some(page_id);
  let mut hops = 0;
  while let Some(id) = current {
    let dict = doc.get_dictionary(id).ok()?;
    if let Ok(v) = dict.get(key) {
      let v = match v {
        Object::Reference(r) => doc.get_object(*r).ok()?,
        other => other,
      };
      if let Object::Integer(i) = v {
        return Some(*i);
      }
    }
    current = dict.get(b"Parent").ok().and_then(|p| match p {
      Object::Reference(r) => Some(*r),
      _ => None,
    });
    hops += 1;
    if hops > 64 {
      break;
    }
  }
  None
}

pub(crate) struct PageGeometry {
  pub width: f64,
  pub height: f64,
  pub rotate: u32,
  /// `[x0, y0, x1, y1]` of the CropBox (fallback MediaBox) in user space.
  #[cfg_attr(not(feature = "ocr"), allow(dead_code))]
  pub bbox: [f64; 4],
}

pub(crate) fn page_geometry(doc: &Document, page_id: ObjectId) -> PageGeometry {
  let bbox = get_inherited_page_box(doc, page_id, b"CropBox")
    .or_else(|| get_inherited_page_box(doc, page_id, b"MediaBox"))
    .unwrap_or([0.0, 0.0, 612.0, 792.0]);
  let rotate = inherited_int(doc, page_id, b"Rotate")
    .map(|r| r.rem_euclid(360) as u32)
    .unwrap_or(0);
  PageGeometry {
    width: (bbox[2] - bbox[0]).abs(),
    height: (bbox[3] - bbox[1]).abs(),
    rotate,
    bbox: [
      bbox[0].min(bbox[2]),
      bbox[1].min(bbox[3]),
      bbox[0].max(bbox[2]),
      bbox[1].max(bbox[3]),
    ],
  }
}

/// Native layout of one page: glyph positions from the content stream.
/// A page's native layout together with the raw words and font metadata the lines were
/// clustered from, so an OCR tier can merge its own words in and re-cluster once.
#[cfg_attr(not(feature = "ocr"), allow(dead_code))]
pub(crate) struct NativeLayout {
  pub layout: PageLayout,
  pub words: Vec<Word>,
  pub fonts: Vec<FontMeta>,
}

pub(crate) fn native_layout(doc: &Document, page_num: u32, page_id: ObjectId) -> NativeLayout {
  let geo = page_geometry(doc, page_id);
  let mut layout = PageLayout {
    page: page_num,
    width: geo.width,
    height: geo.height,
    rotate: geo.rotate,
    content_rotation: 0,
    source: TextSource::Native,
    sheet: None,
    words: Vec::new(),
    lines: Vec::new(),
    images: Vec::new(),
    warnings: Vec::new(),
  };

  let content = match doc.get_page_content_with_limit(page_id, MAX_CONTENT_BYTES) {
    Ok(c) => c,
    Err(e) => {
      layout
        .warnings
        .push(format!("page {page_num}: content stream not decoded: {e}"));
      return NativeLayout {
        layout,
        words: Vec::new(),
        fonts: Vec::new(),
      };
    }
  };
  // Resources with page-tree inheritance; an empty dictionary when the page has none.
  let resources: Dictionary = get_inherited_resources(doc, page_id).unwrap_or_default();

  let mut interp = Interpreter::new(doc);
  interp.run(&content, &resources, Matrix::IDENTITY);
  let fonts = interp.font_metas();
  let raw = words::glyphs_to_words(&interp.glyphs);
  let (words, lines) = words::words_to_lines(raw.clone(), &fonts, &[]);
  layout.words = words;
  layout.lines = lines;
  layout.images = interp.images;
  layout.warnings.extend(interp.warnings);
  NativeLayout {
    layout,
    words: raw,
    fonts,
  }
}

pub(crate) fn page_layout(doc: &Document, page_num: u32, page_id: ObjectId) -> PageLayout {
  native_layout(doc, page_num, page_id).layout
}

/// Native layout of every page, in page order.
pub(crate) fn extract_layout(doc: &Document) -> Vec<PageLayout> {
  let pages = doc.get_pages();
  let entries: Vec<(u32, ObjectId)> = pages.iter().map(|(&k, &v)| (k, v)).collect();
  let mut out: Vec<PageLayout> = entries
    .par_iter()
    .map(|&(num, id)| page_layout(doc, num, id))
    .collect();
  out.sort_unstable_by_key(|p| p.page);
  out
}

#[cfg(test)]
mod tests;
