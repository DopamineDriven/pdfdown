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
  AddressField, AircraftReference, Attachment, BoxType, Capabilities, ChoiceOption, DateField,
  DateOrder, EInvoiceStandard, EmbeddedInvoiceXml, FieldSpan, FormInfo, InvoiceDocumentType,
  InvoiceFields, InvoiceItem, InvoiceOptions, InvoiceResult, InvoiceSource, MoneyField,
  NumberField, PageAnnotation, PageBox, PageFormField, PageImage, PageLayout, PageText,
  PaymentDetail, PdfDocument, PdfMeta, PlacedImage, StringField, StructuredPageText, TaxDetail,
  TextLine, TextSource, TextWord, UnplacedFormField,
};

#[cfg(feature = "ocr")]
pub use types::{OcrOptions, OcrPageText, OcrStructuredPageText, PdfDocumentOcr};

#[cfg(feature = "render")]
pub use types::{RawRenderedPage, RenderMode, RenderOptions, RenderedPage};

// Internal plumbing (used by Task impls in this file — must be `pub` for napi Task trait)
pub use types::{RawAttachment, RawPageImage, RawPdfDocument};

#[cfg(feature = "ocr")]
pub use types::RawPdfDocumentOcr;

// ── Step 1: Compile-time assertion that lopdf::Document is Send + Sync ──
const _: () = {
  fn assert_send_sync<T: Send + Sync>() {}
  // Referenced rather than called: monomorphizing the fn checks the bound at compile time
  // without leaving an uncalled function behind for dead-code analysis.
  let _ = assert_send_sync::<lopdf::Document>;
};

// ── Shared helpers ──────────────────────────────────────────────

use crate::core::attachments::extract_attachments;
use crate::core::document::{extract_all, extract_annotations};
use crate::core::forms::extract_forms;
use crate::core::images::extract_images_raw;
use crate::core::invoice::extract_invoice as extract_invoice_impl;
use crate::core::layout::extract_layout;
use crate::core::meta::extract_metadata;
use crate::core::text::{extract_structured_text, extract_text};

#[cfg(feature = "ocr")]
use crate::core::document::extract_all_with_ocr;
#[cfg(feature = "ocr")]
use crate::core::ocr::{extract_layout_with_ocr, extract_text_with_ocr, normalize_max_threads};

use crate::core::spreadsheet::package::{NOT_PDF_OR_ZIP, SpreadsheetError, detect};
use crate::types::{DocumentFormat, FormatDetection};

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

/// Resolved OCR settings shared by the positioned-text entry points.
#[cfg(feature = "ocr")]
struct OcrParams {
  lang: String,
  min_len: u32,
  max_threads: u32,
  render_dpi: u32,
  render_mode: u8,
}

#[cfg(feature = "ocr")]
fn ocr_params(opts: &Option<OcrOptions>) -> OcrParams {
  maybe_init_pdfium(opts);
  OcrParams {
    lang: opts
      .as_ref()
      .and_then(|o| o.lang.clone())
      .unwrap_or_else(|| "eng".to_string()),
    min_len: opts.as_ref().and_then(|o| o.min_text_length).unwrap_or(1),
    max_threads: normalize_max_threads(opts.as_ref().and_then(|o| o.max_threads)),
    render_dpi: extract_render_dpi(opts),
    render_mode: extract_render_mode(opts),
  }
}

// ── Standalone sync functions ───────────────────────────────────

#[napi]
pub fn extract_text_per_page(buffer: Buffer) -> Result<Vec<PageText>> {
  let doc = load_doc(buffer.as_ref())?;
  extract_text(&doc)
}

/// `Pdf`, `Xlsx`, or `Unknown` with what the buffer was instead.
#[napi]
pub fn detect_format(buffer: Buffer) -> FormatDetection {
  detect(&buffer)
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

/// Positioned words and lines per page from the content stream (native text only).
/// Coordinates are PDF user space in points, y up.
#[napi]
pub fn layout_per_page(buffer: Buffer) -> Result<Vec<PageLayout>> {
  let doc = load_doc(buffer.as_ref())?;
  Ok(extract_layout(&doc))
}

/// Every embedded file: `/Names /EmbeddedFiles`, catalog `/AF`, and page `/FileAttachment` annotations.
#[napi]
pub fn attachments(buffer: Buffer) -> Result<Vec<Attachment>> {
  let doc = load_doc(buffer.as_ref())?;
  Ok(
    extract_attachments(&doc)
      .into_iter()
      .map(Attachment::from)
      .collect(),
  )
}

/// Structured invoice data. This version reads embedded Factur-X / ZUGFeRD /
/// XRechnung / UBL XML; documents without it return `source: "None"`.
#[napi]
pub fn extract_invoice(buffer: Buffer, opts: Option<InvoiceOptions>) -> Result<InvoiceResult> {
  let doc = load_doc(buffer.as_ref())?;
  Ok(extract_invoice_impl(&doc, opts.as_ref()))
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

#[napi]
pub fn extract_form_fields_per_page(buffer: Buffer) -> Result<Vec<PageFormField>> {
  let doc = load_doc(buffer.as_ref())?;
  Ok(extract_forms(&doc).fields)
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

/// Positioned words and lines per page with the OCR cascade: native glyph
/// placement, then OCR of embedded images mapped through their placements,
/// then (with PDFium) OCR of the rasterised page.
#[cfg(feature = "ocr")]
#[napi]
pub fn extract_layout_with_ocr_per_page(
  buffer: Buffer,
  opts: Option<OcrOptions>,
) -> Result<Vec<PageLayout>> {
  let p = ocr_params(&opts);
  let doc = load_doc(buffer.as_ref())?;
  extract_layout_with_ocr(
    &doc,
    buffer.as_ref(),
    &p.lang,
    p.min_len,
    p.max_threads,
    p.render_dpi,
    p.render_mode,
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

pub struct ExtractLayoutTask(Vec<u8>);

#[napi]
impl Task for ExtractLayoutTask {
  type Output = Vec<PageLayout>;
  type JsValue = Vec<PageLayout>;

  fn compute(&mut self) -> Result<Self::Output> {
    let doc = load_doc(&self.0)?;
    Ok(extract_layout(&doc))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

#[napi]
pub fn layout_per_page_async(buffer: Buffer) -> AsyncTask<ExtractLayoutTask> {
  AsyncTask::new(ExtractLayoutTask(buffer.to_vec()))
}

pub struct AttachmentsTask(Vec<u8>);

#[napi]
impl Task for AttachmentsTask {
  type Output = Vec<RawAttachment>;
  type JsValue = Vec<Attachment>;

  fn compute(&mut self) -> Result<Self::Output> {
    let doc = load_doc(&self.0)?;
    Ok(extract_attachments(&doc))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into_iter().map(Attachment::from).collect())
  }
}

#[napi]
pub fn attachments_async(buffer: Buffer) -> AsyncTask<AttachmentsTask> {
  AsyncTask::new(AttachmentsTask(buffer.to_vec()))
}

pub struct ExtractInvoiceTask {
  data: Vec<u8>,
  opts: Option<InvoiceOptions>,
}

#[napi]
impl Task for ExtractInvoiceTask {
  type Output = InvoiceResult;
  type JsValue = InvoiceResult;

  fn compute(&mut self) -> Result<Self::Output> {
    let doc = load_doc(&self.data)?;
    Ok(extract_invoice_impl(&doc, self.opts.as_ref()))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

#[napi]
pub fn extract_invoice_async(
  buffer: Buffer,
  opts: Option<InvoiceOptions>,
) -> AsyncTask<ExtractInvoiceTask> {
  AsyncTask::new(ExtractInvoiceTask {
    data: buffer.to_vec(),
    opts,
  })
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

pub struct ExtractFormFieldsTask(Vec<u8>);

#[napi]
impl Task for ExtractFormFieldsTask {
  type Output = Vec<PageFormField>;
  type JsValue = Vec<PageFormField>;

  fn compute(&mut self) -> Result<Self::Output> {
    let doc = load_doc(&self.0)?;
    Ok(extract_forms(&doc).fields)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

#[napi]
pub fn extract_form_fields_per_page_async(buffer: Buffer) -> AsyncTask<ExtractFormFieldsTask> {
  AsyncTask::new(ExtractFormFieldsTask(buffer.to_vec()))
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
pub struct ExtractLayoutOcrTask {
  data: Vec<u8>,
  params: OcrParams,
}

#[cfg(feature = "ocr")]
#[napi]
impl Task for ExtractLayoutOcrTask {
  type Output = Vec<PageLayout>;
  type JsValue = Vec<PageLayout>;

  fn compute(&mut self) -> Result<Self::Output> {
    let doc = load_doc(&self.data)?;
    let p = &self.params;
    extract_layout_with_ocr(
      &doc,
      &self.data,
      &p.lang,
      p.min_len,
      p.max_threads,
      p.render_dpi,
      p.render_mode,
    )
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

#[cfg(feature = "ocr")]
#[napi]
pub fn extract_layout_with_ocr_per_page_async(
  buffer: Buffer,
  opts: Option<OcrOptions>,
) -> AsyncTask<ExtractLayoutOcrTask> {
  AsyncTask::new(ExtractLayoutOcrTask {
    data: buffer.to_vec(),
    params: ocr_params(&opts),
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

pub struct SharedLayoutTask(Arc<Document>);

#[napi]
impl Task for SharedLayoutTask {
  type Output = Vec<PageLayout>;
  type JsValue = Vec<PageLayout>;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(extract_layout(&self.0))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

pub struct SharedAttachmentsTask(Arc<Document>);

#[napi]
impl Task for SharedAttachmentsTask {
  type Output = Vec<RawAttachment>;
  type JsValue = Vec<Attachment>;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(extract_attachments(&self.0))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into_iter().map(Attachment::from).collect())
  }
}

pub struct SharedInvoiceTask {
  doc: Arc<Document>,
  opts: Option<InvoiceOptions>,
}

#[napi]
impl Task for SharedInvoiceTask {
  type Output = InvoiceResult;
  type JsValue = InvoiceResult;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(extract_invoice_impl(&self.doc, self.opts.as_ref()))
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

pub struct SharedFormFieldsTask(Arc<Document>);

#[napi]
impl Task for SharedFormFieldsTask {
  type Output = Vec<PageFormField>;
  type JsValue = Vec<PageFormField>;

  fn compute(&mut self) -> Result<Self::Output> {
    Ok(extract_forms(&self.0).fields)
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
pub struct SharedLayoutOcrTask {
  doc: Arc<Document>,
  raw: Arc<Vec<u8>>,
  params: OcrParams,
}

#[cfg(feature = "ocr")]
#[napi]
impl Task for SharedLayoutOcrTask {
  type Output = Vec<PageLayout>;
  type JsValue = Vec<PageLayout>;

  fn compute(&mut self) -> Result<Self::Output> {
    let p = &self.params;
    extract_layout_with_ocr(
      &self.doc,
      &self.raw,
      &p.lang,
      p.min_len,
      p.max_threads,
      p.render_dpi,
      p.render_mode,
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

/// A parsed PDF: the document and the bytes it came from (the OCR and render tiers re-read them).
pub struct PdfState {
  doc: Arc<Document>,
  #[allow(dead_code)] // used when ocr or render features are enabled
  raw: Arc<Vec<u8>>,
}

/// What a `PdfDown` holds. A PDF today; the workbook variant lands with the workbook reader.
pub enum ParsedDocument {
  Pdf(PdfState),
}

const _: () = {
  fn assert_send_sync<T: Send + Sync>() {}
  let _ = assert_send_sync::<ParsedDocument>;
};

fn unsupported_operation(operation: &str, document: &str) -> Error {
  Error::from_reason(format!(
    "unsupported operation: {operation} reads a PDF; this document is {document}"
  ))
}

/// Detect the format, then parse. A buffer detection cannot place still goes to lopdf, which
/// accepts a PDF with junk before its header and reports its own error otherwise.
fn parse_document(bytes: Vec<u8>) -> Result<ParsedDocument> {
  let detection = detect(&bytes);
  match detection.format {
    DocumentFormat::Pdf => {}
    DocumentFormat::Xlsx => {
      return Err(
        SpreadsheetError::UnsupportedFormat("workbook reading is not available yet".to_string())
          .into(),
      );
    }
    DocumentFormat::Unknown => {
      let detail = detection.detail.unwrap_or_default();
      if detail != NOT_PDF_OR_ZIP {
        return Err(SpreadsheetError::UnsupportedFormat(detail).into());
      }
    }
  }
  let doc = load_doc(&bytes)?;
  Ok(ParsedDocument::Pdf(PdfState {
    doc: Arc::new(doc),
    raw: Arc::new(bytes),
  }))
}

pub struct OpenDocumentTask(Vec<u8>);

#[napi]
impl Task for OpenDocumentTask {
  type Output = ParsedDocument;
  type JsValue = PdfDown;

  fn compute(&mut self) -> Result<Self::Output> {
    parse_document(std::mem::take(&mut self.0))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(PdfDown {
      inner: Arc::new(output),
    })
  }
}

#[napi]
pub struct PdfDown {
  inner: Arc<ParsedDocument>,
}

#[napi]
impl PdfDown {
  #[napi(constructor)]
  pub fn new(buffer: Buffer) -> Result<Self> {
    Ok(PdfDown {
      inner: Arc::new(parse_document(buffer.to_vec())?),
    })
  }

  /// Open a PDF or a workbook off the JavaScript thread; resolves to a `PdfDown`.
  #[napi]
  pub fn open_async(buffer: Buffer) -> AsyncTask<OpenDocumentTask> {
    AsyncTask::new(OpenDocumentTask(buffer.to_vec()))
  }

  /// The format this document was opened as.
  #[napi(getter)]
  pub fn format(&self) -> DocumentFormat {
    match &*self.inner {
      ParsedDocument::Pdf(_) => DocumentFormat::Pdf,
    }
  }

  /// The PDF behind this document, or the unsupported-operation error naming `operation`.
  fn pdf(&self, operation: &str) -> Result<&PdfState> {
    match &*self.inner {
      ParsedDocument::Pdf(state) => Ok(state),
      #[allow(unreachable_patterns)]
      _ => Err(unsupported_operation(operation, "a workbook")),
    }
  }

  #[napi]
  pub fn text_per_page(&self) -> Result<Vec<PageText>> {
    extract_text(&self.pdf("textPerPage")?.doc)
  }

  #[napi]
  pub fn images_per_page(&self) -> Result<Vec<PageImage>> {
    Ok(
      extract_images_raw(&self.pdf("imagesPerPage")?.doc)
        .into_iter()
        .map(PageImage::from)
        .collect(),
    )
  }

  #[napi]
  pub fn annotations_per_page(&self) -> Result<Vec<PageAnnotation>> {
    Ok(extract_annotations(&self.pdf("annotationsPerPage")?.doc))
  }

  #[napi]
  pub fn metadata(&self) -> Result<PdfMeta> {
    Ok(extract_metadata(&self.pdf("metadata")?.doc))
  }

  #[napi]
  /// Positioned words and lines per page from the content stream (native text only).
  pub fn layout(&self) -> Result<Vec<PageLayout>> {
    Ok(extract_layout(&self.pdf("layout")?.doc))
  }

  #[napi]
  pub fn layout_async(&self) -> Result<AsyncTask<SharedLayoutTask>> {
    Ok(AsyncTask::new(SharedLayoutTask(Arc::clone(
      &self.pdf("layoutAsync")?.doc,
    ))))
  }

  #[napi]
  pub fn attachments(&self) -> Result<Vec<Attachment>> {
    Ok(
      extract_attachments(&self.pdf("attachments")?.doc)
        .into_iter()
        .map(Attachment::from)
        .collect(),
    )
  }

  #[napi]
  pub fn attachments_async(&self) -> Result<AsyncTask<SharedAttachmentsTask>> {
    Ok(AsyncTask::new(SharedAttachmentsTask(Arc::clone(
      &self.pdf("attachmentsAsync")?.doc,
    ))))
  }

  #[napi]
  pub fn invoice(&self, opts: Option<InvoiceOptions>) -> Result<InvoiceResult> {
    Ok(extract_invoice_impl(
      &self.pdf("invoice")?.doc,
      opts.as_ref(),
    ))
  }

  #[napi]
  pub fn invoice_async(
    &self,
    opts: Option<InvoiceOptions>,
  ) -> Result<AsyncTask<SharedInvoiceTask>> {
    Ok(AsyncTask::new(SharedInvoiceTask {
      doc: Arc::clone(&self.pdf("invoiceAsync")?.doc),
      opts,
    }))
  }

  #[napi]
  pub fn text_per_page_async(&self) -> Result<AsyncTask<SharedExtractTextTask>> {
    Ok(AsyncTask::new(SharedExtractTextTask(Arc::clone(
      &self.pdf("textPerPageAsync")?.doc,
    ))))
  }

  #[napi]
  pub fn images_per_page_async(&self) -> Result<AsyncTask<SharedExtractImagesTask>> {
    Ok(AsyncTask::new(SharedExtractImagesTask(Arc::clone(
      &self.pdf("imagesPerPageAsync")?.doc,
    ))))
  }

  #[napi]
  pub fn annotations_per_page_async(&self) -> Result<AsyncTask<SharedExtractAnnotationsTask>> {
    Ok(AsyncTask::new(SharedExtractAnnotationsTask(Arc::clone(
      &self.pdf("annotationsPerPageAsync")?.doc,
    ))))
  }

  #[napi]
  pub fn metadata_async(&self) -> Result<AsyncTask<SharedPdfMetaTask>> {
    Ok(AsyncTask::new(SharedPdfMetaTask(Arc::clone(
      &self.pdf("metadataAsync")?.doc,
    ))))
  }

  #[napi]
  pub fn document(&self) -> Result<PdfDocument> {
    Ok(PdfDocument::from(extract_all(&self.pdf("document")?.doc)?))
  }

  #[napi]
  pub fn document_async(&self) -> Result<AsyncTask<SharedPdfDocumentTask>> {
    Ok(AsyncTask::new(SharedPdfDocumentTask(Arc::clone(
      &self.pdf("documentAsync")?.doc,
    ))))
  }

  #[napi]
  pub fn structured_text(&self) -> Result<Vec<StructuredPageText>> {
    extract_structured_text(&self.pdf("structuredText")?.doc)
  }

  #[napi]
  pub fn structured_text_async(&self) -> Result<AsyncTask<SharedStructuredTextTask>> {
    Ok(AsyncTask::new(SharedStructuredTextTask(Arc::clone(
      &self.pdf("structuredTextAsync")?.doc,
    ))))
  }

  #[napi]
  pub fn form_fields_per_page(&self) -> Result<Vec<PageFormField>> {
    Ok(extract_forms(&self.pdf("formFieldsPerPage")?.doc).fields)
  }

  #[napi]
  pub fn form_fields_per_page_async(&self) -> Result<AsyncTask<SharedFormFieldsTask>> {
    Ok(AsyncTask::new(SharedFormFieldsTask(Arc::clone(
      &self.pdf("formFieldsPerPageAsync")?.doc,
    ))))
  }
}

#[cfg(feature = "ocr")]
#[napi]
impl PdfDown {
  #[napi]
  pub fn text_with_ocr_per_page(&self, opts: Option<OcrOptions>) -> Result<Vec<OcrPageText>> {
    let state = self.pdf("textWithOcrPerPage")?;
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
      &state.doc,
      &state.raw,
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
  ) -> Result<AsyncTask<SharedExtractTextOcrTask>> {
    let state = self.pdf("textWithOcrPerPageAsync")?;
    maybe_init_pdfium(&opts);
    let lang = opts
      .as_ref()
      .and_then(|o| o.lang.clone())
      .unwrap_or_else(|| "eng".to_string());
    let min_len = opts.as_ref().and_then(|o| o.min_text_length).unwrap_or(1);
    let max_threads = normalize_max_threads(opts.as_ref().and_then(|o| o.max_threads));
    let render_dpi = extract_render_dpi(&opts);
    let render_mode = extract_render_mode(&opts);
    Ok(AsyncTask::new(SharedExtractTextOcrTask {
      doc: Arc::clone(&state.doc),
      raw: Arc::clone(&state.raw),
      lang,
      min_len,
      max_threads,
      render_dpi,
      render_mode,
    }))
  }

  /// Positioned words and lines per page with the OCR cascade.
  #[napi]
  pub fn layout_with_ocr(&self, opts: Option<OcrOptions>) -> Result<Vec<PageLayout>> {
    let state = self.pdf("layoutWithOcr")?;
    let p = ocr_params(&opts);
    extract_layout_with_ocr(
      &state.doc,
      &state.raw,
      &p.lang,
      p.min_len,
      p.max_threads,
      p.render_dpi,
      p.render_mode,
    )
  }

  #[napi]
  pub fn layout_with_ocr_async(
    &self,
    opts: Option<OcrOptions>,
  ) -> Result<AsyncTask<SharedLayoutOcrTask>> {
    let state = self.pdf("layoutWithOcrAsync")?;
    Ok(AsyncTask::new(SharedLayoutOcrTask {
      doc: Arc::clone(&state.doc),
      raw: Arc::clone(&state.raw),
      params: ocr_params(&opts),
    }))
  }

  #[napi]
  pub fn document_ocr(&self, opts: Option<OcrOptions>) -> Result<PdfDocumentOcr> {
    let state = self.pdf("documentOcr")?;
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
      &state.doc,
      &state.raw,
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
  ) -> Result<AsyncTask<SharedPdfDocumentOcrTask>> {
    let state = self.pdf("documentOcrAsync")?;
    maybe_init_pdfium(&opts);
    let lang = opts
      .as_ref()
      .and_then(|o| o.lang.clone())
      .unwrap_or_else(|| "eng".to_string());
    let min_len = opts.as_ref().and_then(|o| o.min_text_length).unwrap_or(1);
    let max_threads = normalize_max_threads(opts.as_ref().and_then(|o| o.max_threads));
    let render_dpi = extract_render_dpi(&opts);
    let render_mode = extract_render_mode(&opts);
    Ok(AsyncTask::new(SharedPdfDocumentOcrTask {
      doc: Arc::clone(&state.doc),
      raw: Arc::clone(&state.raw),
      lang,
      min_len,
      max_threads,
      render_dpi,
      render_mode,
    }))
  }
}

#[cfg(feature = "render")]
#[napi]
impl PdfDown {
  #[napi]
  pub fn render_pages_async(
    &self,
    opts: Option<RenderOptions>,
  ) -> Result<AsyncTask<SharedRenderPagesTask>> {
    let state = self.pdf("renderPagesAsync")?;
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
    Ok(AsyncTask::new(SharedRenderPagesTask {
      raw: Arc::clone(&state.raw),
      dpi,
      mode,
    }))
  }
}
