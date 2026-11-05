use crate::core::images::{
  collect_page_decoded_images, collect_page_decoded_images_named, page_path_op_count,
};
use crate::core::layout::interpreter::FontMeta;
use crate::core::layout::words::{Word, words_to_lines};
use crate::core::layout::{NativeLayout, PageGeometry, native_layout, page_geometry, page_layout};
use crate::core::text::{detect_headers_footers, strip_footer_artifacts};
use crate::types::{OcrPageText, OcrStructuredPageText, PageText, TextSource};
use crate::types::{PageLayout, PlacedImage};
use image::DynamicImage;
use lopdf::{Document, ObjectId};
use napi::Result;
use rayon::prelude::*;
use std::sync::Arc;

#[cfg(all(feature = "ocr", feature = "render"))]
use crate::core::images::{page_has_form_xobjects, page_has_image_xobjects};
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

/// Tesseract's orientation detector is trusted from this confidence; below it the blind retry
/// loop decides. Measured on copier scans: upright pages 3.9–13.8, turned pages 6.0–12.2.
const OSD_MIN_CONFIDENCE: f32 = 3.0;
/// An upright pass scoring at least this mean word confidence is accepted without consulting the
/// detector. Measured: correctly oriented reads score ~96 on real and synthetic rasters; upside-down
/// and sideways reads scored 40–62. Keeps the ~1 s detector off the common, upright page.
const OSD_SKIP_CONFIDENCE: f32 = 80.0;

/// Whether an upright pass is good enough to skip orientation detection altogether.
fn upright_is_convincing(words: usize, mean_confidence: f32) -> bool {
  words > 0 && mean_confidence >= OSD_SKIP_CONFIDENCE
}

/// Who decided a raster's orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrientationSource {
  /// The first pass read upright well enough.
  Upright,
  /// Tesseract's orientation and script detector named the turn.
  Osd,
  /// The confidence retry over the quarter and half turns found a better score.
  Retry,
}

impl OrientationSource {
  fn label(self) -> &'static str {
    match self {
      OrientationSource::Upright => "upright",
      OrientationSource::Osd => "by OSD",
      OrientationSource::Retry => "by retry",
    }
  }
}

/// `detect_os` reports how the text currently lies (0, 90, 180 or 270 degrees); the clockwise turn
/// that puts it upright is the complement. The CLI prints "Orientation 270 / Rotate 90".
fn osd_correction(orient_deg: i32) -> u32 {
  ((360 - orient_deg.rem_euclid(360)) % 360) as u32
}

/// The turn to try first, when the detector is confident enough to be worth one OCR pass.
fn pick_first_rotation(osd: Option<(u32, f32)>) -> Option<u32> {
  osd
    .filter(|(_, confidence)| *confidence >= OSD_MIN_CONFIDENCE)
    .map(|(degrees, _)| degrees)
}

/// Orientation and confidence from Tesseract's OSD model, or `None` when `osd.traineddata` is not
/// installed, the image is too sparse for the detector, or anything else goes wrong. The detector
/// sees the full raster: shrinking it to 1200 px drops its confidence below the trust threshold
/// while saving almost no time.
fn osd_rotation(img: &DynamicImage, datapath: &str, ppi: Option<i32>) -> Option<(u32, f32)> {
  use tesseract_rs::TessPageSegMode;

  let rgb = img.to_rgb8();
  let (w, h) = rgb.dimensions();
  let tess = tesseract_rs::TesseractAPI::new();
  tess.init(datapath, "osd").ok()?;
  tess.set_page_seg_mode(TessPageSegMode::PSM_OSD_ONLY).ok()?;
  tess
    .set_image(rgb.as_raw(), w as i32, h as i32, 3, (w * 3) as i32)
    .ok()?;
  // Scans without a stated resolution otherwise make Tesseract assume 70 dpi and warn.
  let _ = tess.set_source_resolution(ppi.unwrap_or(300));
  let (orient_deg, orient_conf, _script, _script_conf) = tess.detect_os().ok()?;
  Some((osd_correction(orient_deg), orient_conf))
}

/// One OCR pass over the raster at a given turn.
struct OrientedPass {
  words: Vec<OcrWord>,
  degrees: u32,
  dims: (u32, u32),
  confidence: f32,
  source: OrientationSource,
}

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

/// OCR a single DynamicImage and return extracted text. The OSD detector names the turn first when
/// confident; otherwise the upright pass and the retry over the other turns decide.
fn ocr_dynamic_image(img: &DynamicImage, lang: &str) -> String {
  let datapath = get_tessdata_prefix().unwrap_or("");
  let mut tried: Vec<u32> = Vec::new();
  let mut best: Option<(String, f32)> = None;
  let mut consider = |degrees: u32, tried: &mut Vec<u32>| -> (bool, bool) {
    tried.push(degrees);
    let candidate = if degrees == 0 {
      ocr_image_text(img, lang)
    } else {
      ocr_image_text(&rotate_image(img, degrees), lang)
    };
    if best.as_ref().is_none_or(|b| candidate.1 > b.1) {
      best = Some(candidate);
    }
    let b = best.as_ref().expect("a pass was recorded");
    (
      !b.0.is_empty() && b.1 >= ROTATION_RETRY_CONFIDENCE,
      upright_is_convincing(b.0.split_whitespace().count(), b.1),
    )
  };
  let (upright_ok, convincing) = consider(0, &mut tried);
  if convincing {
    return best.expect("a pass was recorded").0;
  }
  if let Some(degrees) = pick_first_rotation(osd_rotation(img, datapath, None))
    && degrees != 0
    && consider(degrees, &mut tried).0
  {
    return best.expect("a pass was recorded").0;
  }
  if upright_ok {
    return best.expect("a pass was recorded").0;
  }
  for degrees in ROTATIONS {
    if !tried.contains(&degrees) && consider(degrees, &mut tried).0 {
      break;
    }
  }
  best.map(|b| b.0).unwrap_or_default()
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
  #[cfg(not(feature = "render"))]
  let render_available = false;
  #[cfg(feature = "render")]
  let render_available = render_mode != RENDER_MODE_NEVER;

  #[cfg(feature = "render")]
  let token_count = |s: &str| s.split_whitespace().count();

  let mut results: Vec<OcrPageText> = pool.install(|| {
    page_entries
      .par_iter()
      .map(|&(page_num, page_id)| {
        // Tier 1: native text; the routing decision is made on the page's geometry.
        let raw = doc.extract_text(&[page_num]).unwrap_or_default();
        let native = strip_footer_artifacts(&raw, &page_count_str);
        let layout = page_layout(doc, page_num, page_id);
        let path_ops = page_path_op_count(doc, page_id);
        #[cfg(feature = "render")]
        let outline = path_ops >= OUTLINE_PATH_OPS;
        let geo = page_geometry(doc, page_id);
        let decision = page_needs_ocr(&layout, min_len, path_ops, geo.bbox);

        // Forced rendering replaces the page wholesale, as in the layout cascade; a render that
        // read nothing falls through to the cascade and is not repeated by tier 3.
        #[cfg(feature = "render")]
        let mut rendered_already = false;
        #[cfg(feature = "render")]
        if render_mode == RENDER_MODE_ALWAYS
          && let Some(rendered_text) = try_render_ocr_page(&pdf_arc, page_num, render_dpi, lang)
        {
          rendered_already = true;
          if !rendered_text.is_empty() {
            return OcrPageText {
              page: page_num,
              text: rendered_text,
              source: TextSource::Rendered,
            };
          }
        }

        if decision == OcrDecision::Native {
          return OcrPageText {
            page: page_num,
            text: native,
            source: TextSource::Native,
          };
        }

        // Tier 2: OCR extracted raster images
        #[cfg_attr(not(feature = "render"), allow(unused_mut))]
        let mut text = if decision == OcrDecision::Ocr {
          ocr_page_images(doc, page_id, lang)
        } else {
          String::new()
        };
        #[cfg_attr(not(feature = "render"), allow(unused_mut))]
        let mut source = TextSource::Ocr;

        // Tier 3: render + OCR (render feature only). The text path carries no word boxes or
        // confidences, so unlike the layout cascade a weak tier 2 is a token count, the render
        // wins by token count rather than `ocr_score`, and the native stamp below is concatenated
        // in front of the OCR text rather than deduplicated against it.
        #[cfg(feature = "render")]
        if render_available && !rendered_already {
          let escalate = decision == OcrDecision::RenderOnly
            || (outline && token_count(&text) < WEAK_TIER2_WORDS)
            || (text.is_empty()
              && (path_ops > 10
                || page_has_form_xobjects(doc, page_id)
                || page_has_image_xobjects(doc, page_id)));
          if escalate
            && let Some(rendered_text) = try_render_ocr_page(&pdf_arc, page_num, render_dpi, lang)
            && token_count(&rendered_text) > token_count(&text)
          {
            text = rendered_text;
            source = TextSource::Rendered;
          }
        }
        let _ = render_available;

        // Hybrid page: the native stamp stays in front of the OCR text.
        let combined = match (native.trim().is_empty(), text.is_empty()) {
          (true, _) => text,
          (false, true) => native,
          (false, false) => format!("{native}\n{text}"),
        };
        OcrPageText {
          page: page_num,
          text: combined,
          source,
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

/// Word boxes in the frame where the text reads upright: the image as given, or rotated by the
/// returned clockwise angle. The upright pass runs first and is final when it is convincing
/// ([`OSD_SKIP_CONFIDENCE`]); otherwise the OSD detector names the turn to try next, and the blind
/// retry over the remaining turns is the last resort. Boxes are in pixels of the rotated image,
/// whose size is also returned, together with who decided.
fn ocr_words_best_orientation(
  img: &DynamicImage,
  lang: &str,
  ppi: Option<i32>,
) -> (Vec<OcrWord>, u32, (u32, u32), OrientationSource) {
  let datapath = get_tessdata_prefix().unwrap_or("");
  let mut tried: Vec<u32> = Vec::new();
  let mut best: Option<OrientedPass> = None;

  let mut consider =
    |degrees: u32, source: OrientationSource, tried: &mut Vec<u32>| -> (bool, bool) {
      tried.push(degrees);
      let (words, dims) = if degrees == 0 {
        (
          ocr_dynamic_image_words(img, lang, ppi),
          (img.width(), img.height()),
        )
      } else {
        let rotated = rotate_image(img, degrees);
        (
          ocr_dynamic_image_words(&rotated, lang, ppi),
          (rotated.width(), rotated.height()),
        )
      };
      let confidence = mean_confidence(&words);
      if best.as_ref().is_none_or(|b| confidence > b.confidence) {
        best = Some(OrientedPass {
          words,
          degrees,
          dims,
          confidence,
          source,
        });
      }
      let b = best.as_ref().expect("a pass was recorded");
      (
        !b.words.is_empty() && b.confidence >= ROTATION_RETRY_CONFIDENCE,
        upright_is_convincing(b.words.len(), b.confidence),
      )
    };

  // Upright first: a convincing read needs no detector.
  let (upright_ok, convincing) = consider(0, OrientationSource::Upright, &mut tried);
  if convincing {
    let b = best.expect("a pass was recorded");
    return (b.words, b.degrees, b.dims, b.source);
  }
  // Not convincing: ask the detector which turn to try before the blind retry.
  if let Some(degrees) = pick_first_rotation(osd_rotation(img, datapath, ppi))
    && degrees != 0
    && consider(degrees, OrientationSource::Osd, &mut tried).0
  {
    let b = best.expect("a pass was recorded");
    return (b.words, b.degrees, b.dims, b.source);
  }
  // An acceptable upright read that the detector did not overturn stands; the blind retry is only
  // for pages nothing has read well yet.
  if upright_ok {
    let b = best.expect("a pass was recorded");
    return (b.words, b.degrees, b.dims, b.source);
  }
  for degrees in ROTATIONS {
    if tried.contains(&degrees) {
      continue;
    }
    if consider(degrees, OrientationSource::Retry, &mut tried).0 {
      break;
    }
  }
  let b = best.expect("at least the upright pass ran");
  (b.words, b.degrees, b.dims, b.source)
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
        cell: None,
      }
    })
    .collect()
}

/// A box in a raster that was turned `degrees` clockwise (now `rotated_w` × `rotated_h` pixels),
/// mapped back onto the raster as painted. Boxes are `[left, top, right, bottom]`, y down.
fn unrotate_pixel_box(b: [i32; 4], degrees: u32, rotated_w: i32, rotated_h: i32) -> [i32; 4] {
  let [l, t, r, bt] = b;
  match degrees {
    90 => [t, rotated_w - r, bt, rotated_w - l],
    270 => [rotated_h - bt, l, rotated_h - t, r],
    180 => [rotated_w - r, rotated_h - bt, rotated_w - l, rotated_h - t],
    _ => b,
  }
}

/// A user-space box re-expressed in the page frame turned `degrees` clockwise about the page
/// box `bbox`: the box origin is kept and, for quarter turns, width and height swap.
fn turn_box(b: [f64; 4], degrees: u32, bbox: [f64; 4]) -> [f64; 4] {
  let [bx0, by0, bx1, by1] = bbox;
  let (w, h) = (bx1 - bx0, by1 - by0);
  let (x0, y0, x1, y1) = (b[0] - bx0, b[1] - by0, b[2] - bx0, b[3] - by0);
  match degrees {
    90 => [bx0 + y0, by0 + (w - x1), bx0 + y1, by0 + (w - x0)],
    270 => [bx0 + (h - y1), by0 + x0, bx0 + (h - y0), by0 + x1],
    180 => [
      bx0 + (w - x1),
      by0 + (h - y1),
      bx0 + (w - x0),
      by0 + (h - y0),
    ],
    _ => b,
  }
}

/// The page's turn is the one carrying the most confident OCR text, summed over the images that
/// read upright at it (`ocr_score` per image), so the tiles of a tiled scan vote together and one
/// tile a retry turned cannot outvote them. Ties keep the turn seen first.
fn dominant_rotation(per_image: &[(u32, f64)]) -> u32 {
  let mut totals: Vec<(u32, f64)> = Vec::new();
  for &(degrees, score) in per_image {
    match totals.iter_mut().find(|(d, _)| *d == degrees) {
      Some((_, total)) => *total += score,
      None => totals.push((degrees, score)),
    }
  }
  totals
    .iter()
    .fold(None::<(u32, f64)>, |best, &(degrees, total)| match best {
      Some((_, best_total)) if best_total >= total => best,
      _ => Some((degrees, total)),
    })
    .map(|(degrees, _)| degrees)
    .unwrap_or(0)
}

/// Express every word and image placement in the page frame turned `degrees` clockwise.
/// Baselines follow the turned box's bottom edge: a turned frame has no font baseline to keep.
fn turn_layout(words: &mut [Word], images: &mut [PlacedImage], degrees: u32, bbox: [f64; 4]) {
  if degrees == 0 {
    return;
  }
  for w in words.iter_mut() {
    let b = turn_box([w.x0, w.y0, w.x1, w.y1], degrees, bbox);
    w.x0 = b[0];
    w.y0 = b[1];
    w.x1 = b[2];
    w.y1 = b[3];
    w.baseline = b[1];
  }
  for i in images.iter_mut() {
    let b = turn_box([i.x0, i.y0, i.x1, i.y1], degrees, bbox);
    i.x0 = b[0];
    i.y0 = b[1];
    i.x1 = b[2];
    i.y1 = b[3];
  }
}

/// Word boxes from a turned raster, mapped back onto the raster as painted.
fn words_in_painted_raster(
  words: Vec<OcrWord>,
  degrees: u32,
  rotated_w: u32,
  rotated_h: u32,
) -> Vec<OcrWord> {
  words
    .into_iter()
    .map(|w| {
      let [left, top, right, bottom] = unrotate_pixel_box(
        [w.left, w.top, w.right, w.bottom],
        degrees,
        rotated_w as i32,
        rotated_h as i32,
      );
      OcrWord {
        left,
        top,
        right,
        bottom,
        ..w
      }
    })
    .collect()
}

/// Tier 2: OCR the page's embedded raster images and place each word through the image's `Do`
/// placement from the native pass, in the frame as painted. Returns the words and the page's
/// dominant turn; the caller turns the whole page frame by it.
fn ocr_page_image_words(
  doc: &Document,
  page_id: ObjectId,
  lang: &str,
  placements: &[PlacedImage],
  warnings: &mut Vec<String>,
) -> (Vec<Word>, u32) {
  let mut per_image: Vec<(String, Vec<Word>, u32, f64)> = Vec::new();
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
    let (words, degrees, (rw, rh), decided) = ocr_words_best_orientation(&img, lang, None);
    let painted = words_in_painted_raster(words, degrees, rw, rh);
    let mapped = map_ocr_words(
      painted,
      img.width(),
      img.height(),
      [first.x0, first.y0, first.x1, first.y1],
    );
    // A logo or rule that OCR read as a word or two (fewer than WEAK_TIER2_WORDS) has no
    // orientation worth reporting.
    if degrees != 0 && mapped.len() >= WEAK_TIER2_WORDS {
      warnings.push(format!(
        "image {name}: {} words read upright after a {degrees}° turn ({})",
        mapped.len(),
        decided.label()
      ));
    }
    let score = ocr_score(&mapped);
    per_image.push((name, mapped, degrees, score));
  }
  // An image that read as a word or two is no evidence of the page's turn; it does not vote.
  let votes: Vec<(u32, f64)> = per_image
    .iter()
    .filter(|(_, words, _, _)| words.len() >= WEAK_TIER2_WORDS)
    .map(|(_, _, degrees, score)| (*degrees, *score))
    .collect();
  let dominant = dominant_rotation(&votes);
  let mut out = Vec::new();
  for (name, words, degrees, _) in per_image {
    if degrees != dominant && words.len() >= WEAK_TIER2_WORDS {
      warnings.push(format!(
        "image {name}: {} words read upright at {degrees}° but the page frame is turned {dominant}°; they read sideways in it",
        words.len()
      ));
    }
    out.extend(words);
  }
  (out, dominant)
}

/// Tier-3 words from the rendered raster (the page as displayed, `/Rotate` applied, turned
/// `degrees` more to read upright), mapped back onto the page box in the page's own frame, the
/// frame native words use. The content's turn from that frame is `/Rotate` plus `degrees`.
#[cfg_attr(not(feature = "render"), allow(dead_code))]
fn place_rendered_words(
  words: Vec<OcrWord>,
  degrees: u32,
  turned_dims: (u32, u32),
  geo: &PageGeometry,
) -> (Vec<Word>, u32) {
  let quarter = |d: u32| d == 90 || d == 270;
  let (tw, th) = turned_dims;
  // back to the raster as rendered …
  let displayed = words_in_painted_raster(words, degrees, tw, th);
  let (dw, dh) = if quarter(degrees) { (th, tw) } else { (tw, th) };
  // … then to the page as painted, undoing /Rotate
  let painted = words_in_painted_raster(displayed, geo.rotate, dw, dh);
  let (pw, ph) = if quarter(geo.rotate) {
    (dh, dw)
  } else {
    (dw, dh)
  };
  (
    map_ocr_words(painted, pw, ph, geo.bbox),
    (geo.rotate + degrees) % 360,
  )
}

/// Tier 3: rasterise the page as displayed and place each OCR word on the page box in the page's
/// own frame; the caller turns the page frame by `rotation`, which includes `/Rotate`.
#[cfg(feature = "render")]
fn render_page_words(
  pdf_bytes: &[u8],
  page_num: u32,
  dpi: u32,
  lang: &str,
  geo: &PageGeometry,
  rotation: &mut u32,
  warnings: &mut Vec<String>,
) -> Option<Vec<Word>> {
  if !render::is_pdfium_available() {
    return None;
  }
  let page_index = (page_num - 1) as u16;
  let img = render::render_page_to_image_from_bytes(pdf_bytes, page_index, dpi)?;
  let (words, degrees, turned_dims, decided) =
    ocr_words_best_orientation(&img, lang, Some(dpi as i32));
  if degrees != 0 {
    warnings.push(format!(
      "page {page_num}: text reads upright after a {degrees}° turn ({})",
      decided.label()
    ));
  }
  let (placed, content_rotation) = place_rendered_words(words, degrees, turned_dims, geo);
  *rotation = content_rotation;
  Some(placed)
}

fn finish_ocr_layout(
  mut layout: PageLayout,
  words: Vec<Word>,
  fonts: &[FontMeta],
  source: TextSource,
  rotation: u32,
) -> PageLayout {
  let (words, lines) = words_to_lines(words, fonts, &[]);
  layout.words = words;
  layout.lines = lines;
  layout.source = source;
  layout.content_rotation = rotation;
  if rotation == 90 || rotation == 270 {
    std::mem::swap(&mut layout.width, &mut layout.height);
  }
  layout
}

/// Where a page goes in the OCR cascade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OcrDecision {
  /// Native glyphs are the page: no OCR.
  Native,
  /// OCR the embedded rasters (tier 2), escalating to the render tier when weak.
  Ocr,
  /// No glyphs and no rasters but painted paths ("print as paths"): only the render tier can read it.
  RenderOnly,
}

/// Native words covering less than this share of the page do not prove the page readable…
const OCR_WORD_COVERAGE_MAX: f64 = 0.02;
/// …when image placements cover more than this share: that is a scan with a stamp on it.
const OCR_IMAGE_COVERAGE_MIN: f64 = 0.5;
/// Pages carrying at least this many path operators are text printed as outlines (or dense
/// drawings): not readable from glyphs, and only the render tier can read the paths.
const OUTLINE_PATH_OPS: usize = 200;
/// A tier-2 result this thin on a page with painted paths is escalated to the render tier.
const WEAK_TIER2_WORDS: usize = 3;
const WEAK_TIER2_CONFIDENCE: f64 = 0.3;

fn clipped_area(b: [f64; 4], bbox: [f64; 4]) -> f64 {
  let w = b[2].min(bbox[2]) - b[0].max(bbox[0]);
  let h = b[3].min(bbox[3]) - b[1].max(bbox[1]);
  if w <= 0.0 || h <= 0.0 { 0.0 } else { w * h }
}

/// Share of the page box `bbox` covered by `boxes` (clipped to it; overlaps count twice). Boxes are
/// raw user space, so a CropBox away from the origin must be clipped against, not an origin box.
fn coverage(boxes: impl Iterator<Item = [f64; 4]>, bbox: [f64; 4]) -> f64 {
  let page = ((bbox[2] - bbox[0]) * (bbox[3] - bbox[1])).max(1.0);
  boxes.map(|b| clipped_area(b, bbox)).sum::<f64>() / page
}

/// Decide by coverage, not by character count alone: a scanned page with a few stamped native
/// characters is still a scan, an outline-text page with a logo and a stamp is not readable
/// natively, and a sparse cover sheet without rasters or heavy paths is still native.
pub(crate) fn page_needs_ocr(
  native: &PageLayout,
  min_len: u32,
  path_ops: usize,
  bbox: [f64; 4],
) -> OcrDecision {
  let outline = path_ops >= OUTLINE_PATH_OPS;
  if native.words.is_empty() && native.images.is_empty() {
    return if outline {
      OcrDecision::RenderOnly
    } else {
      OcrDecision::Ocr
    };
  }
  if non_ws_chars(native) < min_len as usize {
    return OcrDecision::Ocr;
  }
  let words = coverage(native.words.iter().map(|w| [w.x0, w.y0, w.x1, w.y1]), bbox);
  let images = coverage(native.images.iter().map(|i| [i.x0, i.y0, i.x1, i.y1]), bbox).min(1.0);
  if words < OCR_WORD_COVERAGE_MAX && (images > OCR_IMAGE_COVERAGE_MIN || outline) {
    OcrDecision::Ocr
  } else {
    OcrDecision::Native
  }
}

fn intersection_over_union(a: &Word, b: &Word) -> f64 {
  let ix = (a.x1.min(b.x1) - a.x0.max(b.x0)).max(0.0);
  let iy = (a.y1.min(b.y1) - a.y0.max(b.y0)).max(0.0);
  let inter = ix * iy;
  let union = (a.x1 - a.x0) * (a.y1 - a.y0) + (b.x1 - b.x0) * (b.y1 - b.y0) - inter;
  if union <= 0.0 { 0.0 } else { inter / union }
}

/// Native words first, then every OCR word that does not sit on a native word (IoU > 0.5 is the
/// same word read twice). Returns the merged words and how many OCR words were dropped.
pub(crate) fn merge_native_and_ocr(native: Vec<Word>, ocr: Vec<Word>) -> (Vec<Word>, usize) {
  let mut out = native;
  let native_count = out.len();
  let mut dropped = 0usize;
  for w in ocr {
    if out[..native_count]
      .iter()
      .any(|n| intersection_over_union(n, &w) > 0.5)
    {
      dropped += 1;
    } else {
      out.push(w);
    }
  }
  (out, dropped)
}

fn ocr_score(words: &[Word]) -> f64 {
  if words.is_empty() {
    return 0.0;
  }
  let mean = words
    .iter()
    .map(|w| w.confidence.unwrap_or(0.0))
    .sum::<f64>()
    / words.len() as f64;
  words.len() as f64 * mean
}

fn tier2_is_weak(words: &[Word]) -> bool {
  if words.len() < WEAK_TIER2_WORDS {
    return true;
  }
  let mean = words
    .iter()
    .map(|w| w.confidence.unwrap_or(0.0))
    .sum::<f64>()
    / words.len() as f64;
  mean < WEAK_TIER2_CONFIDENCE
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
  #[cfg(not(feature = "render"))]
  let render_available = false;
  #[cfg(feature = "render")]
  let render_available = render_mode != RENDER_MODE_NEVER;

  let mut results: Vec<PageLayout> = pool.install(|| {
    page_entries
      .par_iter()
      .map(|&(page_num, page_id)| {
        // Tier 1: native glyph placement (also yields image placements for tier 2).
        let NativeLayout {
          layout: native,
          words: native_words,
          fonts,
        } = native_layout(doc, page_num, page_id);
        let mut warnings = Vec::new();
        let mut rotation = 0u32;
        let path_ops = page_path_op_count(doc, page_id);
        let outline = path_ops >= OUTLINE_PATH_OPS;

        let geo = page_geometry(doc, page_id);
        let mut native = native;

        // Forced rendering replaces the page wholesale; the hybrid merge below is for the cascade.
        // A render that read nothing leaves no trace and is not repeated by tier 3.
        #[cfg(feature = "render")]
        let mut rendered_already = false;
        #[cfg(feature = "render")]
        if render_mode == RENDER_MODE_ALWAYS {
          let mut forced_rotation = 0u32;
          let mut forced_warnings = Vec::new();
          if let Some(mut words) = render_page_words(
            &pdf_arc,
            page_num,
            render_dpi,
            lang,
            &geo,
            &mut forced_rotation,
            &mut forced_warnings,
          ) {
            rendered_already = true;
            if !words.is_empty() {
              warnings.extend(forced_warnings);
              turn_layout(&mut words, &mut native.images, forced_rotation, geo.bbox);
              let mut out =
                finish_ocr_layout(native, words, &[], TextSource::Rendered, forced_rotation);
              out.warnings.extend(warnings);
              return out;
            }
            warnings.push(format!(
              "page {page_num}: forced render read no words; the cascade decided the page"
            ));
          }
        }

        let decision = page_needs_ocr(&native, min_len, path_ops, geo.bbox);
        if decision == OcrDecision::Native {
          return native;
        }

        // Tier 2: OCR embedded raster images.
        #[cfg_attr(not(feature = "render"), allow(unused_mut))]
        let mut ocr_words = if decision == OcrDecision::Ocr {
          let (words, dominant) =
            ocr_page_image_words(doc, page_id, lang, &native.images, &mut warnings);
          rotation = dominant;
          words
        } else {
          Vec::new()
        };
        #[cfg_attr(not(feature = "render"), allow(unused_mut))]
        let mut source = TextSource::Ocr;

        // Tier 3: render + OCR for outline text, weak tier-2 results on path-heavy pages, and
        // rasters tier 2 could not decode.
        #[cfg(feature = "render")]
        if render_available {
          let escalate = decision == OcrDecision::RenderOnly
            || (outline && tier2_is_weak(&ocr_words))
            || (ocr_words.is_empty()
              && (path_ops > 10
                || page_has_form_xobjects(doc, page_id)
                || page_has_image_xobjects(doc, page_id)));
          if escalate && !rendered_already {
            let mut render_rotation = 0u32;
            let mut render_warnings = Vec::new();
            if let Some(rendered) = render_page_words(
              &pdf_arc,
              page_num,
              render_dpi,
              lang,
              &geo,
              &mut render_rotation,
              &mut render_warnings,
            ) && ocr_score(&rendered) > ocr_score(&ocr_words)
            {
              if !ocr_words.is_empty() {
                warnings.push(format!(
                  "page {page_num}: rendered page replaced {} weak tier-2 words",
                  ocr_words.len()
                ));
              }
              ocr_words = rendered;
              rotation = render_rotation;
              warnings.extend(render_warnings);
              source = TextSource::Rendered;
            }
          }
        }
        if outline && !render_available && (decision == OcrDecision::RenderOnly || tier2_is_weak(&ocr_words)) {
          warnings.push(format!(
            "page {page_num}: {path_ops} path operators paint text as outlines; only the render tier can read them"
          ));
        }

        // Hybrid page: native words are real evidence and stay; OCR adds what they do not cover.
        let native_count = native_words.len();
        let (merged, dropped) = merge_native_and_ocr(native_words, ocr_words);
        if merged.is_empty() {
          warnings.push(format!("page {page_num}: OCR found no words"));
        }
        if native_count > 0 {
          if merged.len() > native_count {
            warnings.push(format!(
              "page {page_num}: {native_count} native words kept alongside OCR ({dropped} duplicates dropped)"
            ));
          } else {
            warnings.push(format!(
              "page {page_num}: OCR found no new words; {native_count} native words kept"
            ));
          }
        }
        let mut merged = merged;
        turn_layout(&mut merged, &mut native.images, rotation, geo.bbox);
        let mut out = finish_ocr_layout(native, merged, &fonts, source, rotation);
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

#[cfg(test)]
mod cascade_tests {
  use super::*;
  use crate::types::TextWord;

  fn page(words: &[(f64, f64, f64, f64, &str)], images: &[(f64, f64, f64, f64)]) -> PageLayout {
    PageLayout {
      page: 1,
      width: 612.0,
      height: 792.0,
      rotate: 0,
      content_rotation: 0,
      source: TextSource::Native,
      sheet: None,
      words: words
        .iter()
        .map(|&(x0, y0, x1, y1, t)| TextWord {
          text: t.into(),
          x0,
          y0,
          x1,
          y1,
          line: 0,
          font_size: None,
          font_name: None,
          bold: None,
          italic: None,
          symbolic: None,
          confidence: None,
          cell: None,
        })
        .collect(),
      lines: Vec::new(),
      images: images
        .iter()
        .enumerate()
        .map(|(i, &(x0, y0, x1, y1))| PlacedImage {
          xobject_name: format!("Im{i}"),
          x0,
          y0,
          x1,
          y1,
        })
        .collect(),
      warnings: Vec::new(),
    }
  }

  fn origin_box(p: &PageLayout) -> [f64; 4] {
    [0.0, 0.0, p.width, p.height]
  }

  #[test]
  fn coverage_is_measured_against_the_page_box_not_an_origin_box() {
    // a CropBox at [400, 400, 1012, 1192] with the scan filling it and two stamped words on it:
    // clipped to an origin box the scan would cover 17% and the page would wrongly stay Native
    let bbox = [400.0, 400.0, 1012.0, 1192.0];
    let p = page(
      &[
        (440.0, 1160.0, 480.0, 1170.0, "PO"),
        (485.0, 1160.0, 525.0, 1170.0, "4711"),
      ],
      &[(406.0, 406.0, 1006.0, 1186.0)],
    );
    assert_eq!(page_needs_ocr(&p, 1, 0, bbox), OcrDecision::Ocr);
    assert_eq!(
      page_needs_ocr(&p, 1, 0, origin_box(&p)),
      OcrDecision::Native
    );
  }

  #[test]
  fn ocr_score_weighs_words_by_confidence_and_weak_tier2_needs_words_and_confidence() {
    let row = |conf: f64| -> Vec<Word> {
      (0..4)
        .map(|i| {
          let x = f64::from(i) * 10.0;
          word(x, 0.0, x + 8.0, 10.0, Some(conf))
        })
        .collect()
    };
    let (strong, faint) = (row(0.9), row(0.2));
    assert!((ocr_score(&strong) - 3.6).abs() < 1e-9);
    assert!(ocr_score(&strong) > ocr_score(&faint));
    assert_eq!(ocr_score(&[]), 0.0);
    assert!(!tier2_is_weak(&strong));
    assert!(
      tier2_is_weak(&faint),
      "a mean confidence under WEAK_TIER2_CONFIDENCE is weak"
    );
    assert!(
      tier2_is_weak(&strong[..2]),
      "fewer than WEAK_TIER2_WORDS words is weak"
    );
    assert!(
      !tier2_is_weak(&strong[..WEAK_TIER2_WORDS]),
      "exactly WEAK_TIER2_WORDS words is not weak, and such an image votes on the page's turn"
    );
  }

  fn word(x0: f64, y0: f64, x1: f64, y1: f64, confidence: Option<f64>) -> Word {
    Word {
      text: "w".into(),
      x0,
      y0,
      x1,
      y1,
      baseline: y0,
      font_size: None,
      font: None,
      confidence,
      cell: None,
    }
  }

  #[test]
  fn coverage_rule_sends_stamped_scan_to_ocr() {
    // two stamped words over a full-page scan: 0.2% word area, 97% image area
    let p = page(
      &[
        (40.0, 760.0, 80.0, 770.0, "PO"),
        (85.0, 760.0, 125.0, 770.0, "4711"),
      ],
      &[(6.0, 6.0, 606.0, 786.0)],
    );
    assert_eq!(page_needs_ocr(&p, 1, 0, origin_box(&p)), OcrDecision::Ocr);
  }

  #[test]
  fn outline_text_with_a_logo_and_a_stamp_is_ocr() {
    // two stamped words, a logo covering 1.6% of the page, and thousands of path operators
    let p = page(
      &[
        (40.0, 760.0, 80.0, 770.0, "PO"),
        (85.0, 760.0, 125.0, 770.0, "4711"),
      ],
      &[(44.0, 695.0, 188.0, 750.0)],
    );
    assert_eq!(
      page_needs_ocr(&p, 1, 50_000, origin_box(&p)),
      OcrDecision::Ocr
    );
  }

  #[test]
  fn ruled_cover_sheet_stays_native() {
    // a few words and a table border (dozens of path operators) is still a native page
    let p = page(
      &[
        (50.0, 700.0, 120.0, 712.0, "Cover"),
        (125.0, 700.0, 180.0, 712.0, "sheet"),
      ],
      &[],
    );
    assert_eq!(
      page_needs_ocr(&p, 1, 80, origin_box(&p)),
      OcrDecision::Native
    );
  }

  #[test]
  fn coverage_rule_keeps_sparse_cover_sheet_native() {
    let p = page(
      &[
        (50.0, 700.0, 120.0, 712.0, "Cover"),
        (125.0, 700.0, 180.0, 712.0, "sheet"),
        (50.0, 680.0, 90.0, 692.0, "for"),
        (95.0, 680.0, 160.0, 692.0, "invoice"),
        (165.0, 680.0, 220.0, 692.0, "bundle"),
      ],
      &[],
    );
    assert_eq!(
      page_needs_ocr(&p, 1, 0, origin_box(&p)),
      OcrDecision::Native
    );
  }

  #[test]
  fn min_text_length_still_forces_ocr() {
    let p = page(&[(50.0, 700.0, 120.0, 712.0, "Cover")], &[]);
    assert_eq!(page_needs_ocr(&p, 999, 0, origin_box(&p)), OcrDecision::Ocr);
  }

  #[test]
  fn outline_page_goes_to_render() {
    let p = page(&[], &[]);
    assert_eq!(
      page_needs_ocr(&p, 1, 5_000, origin_box(&p)),
      OcrDecision::RenderOnly
    );
    assert_eq!(page_needs_ocr(&p, 1, 0, origin_box(&p)), OcrDecision::Ocr);
  }

  #[test]
  fn merge_drops_ocr_duplicates_of_native_words() {
    let native = vec![word(0.0, 0.0, 10.0, 10.0, None)];
    let ocr = vec![
      word(1.0, 1.0, 10.0, 10.0, Some(0.9)),
      word(50.0, 50.0, 60.0, 60.0, Some(0.8)),
    ];
    let (merged, dropped) = merge_native_and_ocr(native, ocr);
    assert_eq!(dropped, 1);
    assert_eq!(merged.len(), 2);
    assert_eq!(merged[0].confidence, None, "native word first, kept as is");
    assert_eq!(
      merged[1].confidence,
      Some(0.8),
      "the disjoint OCR word survives"
    );
  }
}

#[cfg(test)]
mod osd_tests {
  use super::*;

  #[test]
  fn osd_below_threshold_is_ignored() {
    assert_eq!(pick_first_rotation(Some((90, 2.9))), None);
  }

  #[test]
  fn osd_seeds_the_first_pass() {
    assert_eq!(pick_first_rotation(Some((90, 6.0))), Some(90));
    assert_eq!(pick_first_rotation(Some((0, 10.0))), Some(0));
  }

  #[test]
  fn no_osd_means_no_seed() {
    assert_eq!(pick_first_rotation(None), None);
  }

  #[test]
  fn convincing_upright_reads_skip_the_detector() {
    assert!(upright_is_convincing(182, 95.6), "a real upright page");
    assert!(
      !upright_is_convincing(24, 62.4),
      "the synthetic page read upside down"
    );
    assert!(
      !upright_is_convincing(449, 41.4),
      "a real page read sideways"
    );
    assert!(
      !upright_is_convincing(0, 100.0),
      "no words is never convincing"
    );
  }

  #[test]
  fn osd_orientation_becomes_a_clockwise_correction() {
    // Tesseract reports how the text currently lies; the CLI prints "Orientation 270 / Rotate 90".
    assert_eq!(osd_correction(270), 90);
    assert_eq!(osd_correction(90), 270);
    assert_eq!(osd_correction(180), 180);
    assert_eq!(osd_correction(0), 0);
  }
}

#[cfg(test)]
mod frame_tests {
  use super::*;

  #[test]
  fn unrotating_a_pixel_box_undoes_the_raster_turn() {
    // an 800×200 raster turned 90° clockwise is 200×800; a box in the turned raster maps back
    assert_eq!(
      unrotate_pixel_box([140, 10, 180, 110], 90, 200, 800),
      [10, 20, 110, 60]
    );
    // 270° clockwise (= 90° counter-clockwise): the same original box lands elsewhere
    assert_eq!(
      unrotate_pixel_box([20, 690, 60, 790], 270, 200, 800),
      [10, 20, 110, 60]
    );
    // 180° keeps the dimensions
    assert_eq!(
      unrotate_pixel_box([690, 140, 790, 180], 180, 800, 200),
      [10, 20, 110, 60]
    );
    assert_eq!(
      unrotate_pixel_box([10, 20, 110, 60], 0, 800, 200),
      [10, 20, 110, 60]
    );
  }

  #[test]
  fn turning_a_box_in_the_page_frame_moves_its_corners() {
    let page = [0.0, 0.0, 612.0, 792.0];
    assert_eq!(
      turn_box([0.0, 0.0, 10.0, 20.0], 90, page),
      [0.0, 602.0, 20.0, 612.0]
    );
    assert_eq!(
      turn_box([0.0, 0.0, 10.0, 20.0], 270, page),
      [772.0, 0.0, 792.0, 10.0]
    );
    assert_eq!(
      turn_box([0.0, 0.0, 10.0, 20.0], 180, page),
      [602.0, 772.0, 612.0, 792.0]
    );
    assert_eq!(
      turn_box([0.0, 0.0, 10.0, 20.0], 0, page),
      [0.0, 0.0, 10.0, 20.0]
    );
    // a CropBox away from the origin keeps its origin
    let cropped = [100.0, 50.0, 400.0, 250.0];
    assert_eq!(
      turn_box([100.0, 50.0, 110.0, 70.0], 90, cropped),
      [100.0, 340.0, 120.0, 350.0]
    );
  }

  #[test]
  fn a_quarter_turn_followed_by_its_complement_is_the_identity() {
    let page = [0.0, 0.0, 612.0, 792.0];
    let turned_page = turn_box(page, 90, page);
    let b = [50.0, 700.0, 300.0, 720.0];
    assert_eq!(turn_box(turn_box(b, 90, page), 270, turned_page), b);
  }

  fn ocr_word(b: [i32; 4]) -> OcrWord {
    OcrWord {
      text: "w".into(),
      left: b[0],
      top: b[1],
      right: b[2],
      bottom: b[3],
      confidence: 90.0,
    }
  }

  fn close(a: [f64; 4], b: [f64; 4]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9)
  }

  fn geo(rotate: u32) -> PageGeometry {
    PageGeometry {
      width: 612.0,
      height: 792.0,
      rotate,
      bbox: [0.0, 0.0, 612.0, 792.0],
    }
  }

  #[test]
  fn rendered_words_land_in_the_page_frame_and_the_turn_includes_rotate() {
    // /Rotate 90: the portrait page renders landscape (792×612 px at 72 dpi); a word across the
    // displayed top-left corner is a vertical box at the bottom-left of the page's own frame
    let (words, turn) =
      place_rendered_words(vec![ocr_word([0, 0, 100, 20])], 0, (792, 612), &geo(90));
    assert_eq!(turn, 90);
    let w = &words[0];
    let b = [w.x0, w.y0, w.x1, w.y1];
    assert!(close(b, [0.0, 0.0, 20.0, 100.0]), "{b:?}");
    // turning the page frame by that turn puts it back across the displayed top-left corner
    let turned = turn_box(b, turn, geo(90).bbox);
    assert!(close(turned, [0.0, 592.0, 100.0, 612.0]), "{turned:?}");
    // no /Rotate: a box at the top-left of a raster turned 180° is at the bottom-right of the
    // rendered page, and the turn is the OCR turn alone
    let (words, turn) =
      place_rendered_words(vec![ocr_word([0, 0, 100, 20])], 180, (612, 792), &geo(0));
    assert_eq!(turn, 180);
    let w = &words[0];
    let b = [w.x0, w.y0, w.x1, w.y1];
    assert!(close(b, [512.0, 0.0, 612.0, 20.0]), "{b:?}");
    // /Rotate 90 plus a 270° OCR turn is no turn at all
    let (_, turn) = place_rendered_words(vec![], 270, (792, 612), &geo(90));
    assert_eq!(turn, 0);
  }

  #[test]
  fn the_turn_carrying_the_most_confident_text_sets_the_page_rotation() {
    assert_eq!(dominant_rotation(&[(270, 10.0), (180, 3.0)]), 270);
    // tiles vote together: two upright tiles outweigh one bigger tile a retry turned
    assert_eq!(dominant_rotation(&[(90, 6.0), (0, 4.0), (0, 4.0)]), 0);
    assert_eq!(
      dominant_rotation(&[(0, 5.0), (90, 5.0)]),
      0,
      "ties keep the first turn seen"
    );
    assert_eq!(dominant_rotation(&[]), 0);
  }
}
