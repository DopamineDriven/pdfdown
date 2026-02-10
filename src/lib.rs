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
  BoxType, PageAnnotation, PageBox, PageImage, PageText, PdfDocument, PdfMeta, StructuredPageText,
};

#[cfg(feature = "ocr")]
pub use types::{OcrOptions, OcrPageText, OcrStructuredPageText, PdfDocumentOcr, TextSource};

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
