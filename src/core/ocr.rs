use crate::core::images::{collect_page_decoded_images, collect_page_decoded_images_named};
use crate::core::layout::words::{Word, words_to_lines};
use crate::core::layout::{page_geometry, page_layout};
use crate::core::text::{detect_headers_footers, strip_footer_artifacts};
use crate::types::{OcrPageText, OcrStructuredPageText, PageText, TextSource};
use crate::types::{PageLayout, PlacedImage};
use image::DynamicImage;
use lopdf::{Document, ObjectId};
use napi::Result;
use rayon::prelude::*;
use std::sync::Arc;

#[cfg(all(feature = "ocr", feature = "render"))]
use crate::core::images::{
  page_has_form_xobjects, page_has_image_xobjects, page_has_vector_content,
};
#[cfg(all(feature = "ocr", feature = "render"))]
use crate::core::render;

pub(crate) fn normalize_max_threads(v: Option<u32>) -> u32 {
  let default = 4u32;
  let max = std::thread::available_parallelism()
    .map(|n| n.get() as u32)
    .unwrap_or(default);
  v.unwrap_or(default).clamp(1, max)
}

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

/// Auto-detect tessdata path, cached for the lifetime of the process.
fn get_tessdata_prefix() -> Option<&'static str> {
  use std::sync::OnceLock;
  static TESSDATA_PATH: OnceLock<Option<String>> = OnceLock::new();

  TESSDATA_PATH
    .get_or_init(|| {
      if let Ok(path) = std::env::var("TESSDATA_PREFIX") {
        return Some(path);
      }

      let output = std::process::Command::new("tesseract")
        .arg("--list-langs")
        .output()
        .ok()?;

      let stderr = String::from_utf8_lossy(&output.stderr);
      let text = if stderr.contains('"') {
        stderr
      } else {
        String::from_utf8_lossy(&output.stdout)
      };

      let start = text.find('"')?;
      let end = text[start + 1..].find('"')?;
      Some(text[start + 1..start + 1 + end].to_string())
    })
    .as_deref()
}

/// Mean Tesseract confidence (0..100) below which a page that did produce
/// text is OCR'd again rotated. Upright copier scans score 80-95; a landscape
/// page scanned in portrait scores 35-45 and comes back as punctuation soup.
const ROTATION_RETRY_CONFIDENCE: f32 = 55.0;

/// Rotations tried, in order, when the upright pass scores too low. Most
/// rotated scans are a landscape sheet fed in portrait, so the quarter turns
/// come first and the half turn is only tried when neither of them is good.
const ROTATIONS: [u32; 3] = [90, 270, 180];

fn rotate_image(img: &DynamicImage, degrees: u32) -> DynamicImage {
  match degrees {
    90 => img.rotate90(),
    180 => img.rotate180(),
    270 => img.rotate270(),
    _ => img.clone(),
  }
}

/// Plain-text OCR of one image plus Tesseract's mean confidence (0..100).
fn ocr_image_text(img: &DynamicImage, lang: &str) -> (String, f32) {
  let datapath = get_tessdata_prefix().unwrap_or("");
  let rgb = img.to_rgb8();
  let (w, h) = rgb.dimensions();
  let pixels = rgb.as_raw();

  let tess = tesseract_rs::TesseractAPI::new();
  if tess.init(datapath, lang).is_err()
    || tess
      .set_image(pixels, w as i32, h as i32, 3, (w * 3) as i32)
      .is_err()
  {
    return (String::new(), 0.0);
  }
  let text = tess
    .get_utf8_text()
    .map(|t| t.trim().to_string())
    .unwrap_or_default();
  let conf = tess.mean_text_conf().unwrap_or(0) as f32;
  (text, conf)
}

/// OCR a single DynamicImage and return extracted text, retrying rotated when
/// the upright pass produced low-confidence text.
fn ocr_dynamic_image(img: &DynamicImage, lang: &str) -> String {
  let (text, conf) = ocr_image_text(img, lang);
  if text.is_empty() || conf >= ROTATION_RETRY_CONFIDENCE {
    return text;
  }
  let mut best = (text, conf);
  for degrees in ROTATIONS {
    let candidate = ocr_image_text(&rotate_image(img, degrees), lang);
    if candidate.1 > best.1 {
      best = candidate;
    }
    if best.1 >= ROTATION_RETRY_CONFIDENCE {
      break;
    }
  }
  best.0
}

fn ocr_page_images(doc: &Document, page_id: ObjectId, lang: &str) -> String {
  let images: Vec<DynamicImage> = collect_page_decoded_images(doc, page_id);
  let mut texts = Vec::new();

  for dyn_img in &images {
    let text = ocr_dynamic_image(dyn_img, lang);
    if !text.is_empty() {
      texts.push(text);
    }
  }

  texts.join("\n")
}

// Re-export render-mode constants so existing `use crate::core::ocr::RENDER_MODE_*`
// paths keep compiling.
#[cfg(feature = "render")]
pub(crate) use crate::core::render::{RENDER_MODE_ALWAYS, RENDER_MODE_NEVER};

// When the render feature is off, define the constants locally (they're still
// accepted as parameters but silently ignored, so only AUTO is ever read).
#[cfg(not(feature = "render"))]
pub(crate) const RENDER_MODE_AUTO: u8 = 0;
#[cfg(not(feature = "render"))]
#[allow(dead_code)]
pub(crate) const RENDER_MODE_NEVER: u8 = 1;
#[cfg(not(feature = "render"))]
#[allow(dead_code)]
pub(crate) const RENDER_MODE_ALWAYS: u8 = 2;

/// Extract text with OCR fallback, with optional render tier.
///
/// `render_dpi` and `render_mode` are always accepted as params.
/// When the `render` feature is disabled they are silently ignored.
pub(crate) fn extract_text_with_ocr(
  doc: &Document,
  pdf_bytes: &[u8],
  lang: &str,
  min_len: u32,
  max_threads: u32,
  render_dpi: u32,
  render_mode: u8,
) -> Result<Vec<OcrPageText>> {
  let pages = doc.get_pages();
  let page_count_str = pages.len().to_string();
  let page_entries: Vec<(u32, ObjectId)> = pages.iter().map(|(&k, &v)| (k, v)).collect();

  let pool = get_ocr_pool(max_threads as usize);

  #[cfg(feature = "render")]
  let pdf_arc: Arc<Vec<u8>> = Arc::new(pdf_bytes.to_vec());
  #[cfg(not(feature = "render"))]
  let _ = (pdf_bytes, render_dpi, render_mode);

  let mut results: Vec<OcrPageText> = pool.install(|| {
    page_entries
      .par_iter()
      .map(|&(page_num, page_id)| {
        // Tier 1: Native text extraction
        let raw = doc.extract_text(&[page_num]).unwrap_or_default();
        let native = strip_footer_artifacts(&raw, &page_count_str);
        let non_ws: usize = native.chars().filter(|c| !c.is_whitespace()).count();
        if non_ws >= min_len as usize {
          #[cfg(feature = "render")]
          {
            // In Always mode, render every page regardless
            if render_mode == RENDER_MODE_ALWAYS
              && let Some(rendered_text) = try_render_ocr_page(&pdf_arc, page_num, render_dpi, lang)
              && !rendered_text.is_empty()
            {
              return OcrPageText {
                page: page_num,
                text: rendered_text,
                source: TextSource::Rendered,
              };
            }
          }
          return OcrPageText {
            page: page_num,
            text: native,
            source: TextSource::Native,
          };
        }

        // Tier 2: OCR extracted raster images
        let ocr_text = ocr_page_images(doc, page_id, lang);
        if !ocr_text.is_empty() {
          return OcrPageText {
            page: page_num,
            text: ocr_text,
            source: TextSource::Ocr,
          };
        }

        // Tier 3: Render page + OCR (render feature only)
        #[cfg(feature = "render")]
        {
          if render_mode != RENDER_MODE_NEVER {
            // Auto: rasterize when the page has content tier 2 could not read directly —
            // form XObjects, vector drawing, or image XObjects whose codec/filter chain we
            // could not decode (otherwise such a page is reported as empty OCR).
            let should_render = render_mode == RENDER_MODE_ALWAYS
              || page_has_form_xobjects(doc, page_id)
              || page_has_image_xobjects(doc, page_id)
              || page_has_vector_content(doc, page_id);

            if should_render
              && let Some(rendered_text) = try_render_ocr_page(&pdf_arc, page_num, render_dpi, lang)
              && !rendered_text.is_empty()
            {
              return OcrPageText {
                page: page_num,
                text: rendered_text,
                source: TextSource::Rendered,
              };
            }
          }
        }

        // Fallback: empty
        OcrPageText {
          page: page_num,
          text: String::new(),
          source: TextSource::Ocr,
        }
      })
      .collect()
  });
  results.sort_unstable_by_key(|r| r.page);
  Ok(results)
}

/// Render a page to an image and OCR the result.
#[cfg(feature = "render")]
fn try_render_ocr_page(pdf_bytes: &[u8], page_num: u32, dpi: u32, lang: &str) -> Option<String> {
  if !render::is_pdfium_available() {
    return None;
  }
  // page_num is 1-based, PDFium page index is 0-based
  let page_index = (page_num - 1) as u16;
  let img = render::render_page_to_image_from_bytes(pdf_bytes, page_index, dpi)?;
  let text = ocr_dynamic_image(&img, lang);
  Some(text)
}

pub(crate) fn detect_headers_footers_ocr(pages: &[OcrPageText]) -> Vec<OcrStructuredPageText> {
  let as_page_text: Vec<PageText> = pages
    .iter()
    .map(|p| PageText {
      page: p.page,
      text: p.text.clone(),
    })
    .collect();
  let structured = detect_headers_footers(&as_page_text);
  structured
    .into_iter()
    .zip(pages.iter())
    .map(|(s, ocr)| OcrStructuredPageText {
      page: s.page,
      header: s.header,
      body: s.body,
      footer: s.footer,
      source: ocr.source,
    })
    .collect()
}

// ── Positioned OCR (phase III) ──────────────────────────────────

/// One Tesseract word in image pixel space (origin top-left).
struct OcrWord {
  text: String,
  left: i32,
  top: i32,
  right: i32,
  bottom: i32,
  confidence: f32,
}

fn mean_confidence(words: &[OcrWord]) -> f32 {
  if words.is_empty() {
    0.0
  } else {
    words.iter().map(|w| w.confidence).sum::<f32>() / words.len() as f32
  }
}

/// Word boxes in the frame where the text reads upright: the image as given,
/// or rotated by the returned angle (clockwise degrees) when the upright pass
/// scored below [`ROTATION_RETRY_CONFIDENCE`] and a rotation scored higher.
/// Boxes are in pixels of the rotated image, whose size is also returned.
fn ocr_words_best_orientation(
  img: &DynamicImage,
  lang: &str,
  ppi: Option<i32>,
) -> (Vec<OcrWord>, u32, (u32, u32)) {
  let upright = ocr_dynamic_image_words(img, lang, ppi);
  let conf = mean_confidence(&upright);
  if upright.is_empty() || conf >= ROTATION_RETRY_CONFIDENCE {
    return (upright, 0, (img.width(), img.height()));
  }
  let mut best = (upright, 0, (img.width(), img.height()), conf);
  for degrees in ROTATIONS {
    let rotated = rotate_image(img, degrees);
    let words = ocr_dynamic_image_words(&rotated, lang, ppi);
    let conf = mean_confidence(&words);
    if conf > best.3 {
      best = (words, degrees, (rotated.width(), rotated.height()), conf);
    }
    if best.3 >= ROTATION_RETRY_CONFIDENCE {
      break;
    }
  }
  (best.0, best.1, best.2)
}

/// OCR an image and return every recognised word with its pixel box.
fn ocr_dynamic_image_words(img: &DynamicImage, lang: &str, ppi: Option<i32>) -> Vec<OcrWord> {
  use tesseract_rs::TessPageIteratorLevel;

  let datapath = get_tessdata_prefix().unwrap_or("");
  let rgb = img.to_rgb8();
  let (w, h) = rgb.dimensions();
  let pixels = rgb.as_raw();

  let tess = tesseract_rs::TesseractAPI::new();
  if tess.init(datapath, lang).is_err()
    || tess
      .set_image(pixels, w as i32, h as i32, 3, (w * 3) as i32)
      .is_err()
  {
    return Vec::new();
  }
  if let Some(ppi) = ppi {
    let _ = tess.set_source_resolution(ppi);
  }
  if tess.recognize().is_err() {
    return Vec::new();
  }
  let Ok(it) = tess.get_iterator() else {
    return Vec::new();
  };
  let mut out = Vec::new();
  loop {
    if let Ok((text, left, top, right, bottom, confidence)) = it.get_word_with_bounds()
      && !text.trim().is_empty()
    {
      out.push(OcrWord {
        text: text.trim().to_string(),
        left,
        top,
        right,
        bottom,
        confidence,
      });
    }
    match it.next(TessPageIteratorLevel::RIL_WORD) {
      Ok(true) => continue,
      _ => break,
    }
  }
  out
}

/// Map pixel boxes of an image of `img_w`×`img_h` pixels onto the user-space
/// rectangle `dest = [x0, y0, x1, y1]` the image covers (y up).
fn map_ocr_words(words: Vec<OcrWord>, img_w: u32, img_h: u32, dest: [f64; 4]) -> Vec<Word> {
  let (iw, ih) = (f64::from(img_w.max(1)), f64::from(img_h.max(1)));
  let (dw, dh) = (dest[2] - dest[0], dest[3] - dest[1]);
  words
    .into_iter()
    .map(|w| {
      let x0 = dest[0] + f64::from(w.left) / iw * dw;
      let x1 = dest[0] + f64::from(w.right) / iw * dw;
      let y1 = dest[1] + (1.0 - f64::from(w.top) / ih) * dh;
      let y0 = dest[1] + (1.0 - f64::from(w.bottom) / ih) * dh;
      Word {
        text: w.text,
        x0,
        y0,
        x1,
        y1,
        baseline: y0,
        font_size: None,
        font: None,
        confidence: Some((f64::from(w.confidence) / 100.0).clamp(0.0, 1.0)),
      }
    })
    .collect()
}

/// Tier 2: OCR the page's embedded raster images and place each word through
/// the image's `Do` placement from the native pass.
/// The rectangle an upright-frame image maps onto: the placement itself, or,
/// after a quarter turn, a rectangle of the placement's height × width anchored
/// at the same corner so the upright text keeps the placement's scale.
fn upright_dest(dest: [f64; 4], rotation: u32) -> [f64; 4] {
  if rotation == 90 || rotation == 270 {
    let (w, h) = (dest[2] - dest[0], dest[3] - dest[1]);
    [dest[0], dest[1], dest[0] + h, dest[1] + w]
  } else {
    dest
  }
}

fn ocr_page_image_words(
  doc: &Document,
  page_id: ObjectId,
  lang: &str,
  placements: &[PlacedImage],
  rotation: &mut u32,
  warnings: &mut Vec<String>,
) -> Vec<Word> {
  let mut out = Vec::new();
  for (name, img) in collect_page_decoded_images_named(doc, page_id) {
    let placed: Vec<&PlacedImage> = placements
      .iter()
      .filter(|p| p.xobject_name == name)
      .collect();
    let Some(first) = placed.first() else {
      warnings.push(format!(
        "image {name}: no placement found; its OCR words were skipped"
      ));
      continue;
    };
    if placed.len() > 1 {
      warnings.push(format!(
        "image {name}: painted {} times; OCR words use the first placement",
        placed.len()
      ));
    }
    let (words, degrees, (w, h)) = ocr_words_best_orientation(&img, lang, None);
    if degrees != 0 {
      warnings.push(format!(
        "image {name}: text reads upright after a {degrees}° turn; its words are in that frame"
      ));
      *rotation = degrees;
    }
    out.extend(map_ocr_words(
      words,
      w,
      h,
      upright_dest([first.x0, first.y0, first.x1, first.y1], degrees),
    ));
  }
  out
}

/// Tier 3: rasterise the page and place each OCR word on the page box.
#[cfg(feature = "render")]
fn render_page_words(
  pdf_bytes: &[u8],
  page_num: u32,
  dpi: u32,
  lang: &str,
  geo: &crate::core::layout::PageGeometry,
  rotation: &mut u32,
  warnings: &mut Vec<String>,
) -> Option<Vec<Word>> {
  if !render::is_pdfium_available() {
    return None;
  }
  let page_index = (page_num - 1) as u16;
  let img = render::render_page_to_image_from_bytes(pdf_bytes, page_index, dpi)?;
  if geo.rotate != 0 {
    warnings.push(format!(
      "page {page_num}: /Rotate {}; rendered OCR coordinates are in displayed orientation",
      geo.rotate
    ));
  }
  let (words, degrees, (w, h)) = ocr_words_best_orientation(&img, lang, Some(dpi as i32));
  if degrees != 0 {
    warnings.push(format!(
      "page {page_num}: text reads upright after a {degrees}° turn; words, width and height are in that frame"
    ));
    *rotation = degrees;
  }
  Some(map_ocr_words(words, w, h, upright_dest(geo.bbox, degrees)))
}

fn finish_ocr_layout(
  mut layout: PageLayout,
  words: Vec<Word>,
  source: TextSource,
  rotation: u32,
) -> PageLayout {
  let (words, lines) = words_to_lines(words, &[]);
  layout.words = words;
  layout.lines = lines;
  layout.source = source;
  layout.content_rotation = rotation;
  if rotation == 90 || rotation == 270 {
    std::mem::swap(&mut layout.width, &mut layout.height);
  }
  layout
}

fn non_ws_chars(layout: &PageLayout) -> usize {
  layout
    .words
    .iter()
    .map(|w| w.text.chars().filter(|c| !c.is_whitespace()).count())
    .sum()
}

/// Positioned text with the same three-tier cascade as [`extract_text_with_ocr`]:
/// native glyph placement, then OCR of embedded images mapped through their
/// placements, then (with `render`) OCR of the rasterised page.
pub(crate) fn extract_layout_with_ocr(
  doc: &Document,
  pdf_bytes: &[u8],
  lang: &str,
  min_len: u32,
  max_threads: u32,
  render_dpi: u32,
  render_mode: u8,
) -> Result<Vec<PageLayout>> {
  let pages = doc.get_pages();
  let page_entries: Vec<(u32, ObjectId)> = pages.iter().map(|(&k, &v)| (k, v)).collect();
  let pool = get_ocr_pool(max_threads as usize);

  #[cfg(feature = "render")]
  let pdf_arc: Arc<Vec<u8>> = Arc::new(pdf_bytes.to_vec());
  #[cfg(not(feature = "render"))]
  let _ = (pdf_bytes, render_dpi, render_mode);

  let mut results: Vec<PageLayout> = pool.install(|| {
    page_entries
      .par_iter()
      .map(|&(page_num, page_id)| {
        // Tier 1: native glyph placement (also yields image placements for tier 2).
        let native = page_layout(doc, page_num, page_id);
        let mut warnings = Vec::new();
        let mut rotation = 0u32;

        #[cfg(feature = "render")]
        if render_mode == RENDER_MODE_ALWAYS {
          let geo = page_geometry(doc, page_id);
          if let Some(words) = render_page_words(
            &pdf_arc,
            page_num,
            render_dpi,
            lang,
            &geo,
            &mut rotation,
            &mut warnings,
          ) && !words.is_empty()
          {
            let mut out = finish_ocr_layout(native, words, TextSource::Rendered, rotation);
            out.warnings.extend(warnings);
            return out;
          }
        }

        if non_ws_chars(&native) >= min_len as usize {
          return native;
        }

        // Tier 2: OCR embedded raster images.
        let words = ocr_page_image_words(
          doc,
          page_id,
          lang,
          &native.images,
          &mut rotation,
          &mut warnings,
        );
        if !words.is_empty() {
          let mut out = finish_ocr_layout(native, words, TextSource::Ocr, rotation);
          out.warnings.extend(warnings);
          return out;
        }

        // Tier 3: render + OCR.
        #[cfg(feature = "render")]
        if render_mode != RENDER_MODE_NEVER {
          let should_render = page_has_form_xobjects(doc, page_id)
            || page_has_image_xobjects(doc, page_id)
            || page_has_vector_content(doc, page_id);
          if should_render {
            let geo = page_geometry(doc, page_id);
            if let Some(words) = render_page_words(
              &pdf_arc,
              page_num,
              render_dpi,
              lang,
              &geo,
              &mut rotation,
              &mut warnings,
            ) && !words.is_empty()
            {
              let mut out = finish_ocr_layout(native, words, TextSource::Rendered, rotation);
              out.warnings.extend(warnings);
              return out;
            }
          }
        }

        let mut out = finish_ocr_layout(native, Vec::new(), TextSource::Ocr, 0);
        out.warnings.extend(warnings);
        out
      })
      .collect()
  });
  results.sort_unstable_by_key(|r| r.page);
  Ok(results)
}

#[cfg(test)]
mod layout_tests {
  use super::*;

  #[test]
  fn quarter_turns_swap_the_destination_rectangle() {
    assert_eq!(
      upright_dest([10.0, 20.0, 110.0, 220.0], 90),
      [10.0, 20.0, 210.0, 120.0]
    );
    assert_eq!(
      upright_dest([10.0, 20.0, 110.0, 220.0], 270),
      [10.0, 20.0, 210.0, 120.0]
    );
    assert_eq!(
      upright_dest([10.0, 20.0, 110.0, 220.0], 180),
      [10.0, 20.0, 110.0, 220.0]
    );
    assert_eq!(
      upright_dest([10.0, 20.0, 110.0, 220.0], 0),
      [10.0, 20.0, 110.0, 220.0]
    );
  }

  #[test]
  fn pixel_boxes_map_onto_the_destination_rectangle() {
    let words = vec![OcrWord {
      text: "Hi".into(),
      left: 0,
      top: 0,
      right: 50,
      bottom: 10,
      confidence: 87.5,
    }];
    // 100×100 px image painted on [10, 20, 110, 220]: 1 px = 1 pt wide, 2 pt tall.
    let mapped = map_ocr_words(words, 100, 100, [10.0, 20.0, 110.0, 220.0]);
    let w = &mapped[0];
    assert_eq!((w.x0, w.x1), (10.0, 60.0));
    assert_eq!((w.y0, w.y1), (200.0, 220.0));
    assert_eq!(w.confidence, Some(0.875));
    assert_eq!(w.baseline, 200.0);
  }
}
