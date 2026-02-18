#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi::{Env, Task};
use napi_derive::napi;

use lopdf::Document;
use std::sync::Arc;

mod core;
mod types;

// Public API types (appear in generated .d.ts)
pub use types::{
  BoxType, Capabilities, PageAnnotation, PageBox, PageImage, PageText, PdfDocument, PdfMeta,
  StructuredPageText,
};

#[cfg(feature = "ocr")]
pub use types::{OcrOptions, OcrPageText, OcrStructuredPageText, PdfDocumentOcr, TextSource};

#[cfg(feature = "render")]
pub use types::{RawRenderedPage, RenderMode, RenderOptions, RenderedPage};

// Internal plumbing (used by Task impls in this file — must be `pub` for napi Task trait)
pub use types::{RawPageImage, RawPdfDocument};

#[cfg(feature = "ocr")]
pub use types::RawPdfDocumentOcr;

// ── Step 1: Compile-time assertion that lopdf::Document is Send + Sync ──
const _: () = {
  fn assert_send_sync<T: Send + Sync>() {}
  fn check() {
    assert_send_sync::<lopdf::Document>();
  }
};

// ── Shared helpers ──────────────────────────────────────────────

use crate::core::document::{extract_all, extract_annotations};
use crate::core::images::extract_images_raw;
use crate::core::meta::extract_metadata;
use crate::core::text::{extract_structured_text, extract_text};

#[cfg(feature = "ocr")]
use crate::core::document::extract_all_with_ocr;
#[cfg(feature = "ocr")]
use crate::core::ocr::{extract_text_with_ocr, normalize_max_threads};

fn load_doc(buf: &[u8]) -> Result<Document> {
  Document::load_mem(buf).map_err(|e| Error::from_reason(format!("Failed to load PDF: {e}")))
}

/// Extract render mode (u8) from OcrOptions when render feature is enabled.
#[cfg(all(feature = "ocr", feature = "render"))]
fn extract_render_mode(opts: &Option<OcrOptions>) -> u8 {
  use crate::core::render::{RENDER_MODE_ALWAYS, RENDER_MODE_AUTO, RENDER_MODE_NEVER};
  opts
    .as_ref()
    .and_then(|o| o.render.as_ref())
    .map(|m| match m {
      RenderMode::Auto => RENDER_MODE_AUTO,
      RenderMode::Never => RENDER_MODE_NEVER,
      RenderMode::Always => RENDER_MODE_ALWAYS,
    })
    .unwrap_or(RENDER_MODE_AUTO)
}

/// Extract render DPI from OcrOptions when render feature is enabled.
#[cfg(all(feature = "ocr", feature = "render"))]
fn extract_render_dpi(opts: &Option<OcrOptions>) -> u32 {
  crate::core::render::normalize_dpi(opts.as_ref().and_then(|o| o.render_dpi))
}

/// Extract render mode as u8 — when render feature is off, always return Auto (0).
#[cfg(all(feature = "ocr", not(feature = "render")))]
fn extract_render_mode(_opts: &Option<OcrOptions>) -> u8 {
  crate::core::ocr::RENDER_MODE_AUTO
}

/// Extract render DPI — when render feature is off, return default 300.
#[cfg(all(feature = "ocr", not(feature = "render")))]
fn extract_render_dpi(_opts: &Option<OcrOptions>) -> u32 {
  300
}

/// Initialize PDFium from pdfium_path in OcrOptions (if present).
#[cfg(all(feature = "ocr", feature = "render"))]
fn maybe_init_pdfium(opts: &Option<OcrOptions>) {
  let path = opts.as_ref().and_then(|o| o.pdfium_path.as_deref());
  let _ = crate::core::render::ensure_pdfium_with_path(path);
}

#[cfg(all(feature = "ocr", not(feature = "render")))]
fn maybe_init_pdfium(_opts: &Option<OcrOptions>) {}

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
  maybe_init_pdfium(&opts);
  let doc = load_doc(buffer.as_ref())?;
  let lang = opts
    .as_ref()
    .and_then(|o| o.lang.as_deref())
    .unwrap_or("eng");
  let min_len = opts.as_ref().and_then(|o| o.min_text_length).unwrap_or(1);
  let max_threads = normalize_max_threads(opts.as_ref().and_then(|o| o.max_threads));
  let render_dpi = extract_render_dpi(&opts);
  let render_mode = extract_render_mode(&opts);
  extract_text_with_ocr(
    &doc,
    buffer.as_ref(),
    lang,
    min_len,
    max_threads,
    render_dpi,
    render_mode,
  )
}

#[cfg(feature = "ocr")]
#[napi]
pub fn pdf_document_ocr(buffer: Buffer, opts: Option<OcrOptions>) -> Result<PdfDocumentOcr> {
  maybe_init_pdfium(&opts);
  let doc = load_doc(buffer.as_ref())?;
  let lang = opts
    .as_ref()
    .and_then(|o| o.lang.as_deref())
    .unwrap_or("eng");
  let min_len = opts.as_ref().and_then(|o| o.min_text_length).unwrap_or(1);
  let max_threads = normalize_max_threads(opts.as_ref().and_then(|o| o.max_threads));
  let render_dpi = extract_render_dpi(&opts);
  let render_mode = extract_render_mode(&opts);
  Ok(PdfDocumentOcr::from(extract_all_with_ocr(
    &doc,
    buffer.as_ref(),
    lang,
    min_len,
    max_threads,
    render_dpi,
    render_mode,
  )?))
}

// ── Standalone render functions ─────────────────────────────────

#[cfg(feature = "render")]
pub struct RenderPagesTask {
  data: Vec<u8>,
  dpi: u32,
  mode: u8,
}

#[cfg(feature = "render")]
#[napi]
impl Task for RenderPagesTask {
  type Output = Vec<RawRenderedPage>;
  type JsValue = Vec<RenderedPage>;

  fn compute(&mut self) -> Result<Self::Output> {
    use crate::core::render::{RENDER_MODE_AUTO, RENDER_MODE_NEVER};

    let dpi = self.dpi;
    let mode = self.mode;
    let pdf_bytes = &self.data;

    if mode == RENDER_MODE_NEVER {
      return Ok(Vec::new());
    }

    crate::core::render::ensure_pdfium_with_path(None)
      .map_err(|e| Error::from_reason(e.to_string()))?;

    let doc = load_doc(pdf_bytes)?;
    let pages = doc.get_pages();
    let page_count = pages.len() as u16;

    let indices: Vec<u16> = if mode == RENDER_MODE_AUTO {
      pages
        .iter()
        .filter_map(|(&page_num, &page_id)| {
          let raw = doc.extract_text(&[page_num]).unwrap_or_default();
          let non_ws: usize = raw.chars().filter(|c| !c.is_whitespace()).count();
          if non_ws > 0 {
            return None;
          }
          let has_images =
            !crate::core::images::collect_page_decoded_images(&doc, page_id).is_empty();
          if has_images {
            return None;
          }
          Some((page_num - 1) as u16)
        })
        .collect()
    } else {
      (0..page_count).collect()
    };

    let rendered = crate::core::render::render_pages_to_png(pdf_bytes, &indices, dpi);
    Ok(
      rendered
        .into_iter()
        .map(|(idx, w, h, data)| RawRenderedPage {
          page: (idx as u32) + 1,
          width: w,
          height: h,
          dpi,
          data,
        })
        .collect(),
    )
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into_iter().map(RenderedPage::from).collect())
  }
}

#[cfg(feature = "render")]
#[napi]
pub fn render_pages_async(
  buffer: Buffer,
  opts: Option<RenderOptions>,
) -> AsyncTask<RenderPagesTask> {
  use crate::core::render::{RENDER_MODE_ALWAYS, RENDER_MODE_AUTO, RENDER_MODE_NEVER};
  let dpi = crate::core::render::normalize_dpi(opts.as_ref().and_then(|o| o.dpi));
  let mode = opts
    .as_ref()
    .and_then(|o| o.mode.as_ref())
    .map(|m| match m {
      RenderMode::Auto => RENDER_MODE_AUTO,
      RenderMode::Never => RENDER_MODE_NEVER,
      RenderMode::Always => RENDER_MODE_ALWAYS,
    })
    .unwrap_or(RENDER_MODE_ALWAYS);
  AsyncTask::new(RenderPagesTask {
    data: buffer.to_vec(),
    dpi,
    mode,
  })
}

// ── Capabilities check ──────────────────────────────────────────

#[napi]
pub fn capabilities() -> Capabilities {
  Capabilities {
    ocr: cfg!(feature = "ocr"),
    #[cfg(feature = "render")]
    render: crate::core::render::is_pdfium_available(),
    #[cfg(not(feature = "render"))]
    render: false,
  }
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
  render_dpi: u32,
  render_mode: u8,
}

#[cfg(feature = "ocr")]
#[napi]
impl Task for ExtractTextOcrTask {
  type Output = Vec<OcrPageText>;
  type JsValue = Vec<OcrPageText>;

  fn compute(&mut self) -> Result<Self::Output> {
    let doc = load_doc(&self.data)?;
    extract_text_with_ocr(
      &doc,
      &self.data,
      &self.lang,
      self.min_len,
      self.max_threads,
      self.render_dpi,
      self.render_mode,
    )
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
  maybe_init_pdfium(&opts);
  let lang = opts
    .as_ref()
    .and_then(|o| o.lang.clone())
    .unwrap_or_else(|| "eng".to_string());
  let min_len = opts.as_ref().and_then(|o| o.min_text_length).unwrap_or(1);
  let max_threads = normalize_max_threads(opts.as_ref().and_then(|o| o.max_threads));
  let render_dpi = extract_render_dpi(&opts);
  let render_mode = extract_render_mode(&opts);
  AsyncTask::new(ExtractTextOcrTask {
    data: buffer.to_vec(),
    lang,
    min_len,
    max_threads,
    render_dpi,
    render_mode,
  })
}

#[cfg(feature = "ocr")]
pub struct PdfDocumentOcrTask {
  data: Vec<u8>,
  lang: String,
  min_len: u32,
  max_threads: u32,
  render_dpi: u32,
  render_mode: u8,
}

#[cfg(feature = "ocr")]
#[napi]
impl Task for PdfDocumentOcrTask {
  type Output = RawPdfDocumentOcr;
  type JsValue = PdfDocumentOcr;

  fn compute(&mut self) -> Result<Self::Output> {
    let doc = load_doc(&self.data)?;
    extract_all_with_ocr(
      &doc,
      &self.data,
      &self.lang,
      self.min_len,
      self.max_threads,
      self.render_dpi,
      self.render_mode,
    )
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
  maybe_init_pdfium(&opts);
  let lang = opts
    .as_ref()
    .and_then(|o| o.lang.clone())
    .unwrap_or_else(|| "eng".to_string());
  let min_len = opts.as_ref().and_then(|o| o.min_text_length).unwrap_or(1);
  let max_threads = normalize_max_threads(opts.as_ref().and_then(|o| o.max_threads));
  let render_dpi = extract_render_dpi(&opts);
  let render_mode = extract_render_mode(&opts);
  AsyncTask::new(PdfDocumentOcrTask {
    data: buffer.to_vec(),
    lang,
    min_len,
    max_threads,
    render_dpi,
    render_mode,
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
  raw: Arc<Vec<u8>>,
  lang: String,
  min_len: u32,
  max_threads: u32,
  render_dpi: u32,
  render_mode: u8,
}

#[cfg(feature = "ocr")]
#[napi]
impl Task for SharedExtractTextOcrTask {
  type Output = Vec<OcrPageText>;
  type JsValue = Vec<OcrPageText>;

  fn compute(&mut self) -> Result<Self::Output> {
    extract_text_with_ocr(
      &self.doc,
      &self.raw,
      &self.lang,
      self.min_len,
      self.max_threads,
      self.render_dpi,
      self.render_mode,
    )
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

#[cfg(feature = "ocr")]
pub struct SharedPdfDocumentOcrTask {
  doc: Arc<Document>,
  raw: Arc<Vec<u8>>,
  lang: String,
  min_len: u32,
  max_threads: u32,
  render_dpi: u32,
  render_mode: u8,
}

#[cfg(feature = "ocr")]
#[napi]
impl Task for SharedPdfDocumentOcrTask {
  type Output = RawPdfDocumentOcr;
  type JsValue = PdfDocumentOcr;

  fn compute(&mut self) -> Result<Self::Output> {
    extract_all_with_ocr(
      &self.doc,
      &self.raw,
      &self.lang,
      self.min_len,
      self.max_threads,
      self.render_dpi,
      self.render_mode,
    )
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(PdfDocumentOcr::from(output))
  }
}

#[cfg(feature = "render")]
pub struct SharedRenderPagesTask {
  raw: Arc<Vec<u8>>,
  dpi: u32,
  mode: u8,
}

#[cfg(feature = "render")]
#[napi]
impl Task for SharedRenderPagesTask {
  type Output = Vec<RawRenderedPage>;
  type JsValue = Vec<RenderedPage>;

  fn compute(&mut self) -> Result<Self::Output> {
    use crate::core::render::{RENDER_MODE_AUTO, RENDER_MODE_NEVER};

    let dpi = self.dpi;
    let mode = self.mode;
    let pdf_bytes = &self.raw;

    if mode == RENDER_MODE_NEVER {
      return Ok(Vec::new());
    }

    crate::core::render::ensure_pdfium_with_path(None)
      .map_err(|e| Error::from_reason(e.to_string()))?;

    let doc = load_doc(pdf_bytes)?;
    let pages = doc.get_pages();
    let page_count = pages.len() as u16;

    let indices: Vec<u16> = if mode == RENDER_MODE_AUTO {
      pages
        .iter()
        .filter_map(|(&page_num, &page_id)| {
          let raw = doc.extract_text(&[page_num]).unwrap_or_default();
          let non_ws: usize = raw.chars().filter(|c| !c.is_whitespace()).count();
          if non_ws > 0 {
            return None;
          }
          let has_images =
            !crate::core::images::collect_page_decoded_images(&doc, page_id).is_empty();
          if has_images {
            return None;
          }
          Some((page_num - 1) as u16)
        })
        .collect()
    } else {
      (0..page_count).collect()
    };

    let rendered = crate::core::render::render_pages_to_png(pdf_bytes, &indices, dpi);
    Ok(
      rendered
        .into_iter()
        .map(|(idx, w, h, data)| RawRenderedPage {
          page: (idx as u32) + 1,
          width: w,
          height: h,
          dpi,
          data,
        })
        .collect(),
    )
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into_iter().map(RenderedPage::from).collect())
  }
}

#[napi]
pub struct PdfDown {
  doc: Arc<Document>,
  #[allow(dead_code)] // used when ocr or render features are enabled
  raw: Arc<Vec<u8>>,
}

#[napi]
impl PdfDown {
  #[napi(constructor)]
  pub fn new(buffer: Buffer) -> Result<Self> {
    let bytes = buffer.to_vec();
    let doc = Document::load_mem(&bytes)
      .map_err(|e| Error::from_reason(format!("Failed to load PDF: {e}")))?;
    Ok(PdfDown {
      doc: Arc::new(doc),
      raw: Arc::new(bytes),
    })
  }

  #[napi]
  pub fn text_per_page(&self) -> Result<Vec<PageText>> {
    extract_text(&self.doc)
  }

  #[napi]
  pub fn images_per_page(&self) -> Result<Vec<PageImage>> {
    Ok(
      extract_images_raw(&self.doc)
        .into_iter()
        .map(PageImage::from)
        .collect(),
    )
  }

  #[napi]
  pub fn annotations_per_page(&self) -> Vec<PageAnnotation> {
    extract_annotations(&self.doc)
  }

  #[napi]
  pub fn metadata(&self) -> PdfMeta {
    extract_metadata(&self.doc)
  }

  #[napi]
  pub fn text_per_page_async(&self) -> AsyncTask<SharedExtractTextTask> {
    AsyncTask::new(SharedExtractTextTask(Arc::clone(&self.doc)))
  }

  #[napi]
  pub fn images_per_page_async(&self) -> AsyncTask<SharedExtractImagesTask> {
    AsyncTask::new(SharedExtractImagesTask(Arc::clone(&self.doc)))
  }

  #[napi]
  pub fn annotations_per_page_async(&self) -> AsyncTask<SharedExtractAnnotationsTask> {
    AsyncTask::new(SharedExtractAnnotationsTask(Arc::clone(&self.doc)))
  }

  #[napi]
  pub fn metadata_async(&self) -> AsyncTask<SharedPdfMetaTask> {
    AsyncTask::new(SharedPdfMetaTask(Arc::clone(&self.doc)))
  }

  #[napi]
  pub fn document(&self) -> Result<PdfDocument> {
    Ok(PdfDocument::from(extract_all(&self.doc)?))
  }

  #[napi]
  pub fn document_async(&self) -> AsyncTask<SharedPdfDocumentTask> {
    AsyncTask::new(SharedPdfDocumentTask(Arc::clone(&self.doc)))
  }

  #[napi]
  pub fn structured_text(&self) -> Result<Vec<StructuredPageText>> {
    extract_structured_text(&self.doc)
  }

  #[napi]
  pub fn structured_text_async(&self) -> AsyncTask<SharedStructuredTextTask> {
    AsyncTask::new(SharedStructuredTextTask(Arc::clone(&self.doc)))
  }
}

#[cfg(feature = "ocr")]
#[napi]
impl PdfDown {
  #[napi]
  pub fn text_with_ocr_per_page(&self, opts: Option<OcrOptions>) -> Result<Vec<OcrPageText>> {
    maybe_init_pdfium(&opts);
    let lang = opts
      .as_ref()
      .and_then(|o| o.lang.as_deref())
      .unwrap_or("eng");
    let min_len = opts.as_ref().and_then(|o| o.min_text_length).unwrap_or(1);
    let max_threads = normalize_max_threads(opts.as_ref().and_then(|o| o.max_threads));
    let render_dpi = extract_render_dpi(&opts);
    let render_mode = extract_render_mode(&opts);
    extract_text_with_ocr(
      &self.doc,
      &self.raw,
      lang,
      min_len,
      max_threads,
      render_dpi,
      render_mode,
    )
  }

  #[napi]
  pub fn text_with_ocr_per_page_async(
    &self,
    opts: Option<OcrOptions>,
  ) -> AsyncTask<SharedExtractTextOcrTask> {
    maybe_init_pdfium(&opts);
    let lang = opts
      .as_ref()
      .and_then(|o| o.lang.clone())
      .unwrap_or_else(|| "eng".to_string());
    let min_len = opts.as_ref().and_then(|o| o.min_text_length).unwrap_or(1);
    let max_threads = normalize_max_threads(opts.as_ref().and_then(|o| o.max_threads));
    let render_dpi = extract_render_dpi(&opts);
    let render_mode = extract_render_mode(&opts);
    AsyncTask::new(SharedExtractTextOcrTask {
      doc: Arc::clone(&self.doc),
      raw: Arc::clone(&self.raw),
      lang,
      min_len,
      max_threads,
      render_dpi,
      render_mode,
    })
  }

  #[napi]
  pub fn document_ocr(&self, opts: Option<OcrOptions>) -> Result<PdfDocumentOcr> {
    maybe_init_pdfium(&opts);
    let lang = opts
      .as_ref()
      .and_then(|o| o.lang.as_deref())
      .unwrap_or("eng");
    let min_len = opts.as_ref().and_then(|o| o.min_text_length).unwrap_or(1);
    let max_threads = normalize_max_threads(opts.as_ref().and_then(|o| o.max_threads));
    let render_dpi = extract_render_dpi(&opts);
    let render_mode = extract_render_mode(&opts);
    Ok(PdfDocumentOcr::from(extract_all_with_ocr(
      &self.doc,
      &self.raw,
      lang,
      min_len,
      max_threads,
      render_dpi,
      render_mode,
    )?))
  }

  #[napi]
  pub fn document_ocr_async(
    &self,
    opts: Option<OcrOptions>,
  ) -> AsyncTask<SharedPdfDocumentOcrTask> {
    maybe_init_pdfium(&opts);
    let lang = opts
      .as_ref()
      .and_then(|o| o.lang.clone())
      .unwrap_or_else(|| "eng".to_string());
    let min_len = opts.as_ref().and_then(|o| o.min_text_length).unwrap_or(1);
    let max_threads = normalize_max_threads(opts.as_ref().and_then(|o| o.max_threads));
    let render_dpi = extract_render_dpi(&opts);
    let render_mode = extract_render_mode(&opts);
    AsyncTask::new(SharedPdfDocumentOcrTask {
      doc: Arc::clone(&self.doc),
      raw: Arc::clone(&self.raw),
      lang,
      min_len,
      max_threads,
      render_dpi,
      render_mode,
    })
  }
}

#[cfg(feature = "render")]
#[napi]
impl PdfDown {
  #[napi]
  pub fn render_pages_async(
    &self,
    opts: Option<RenderOptions>,
  ) -> AsyncTask<SharedRenderPagesTask> {
    use crate::core::render::{RENDER_MODE_ALWAYS, RENDER_MODE_AUTO, RENDER_MODE_NEVER};
    let dpi = crate::core::render::normalize_dpi(opts.as_ref().and_then(|o| o.dpi));
    let mode = opts
      .as_ref()
      .and_then(|o| o.mode.as_ref())
      .map(|m| match m {
        RenderMode::Auto => RENDER_MODE_AUTO,
        RenderMode::Never => RENDER_MODE_NEVER,
        RenderMode::Always => RENDER_MODE_ALWAYS,
      })
      .unwrap_or(RENDER_MODE_ALWAYS);
    AsyncTask::new(SharedRenderPagesTask {
      raw: Arc::clone(&self.raw),
      dpi,
      mode,
    })
  }
}
