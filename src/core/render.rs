use image::{DynamicImage, ImageFormat};
use pdfium_render::prelude::*;
use std::io::Cursor;
use std::sync::{Mutex, OnceLock};

static PDFIUM: OnceLock<Result<Pdfium, String>> = OnceLock::new();

/// Serializes all PDFium FFI operations across libuv worker threads.
static PDFIUM_LOCK: Mutex<()> = Mutex::new(());

/// Render-mode constants shared by both the render and OCR subsystems.
pub(crate) const RENDER_MODE_AUTO: u8 = 0;
pub(crate) const RENDER_MODE_NEVER: u8 = 1;
pub(crate) const RENDER_MODE_ALWAYS: u8 = 2;

fn try_bind_pdfium_at(path: &str) -> Option<Pdfium> {
  Pdfium::bind_to_library(path).ok().map(Pdfium::new)
}

fn init_pdfium() -> Result<Pdfium, String> {
  // 1. PDFIUM_LIBRARY_PATH env var
  if let Ok(path) = std::env::var("PDFIUM_LIBRARY_PATH") {
    if let Some(p) = try_bind_pdfium_at(&path) {
      return Ok(p);
    }
  }

  // 2. System library paths (dlopen search)
  if let Ok(bindings) = Pdfium::bind_to_system_library() {
    return Ok(Pdfium::new(bindings));
  }

  Err("PDFium library not found. Install PDFium or set PDFIUM_LIBRARY_PATH.".to_string())
}

fn get_pdfium() -> Result<&'static Pdfium, &'static str> {
  PDFIUM
    .get_or_init(init_pdfium)
    .as_ref()
    .map_err(|e| e.as_str())
}

/// Attempt to initialize PDFium with a custom path (called before the lazy singleton).
fn get_pdfium_with_path(custom_path: Option<&str>) -> Result<&'static Pdfium, &'static str> {
  if let Some(path) = custom_path {
    if PDFIUM.get().is_none() {
      let result = try_bind_pdfium_at(path).map(Ok).unwrap_or_else(init_pdfium);
      let _ = PDFIUM.set(result);
    }
  }
  get_pdfium()
}

pub(crate) fn is_pdfium_available() -> bool {
  get_pdfium().is_ok()
}

pub(crate) fn ensure_pdfium_with_path(path: Option<&str>) -> Result<(), &'static str> {
  get_pdfium_with_path(path).map(|_| ())
}

pub(crate) fn normalize_dpi(dpi: Option<u32>) -> u32 {
  dpi.unwrap_or(300).clamp(72, 600)
}

// ── Internal (caller must hold PDFIUM_LOCK) ─────────────────────

fn render_page_to_image_inner(pdf_bytes: &[u8], page_index: u16, dpi: u32) -> Option<DynamicImage> {
  let pdfium = get_pdfium().ok()?;
  let doc = pdfium.load_pdf_from_byte_slice(pdf_bytes, None).ok()?;
  let page = doc.pages().get(page_index).ok()?;

  let config = PdfRenderConfig::new()
    .set_target_width(((page.width().value * dpi as f32) / 72.0) as Pixels)
    .set_target_height(((page.height().value * dpi as f32) / 72.0) as Pixels)
    .render_form_data(true);

  page
    .render_with_config(&config)
    .ok()
    .map(|bmp| bmp.as_image())
}

// ── Public lock-acquiring wrappers ──────────────────────────────

pub(crate) fn render_page_to_image(
  pdf_bytes: &[u8],
  page_index: u16,
  dpi: u32,
) -> Option<DynamicImage> {
  let _guard = PDFIUM_LOCK.lock().unwrap();
  render_page_to_image_inner(pdf_bytes, page_index, dpi)
}

#[allow(dead_code)]
pub(crate) fn render_page_to_png(
  pdf_bytes: &[u8],
  page_index: u16,
  dpi: u32,
) -> Option<(u32, u32, Vec<u8>)> {
  let _guard = PDFIUM_LOCK.lock().unwrap();
  let img = render_page_to_image_inner(pdf_bytes, page_index, dpi)?;
  let (w, h) = (img.width(), img.height());
  let mut buf = Cursor::new(Vec::new());
  img.write_to(&mut buf, ImageFormat::Png).ok()?;
  Some((w, h, buf.into_inner()))
}

pub(crate) fn render_pages_to_png(
  pdf_bytes: &[u8],
  page_indices: &[u16],
  dpi: u32,
) -> Vec<(u16, u32, u32, Vec<u8>)> {
  let _guard = PDFIUM_LOCK.lock().unwrap();

  let pdfium = match get_pdfium() {
    Ok(p) => p,
    Err(_) => return Vec::new(),
  };
  let doc = match pdfium.load_pdf_from_byte_slice(pdf_bytes, None) {
    Ok(d) => d,
    Err(_) => return Vec::new(),
  };

  let mut results = Vec::with_capacity(page_indices.len());

  for &idx in page_indices {
    let page = match doc.pages().get(idx) {
      Ok(p) => p,
      Err(_) => continue,
    };

    let config = PdfRenderConfig::new()
      .set_target_width(((page.width().value * dpi as f32) / 72.0) as Pixels)
      .set_target_height(((page.height().value * dpi as f32) / 72.0) as Pixels)
      .render_form_data(true);

    if let Ok(bmp) = page.render_with_config(&config) {
      let img = bmp.as_image();
      let (w, h) = (img.width(), img.height());
      let mut buf = Cursor::new(Vec::new());
      if img.write_to(&mut buf, ImageFormat::Png).is_ok() {
        results.push((idx, w, h, buf.into_inner()));
      }
    }
  }

  results
}

/// Render a single page to a DynamicImage. Used by the OCR cascade.
#[cfg_attr(not(feature = "ocr"), allow(dead_code))]
pub(crate) fn render_page_to_image_from_bytes(
  pdf_bytes: &[u8],
  page_index: u16,
  dpi: u32,
) -> Option<DynamicImage> {
  render_page_to_image(pdf_bytes, page_index, dpi)
}
