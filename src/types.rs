use napi::bindgen_prelude::Buffer;
use napi_derive::napi;

// ── Napi object types (JS boundary) ─────────────────────────────

#[napi(object)]
pub struct PageText {
  pub page: u32,
  pub text: String,
}

#[napi(object)]
pub struct StructuredPageText {
  pub page: u32,
  pub header: String,
  pub body: String,
  pub footer: String,
}

#[cfg(feature = "ocr")]
#[napi(string_enum)]
pub enum TextSource {
  Native,
  Ocr,
}

#[cfg(feature = "ocr")]
#[napi(object)]
pub struct OcrPageText {
  pub page: u32,
  pub text: String,
  pub source: TextSource,
}

#[cfg(feature = "ocr")]
#[napi(object)]
pub struct OcrOptions {
  pub lang: Option<String>,
  pub min_text_length: Option<u32>,
  pub max_threads: Option<u32>,
}

#[napi(object)]
pub struct PdfMeta {
  pub page_count: u32,
  pub version: String,
  pub is_linearized: bool,
  pub creator: Option<String>,
  pub producer: Option<String>,
  pub creation_date: Option<String>,
  pub modification_date: Option<String>,
  pub page_boxes: Vec<PageBox>,
}

#[napi(string_enum)]
pub enum BoxType {
  CropBox,
  MediaBox,
  Unknown,
}

#[napi(object)]
pub struct PageBox {
  /// Number of pages that share these dimensions.
  pub page_count: u32,
  pub left: f64,
  pub bottom: f64,
  pub right: f64,
  pub top: f64,
  pub width: f64,
  pub height: f64,
  pub box_type: BoxType,
  /// Present only on non-dominant boxes — lists the specific pages with these
  /// dimensions. `None` on the first (most frequent) entry means "all pages
  /// not listed in any other entry's `pages` array."
  pub pages: Option<Vec<u32>>,
}

#[napi(object)]
pub struct PageImage {
  pub page: u32,
  pub image_index: u32,
  pub width: u32,
  pub height: u32,
  pub data: Buffer,
  pub color_space: String,
  pub bits_per_component: u32,
  pub filter: String,
  pub xobject_name: String,
  pub object_id: String,
}

#[napi(object)]
pub struct PageAnnotation {
  pub page: u32,
  pub subtype: String,
  pub rect: Vec<f64>,
  pub uri: Option<String>,
  pub dest: Option<String>,
  pub content: Option<String>,
}

#[napi(object)]
pub struct PdfDocument {
  pub version: String,
  pub is_linearized: bool,
  pub page_count: u32,
  pub creator: Option<String>,
  pub producer: Option<String>,
  pub creation_date: Option<String>,
  pub modification_date: Option<String>,
  pub page_boxes: Vec<PageBox>,
  pub total_images: u32,
  pub total_annotations: u32,
  pub image_pages: Vec<u32>,
  pub annotation_pages: Vec<u32>,
  pub text: Vec<PageText>,
  pub structured_text: Vec<StructuredPageText>,
  pub images: Vec<PageImage>,
  pub annotations: Vec<PageAnnotation>,
}

#[cfg(feature = "ocr")]
#[napi(object)]
pub struct OcrStructuredPageText {
  pub page: u32,
  pub header: String,
  pub body: String,
  pub footer: String,
  pub source: TextSource,
}

#[cfg(feature = "ocr")]
#[napi(object)]
pub struct PdfDocumentOcr {
  pub version: String,
  pub is_linearized: bool,
  pub page_count: u32,
  pub creator: Option<String>,
  pub producer: Option<String>,
  pub creation_date: Option<String>,
  pub modification_date: Option<String>,
  pub page_boxes: Vec<PageBox>,
  pub total_images: u32,
  pub total_annotations: u32,
  pub image_pages: Vec<u32>,
  pub annotation_pages: Vec<u32>,
  pub text: Vec<OcrPageText>,
  pub structured_text: Vec<OcrStructuredPageText>,
  pub images: Vec<PageImage>,
  pub annotations: Vec<PageAnnotation>,
}

// ── Internal type — no napi types, safe for any thread ──

pub struct RawPageImage {
  pub page: u32,
  pub image_index: u32,
  pub width: u32,
  pub height: u32,
  pub data: Vec<u8>,
  pub color_space: String,
  pub bits_per_component: u32,
  pub filter: String,
  pub xobject_name: String,
  pub object_id: String,
}

impl From<RawPageImage> for PageImage {
  fn from(r: RawPageImage) -> Self {
    PageImage {
      page: r.page,
      image_index: r.image_index,
      width: r.width,
      height: r.height,
      data: r.data.into(),
      color_space: r.color_space,
      bits_per_component: r.bits_per_component,
      filter: r.filter,
      xobject_name: r.xobject_name,
      object_id: r.object_id,
    }
  }
}

pub struct RawPdfDocument {
  pub meta: PdfMeta,
  pub text: Vec<PageText>,
  pub structured_text: Vec<StructuredPageText>,
  pub images: Vec<RawPageImage>,
  pub annotations: Vec<PageAnnotation>,
  pub image_pages: Vec<u32>,
  pub annotation_pages: Vec<u32>,
}

impl From<RawPdfDocument> for PdfDocument {
  fn from(r: RawPdfDocument) -> Self {
    let total_images = r.images.len() as u32;
    let total_annotations = r.annotations.len() as u32;
    PdfDocument {
      version: r.meta.version,
      is_linearized: r.meta.is_linearized,
      page_count: r.meta.page_count,
      creator: r.meta.creator,
      producer: r.meta.producer,
      creation_date: r.meta.creation_date,
      modification_date: r.meta.modification_date,
      page_boxes: r.meta.page_boxes,
      total_images,
      total_annotations,
      image_pages: r.image_pages,
      annotation_pages: r.annotation_pages,
      text: r.text,
      structured_text: r.structured_text,
      images: r.images.into_iter().map(PageImage::from).collect(),
      annotations: r.annotations,
    }
  }
}

#[cfg(feature = "ocr")]
pub struct RawPdfDocumentOcr {
  pub meta: PdfMeta,
  pub text: Vec<OcrPageText>,
  pub structured_text: Vec<OcrStructuredPageText>,
  pub images: Vec<RawPageImage>,
  pub annotations: Vec<PageAnnotation>,
  pub image_pages: Vec<u32>,
  pub annotation_pages: Vec<u32>,
}

#[cfg(feature = "ocr")]
impl From<RawPdfDocumentOcr> for PdfDocumentOcr {
  fn from(r: RawPdfDocumentOcr) -> Self {
    let total_images = r.images.len() as u32;
    let total_annotations = r.annotations.len() as u32;
    PdfDocumentOcr {
      version: r.meta.version,
      is_linearized: r.meta.is_linearized,
      page_count: r.meta.page_count,
      creator: r.meta.creator,
      producer: r.meta.producer,
      creation_date: r.meta.creation_date,
      modification_date: r.meta.modification_date,
      page_boxes: r.meta.page_boxes,
      total_images,
      total_annotations,
      image_pages: r.image_pages,
      annotation_pages: r.annotation_pages,
      text: r.text,
      structured_text: r.structured_text,
      images: r.images.into_iter().map(PageImage::from).collect(),
      annotations: r.annotations,
    }
  }
}
