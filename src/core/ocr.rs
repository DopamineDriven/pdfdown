use crate::core::images::collect_page_decoded_images;
use crate::core::text::{detect_headers_footers, strip_footer_artifacts};
use crate::types::{OcrPageText, OcrStructuredPageText, PageText, TextSource};
use image::DynamicImage;
use lopdf::{Document, ObjectId};
use napi::Result;
use rayon::prelude::*;
use std::sync::Arc;

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
/// Checks `TESSDATA_PREFIX` env var first (user override), then falls back to
/// parsing the output of `tesseract --list-langs` (e.g.
/// `List of available languages in "/usr/share/tesseract-ocr/5/tessdata/" (161):`).
/// Returns `None` if neither source yields a path, letting tesseract use its
/// compiled-in default.
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

fn ocr_page_images(doc: &Document, page_id: ObjectId, lang: &str) -> String {
  let datapath = get_tessdata_prefix().unwrap_or("");
  let images: Vec<DynamicImage> = collect_page_decoded_images(doc, page_id);
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

pub(crate) fn extract_text_with_ocr(
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

pub(crate) fn detect_headers_footers_ocr(pages: &[OcrPageText]) -> Vec<OcrStructuredPageText> {
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
