use crate::core::images::collect_page_decoded_images;
use crate::core::text::{detect_headers_footers, strip_footer_artifacts};
use crate::types::{OcrPageText, OcrStructuredPageText, PageText, TextSource};
use image::DynamicImage;
use lopdf::{Document, ObjectId};
use napi::Result;
use rayon::prelude::*;
use std::sync::Arc;

#[cfg(all(feature = "ocr", feature = "render"))]
use crate::core::images::{page_has_form_xobjects, page_has_vector_content};
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

/// OCR a single DynamicImage and return extracted text.
fn ocr_dynamic_image(img: &DynamicImage, lang: &str) -> String {
  let datapath = get_tessdata_prefix().unwrap_or("");
  let rgb = img.to_rgb8();
  let (w, h) = rgb.dimensions();
  let pixels = rgb.as_raw();

  let tess = tesseract_rs::TesseractAPI::new();
  if tess.init(datapath, lang).is_err() {
    return String::new();
  }
  if tess
    .set_image(pixels, w as i32, h as i32, 3, (w * 3) as i32)
    .is_err()
  {
    return String::new();
  }
  match tess.get_utf8_text() {
    Ok(text) => {
      let trimmed = text.trim();
      if trimmed.is_empty() {
        String::new()
      } else {
        trimmed.to_string()
      }
    }
    Err(_) => String::new(),
  }
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
// accepted as parameters but silently ignored).
#[cfg(not(feature = "render"))]
pub(crate) const RENDER_MODE_AUTO: u8 = 0;
#[cfg(not(feature = "render"))]
pub(crate) const RENDER_MODE_NEVER: u8 = 1;
#[cfg(not(feature = "render"))]
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
            if render_mode == RENDER_MODE_ALWAYS {
              if let Some(rendered_text) = try_render_ocr_page(&pdf_arc, page_num, render_dpi, lang)
              {
                if !rendered_text.is_empty() {
                  return OcrPageText {
                    page: page_num,
                    text: rendered_text,
                    source: TextSource::Rendered,
                  };
                }
              }
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
            let should_render = render_mode == RENDER_MODE_ALWAYS
              || page_has_form_xobjects(doc, page_id)
              || page_has_vector_content(doc, page_id);

            if should_render {
              if let Some(rendered_text) = try_render_ocr_page(&pdf_arc, page_num, render_dpi, lang)
              {
                if !rendered_text.is_empty() {
                  return OcrPageText {
                    page: page_num,
                    text: rendered_text,
                    source: TextSource::Rendered,
                  };
                }
              }
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
      source: match ocr.source {
        TextSource::Native => TextSource::Native,
        TextSource::Ocr => TextSource::Ocr,
        #[cfg(feature = "render")]
        TextSource::Rendered => TextSource::Rendered,
      },
    })
    .collect()
}
