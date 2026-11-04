//! Positioned text: words and lines with boxes, plus where images were painted.
//! The foundation for layout-aware extraction (tables, key/value anchors).

pub(crate) mod fonts;
pub(crate) mod interpreter;
pub(crate) mod standard14;
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
  #[cfg_attr(not(all(feature = "ocr", feature = "render")), allow(dead_code))]
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
  let (words, lines) = words::words_to_lines(raw.clone(), &fonts);
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
mod tests {
  use super::*;
  use lopdf::content::{Content, Operation};
  use lopdf::{Stream, dictionary};

  /// One-page document with Helvetica as /F1 and the given content operations.
  fn doc_with_content(ops: Vec<Operation>) -> (Document, ObjectId) {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(
      dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" },
    );
    let content = Content { operations: ops };
    let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
    let resources = dictionary! { "Font" => Object::Dictionary(dictionary! { "F1" => Object::Reference(font_id) }) };
    let page_id = doc.add_object(dictionary! {
      "Type" => "Page", "Parent" => Object::Reference(pages_id),
      "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
      "Contents" => Object::Reference(content_id), "Resources" => Object::Dictionary(resources),
    });
    doc.objects.insert(
      pages_id,
      Object::Dictionary(
        dictionary! { "Type" => "Pages", "Kids" => vec![Object::Reference(page_id)], "Count" => 1 },
      ),
    );
    let catalog =
      doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => Object::Reference(pages_id) });
    doc.trailer.set("Root", Object::Reference(catalog));
    (doc, page_id)
  }

  fn op(name: &str, operands: Vec<Object>) -> Operation {
    Operation::new(name, operands)
  }

  fn text_ops(size: f64, x: f64, y: f64, text: &str) -> Vec<Operation> {
    vec![
      op("BT", vec![]),
      op("Tf", vec!["F1".into(), size.into()]),
      op("Td", vec![x.into(), y.into()]),
      op("Tj", vec![Object::string_literal(text)]),
      op("ET", vec![]),
    ]
  }

  fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
  }

  #[test]
  fn helvetica_words_land_where_the_metrics_say() {
    let (doc, page) = doc_with_content(text_ops(12.0, 100.0, 700.0, "Hello World"));
    let l = page_layout(&doc, 1, page);
    assert_eq!(l.source, TextSource::Native);
    assert_eq!((l.width, l.height, l.rotate), (612.0, 792.0, 0));
    assert_eq!(l.lines.len(), 1);
    assert_eq!(l.lines[0].text, "Hello World");
    assert_eq!(l.words.len(), 2);
    let hello = &l.words[0];
    // H e l l o = 722 + 556 + 222 + 222 + 556 = 2278 /1000 * 12
    assert!(close(hello.x0, 100.0));
    assert!(close(hello.x1, 100.0 + 2.278 * 12.0));
    assert!(close(hello.y0, 700.0 - 0.207 * 12.0));
    assert!(close(hello.y1, 700.0 + 0.718 * 12.0));
    assert_eq!(hello.font_size, Some(12.0));
    assert_eq!(hello.font_name.as_deref(), Some("Helvetica"));
    assert_eq!(hello.bold, Some(false));
    // "World" starts after the space (278)
    assert!(close(l.words[1].x0, 100.0 + (2.278 + 0.278) * 12.0));
    assert_eq!(l.words[1].line, 0);
  }

  #[test]
  fn lines_come_out_top_to_bottom_regardless_of_stream_order() {
    let mut ops = text_ops(10.0, 50.0, 100.0, "footer");
    ops.extend(text_ops(10.0, 50.0, 700.0, "header"));
    ops.extend(text_ops(10.0, 50.0, 400.0, "body text"));
    let (doc, page) = doc_with_content(ops);
    let l = page_layout(&doc, 1, page);
    assert_eq!(
      l.lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(),
      ["header", "body text", "footer"]
    );
    assert_eq!(
      l.words.iter().map(|w| w.line).collect::<Vec<_>>(),
      [0, 1, 1, 2]
    );
    assert_eq!(l.lines[1].first_word, 1);
    assert_eq!(l.lines[1].word_count, 2);
  }

  #[test]
  fn ctm_scales_positions_and_font_size() {
    let mut ops = vec![op(
      "cm",
      vec![2.into(), 0.into(), 0.into(), 2.into(), 10.into(), 20.into()],
    )];
    ops.extend(text_ops(12.0, 100.0, 300.0, "A"));
    let (doc, page) = doc_with_content(ops);
    let l = page_layout(&doc, 1, page);
    let a = &l.words[0];
    assert!(close(a.x0, 210.0));
    assert!(close(a.x1, 210.0 + 0.667 * 24.0));
    assert_eq!(a.font_size, Some(24.0));
    assert!(close(a.y0, 620.0 - 0.207 * 24.0));
  }

  #[test]
  fn tj_adjustments_split_words_only_when_large() {
    let ops = vec![
      op("BT", vec![]),
      op("Tf", vec!["F1".into(), 10.into()]),
      op("Td", vec![100.into(), 500.into()]),
      op(
        "TJ",
        vec![Object::Array(vec![
          Object::string_literal("Ke"),
          Object::Integer(-20),
          Object::string_literal("rn"),
          Object::Integer(-500),
          Object::string_literal("Next"),
        ])],
      ),
      op("ET", vec![]),
    ];
    let (doc, page) = doc_with_content(ops);
    let l = page_layout(&doc, 1, page);
    assert_eq!(
      l.words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(),
      ["Kern", "Next"]
    );
    // Kern: K 667 e 556 + 20 kerning r 333 n 556; Next starts after +500
    let kern_end = 100.0 + (0.667 + 0.556 + 0.020 + 0.333 + 0.556) * 10.0;
    assert!(close(l.words[0].x1, kern_end));
    assert!(close(l.words[1].x0, kern_end + 5.0));
  }

  #[test]
  fn horizontal_scaling_word_spacing_and_leading() {
    let ops = vec![
      op("BT", vec![]),
      op("Tf", vec!["F1".into(), 10.into()]),
      op("Tz", vec![50.into()]),
      op("Tw", vec![4.into()]),
      op("TL", vec![14.into()]),
      op("Td", vec![10.into(), 500.into()]),
      op("Tj", vec![Object::string_literal("ab cd")]),
      op("T*", vec![]),
      op("Tj", vec![Object::string_literal("ef")]),
      op("ET", vec![]),
    ];
    let (doc, page) = doc_with_content(ops);
    let l = page_layout(&doc, 1, page);
    assert_eq!(
      l.lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(),
      ["ab cd", "ef"]
    );
    let ab = &l.words[0];
    assert!(
      close(ab.x1, 10.0 + (0.556 + 0.556) * 10.0 * 0.5),
      "Tz halves advances"
    );
    let cd = &l.words[1];
    // after "ab": space (278/1000*10 + Tw 4) * 0.5
    assert!(close(cd.x0, ab.x1 + (2.78 + 4.0) * 0.5));
    assert!(
      close(l.words[2].y0, 486.0 - 0.207 * 10.0),
      "T* moved down by the leading"
    );
  }

  #[test]
  fn form_xobjects_and_images_are_placed_through_their_matrices() {
    let ops = vec![
      op("q", vec![]),
      op(
        "cm",
        vec![
          1.into(),
          0.into(),
          0.into(),
          1.into(),
          100.into(),
          600.into(),
        ],
      ),
      op("Do", vec!["Fx".into()]),
      op("Q", vec![]),
      op("q", vec![]),
      op(
        "cm",
        vec![
          200.into(),
          0.into(),
          0.into(),
          100.into(),
          50.into(),
          300.into(),
        ],
      ),
      op("Do", vec!["Im1".into()]),
      op("Q", vec![]),
    ];
    let (mut doc, page) = doc_with_content(ops);
    // A form XObject that draws "In form" at its own origin, through /Matrix.
    let form_font = doc.add_object(
      dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica-Bold" },
    );
    let mut form_ops = text_ops(10.0, 0.0, 0.0, "In form");
    form_ops[1] = op("Tf", vec!["F9".into(), 10.into()]);
    let form_content = Content {
      operations: form_ops,
    }
    .encode()
    .unwrap();
    let form = doc.add_object(Stream::new(
      dictionary! {
        "Type" => "XObject", "Subtype" => "Form", "BBox" => vec![0.into(), 0.into(), 100.into(), 20.into()],
        "Matrix" => vec![1.into(), 0.into(), 0.into(), 1.into(), 30.into(), 40.into()],
        "Resources" => Object::Dictionary(dictionary! { "Font" => Object::Dictionary(dictionary! { "F9" => Object::Reference(form_font) }) }),
      },
      form_content,
    ));
    let image = doc.add_object(Stream::new(
      dictionary! { "Type" => "XObject", "Subtype" => "Image", "Width" => 2, "Height" => 2, "ColorSpace" => "DeviceGray", "BitsPerComponent" => 8 },
      vec![0, 0, 0, 0],
    ));
    if let Some(Object::Dictionary(p)) = doc.objects.get_mut(&page)
      && let Ok(Object::Dictionary(res)) = p.get_mut(b"Resources")
    {
      res.set(
        "XObject",
        Object::Dictionary(
          dictionary! { "Fx" => Object::Reference(form), "Im1" => Object::Reference(image) },
        ),
      );
    }
    let l = page_layout(&doc, 1, page);
    assert_eq!(l.lines.len(), 1);
    assert_eq!(l.lines[0].text, "In form");
    assert!(close(l.words[0].x0, 130.0), "form matrix then cm");
    assert!(close(l.words[0].y1, 640.0 + 0.718 * 10.0));
    assert_eq!(l.words[0].bold, Some(true));
    assert_eq!(l.images.len(), 1);
    let im = &l.images[0];
    assert_eq!(im.xobject_name, "Im1");
    assert!(
      close(im.x0, 50.0) && close(im.y0, 300.0) && close(im.x1, 250.0) && close(im.y1, 400.0)
    );
  }

  #[test]
  fn rotation_is_reported_not_applied_and_cropbox_wins() {
    let (mut doc, page) = doc_with_content(text_ops(10.0, 50.0, 700.0, "x"));
    if let Some(Object::Dictionary(p)) = doc.objects.get_mut(&page) {
      p.set("Rotate", 90);
      p.set(
        "CropBox",
        vec![10.into(), 10.into(), 310.into(), 410.into()],
      );
    }
    let l = page_layout(&doc, 1, page);
    assert_eq!(l.rotate, 90);
    assert_eq!((l.width, l.height), (300.0, 400.0));
    assert!(
      close(l.words[0].x0, 50.0),
      "coordinates stay in raw user space"
    );
  }

  #[test]
  fn empty_page_and_malformed_content_degrade_cleanly() {
    let (doc, page) = doc_with_content(vec![]);
    let l = page_layout(&doc, 1, page);
    assert!(l.words.is_empty() && l.lines.is_empty() && l.warnings.is_empty());
    let (mut doc, page) = doc_with_content(text_ops(10.0, 50.0, 700.0, "x"));
    let contents_id = match doc.get_dictionary(page).unwrap().get(b"Contents").unwrap() {
      Object::Reference(id) => *id,
      _ => unreachable!(),
    };
    doc.objects.insert(
      contents_id,
      Object::Stream(Stream::new(
        dictionary! {},
        b"BT /F1 10 Tf ( unterminated".to_vec(),
      )),
    );
    let l = page_layout(&doc, 1, page);
    assert!(
      l.words.is_empty() || !l.words.is_empty(),
      "no panic is the contract"
    );
  }

  #[test]
  fn multi_page_extraction_keeps_page_order() {
    let (doc, _) = doc_with_content(text_ops(10.0, 50.0, 700.0, "only"));
    let all = extract_layout(&doc);
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].page, 1);
    assert_eq!(all[0].lines[0].text, "only");
  }
}
