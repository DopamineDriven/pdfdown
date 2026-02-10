use crate::core::images::extract_images_raw;
use crate::core::meta::extract_metadata;
use crate::core::text::{detect_headers_footers, extract_text};
use crate::types::{PageAnnotation, RawPdfDocument};
use lopdf::{Document, Object, ObjectId};
use napi::Result;
use rayon::prelude::*;
use std::collections::HashSet;

#[cfg(feature = "ocr")]
use crate::core::ocr::{detect_headers_footers_ocr, extract_text_with_ocr};
#[cfg(feature = "ocr")]
use crate::types::RawPdfDocumentOcr;

fn collect_page_annotations(
  doc: &Document,
  page_id: ObjectId,
  page_num: u32,
) -> Vec<PageAnnotation> {
  let annots = match doc.get_page_annotations(page_id) {
    Ok(a) => a,
    Err(_) => return Vec::new(),
  };

  let mut results = Vec::new();
  for annot in annots {
    let subtype = annot
      .get(b"Subtype")
      .ok()
      .and_then(|v| {
        if let Object::Name(n) = v {
          Some(String::from_utf8_lossy(n).to_string())
        } else {
          None
        }
      })
      .unwrap_or_default();

    let rect = annot
      .get(b"Rect")
      .ok()
      .and_then(|v| {
        if let Object::Array(arr) = v {
          Some(
            arr
              .iter()
              .filter_map(|o| match o {
                Object::Real(f) => Some(*f as f64),
                Object::Integer(i) => Some(*i as f64),
                _ => None,
              })
              .collect::<Vec<f64>>(),
          )
        } else {
          None
        }
      })
      .unwrap_or_default();

    // Extract URI from /A action dictionary
    let uri = annot.get(b"A").ok().and_then(|action| {
      let action_dict = match action {
        Object::Dictionary(d) => Some(d),
        Object::Reference(id) => doc.get_dictionary(*id).ok(),
        _ => None,
      }?;
      let uri_obj = action_dict.get(b"URI").ok()?;
      match uri_obj {
        Object::String(bytes, _) => Some(String::from_utf8_lossy(bytes).to_string()),
        _ => None,
      }
    });

    // Extract /Dest (named or direct destination)
    let dest = annot.get(b"Dest").ok().and_then(|d| match d {
      Object::String(bytes, _) => Some(String::from_utf8_lossy(bytes).to_string()),
      Object::Name(n) => Some(String::from_utf8_lossy(n).to_string()),
      _ => None,
    });

    // Extract /Contents (tooltip / alt text)
    let content = annot.get(b"Contents").ok().and_then(|c| match c {
      Object::String(bytes, _) => Some(String::from_utf8_lossy(bytes).to_string()),
      _ => None,
    });

    results.push(PageAnnotation {
      page: page_num,
      subtype,
      rect,
      uri,
      dest,
      content,
    });
  }

  results
}

pub(crate) fn extract_annotations(doc: &Document) -> Vec<PageAnnotation> {
  let pages = doc.get_pages();
  let page_entries: Vec<(u32, ObjectId)> = pages.iter().map(|(&k, &v)| (k, v)).collect();
  let mut results: Vec<PageAnnotation> = page_entries
    .par_iter()
    .flat_map(|&(page_num, page_id)| collect_page_annotations(doc, page_id, page_num))
    .collect();
  results.sort_unstable_by_key(|a| a.page);
  results
}

pub(crate) fn extract_all(doc: &Document) -> Result<RawPdfDocument> {
  let meta = extract_metadata(doc);
  let ((text, images), annotations) = rayon::join(
    || rayon::join(|| extract_text(doc), || extract_images_raw(doc)),
    || extract_annotations(doc),
  );
  let text = text?;
  let structured_text = detect_headers_footers(&text);

  let mut image_pages: Vec<u32> = images
    .iter()
    .map(|i| i.page)
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();
  image_pages.sort_unstable();

  let mut annotation_pages: Vec<u32> = annotations
    .iter()
    .map(|a| a.page)
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();
  annotation_pages.sort_unstable();

  Ok(RawPdfDocument {
    meta,
    text,
    structured_text,
    images,
    annotations,
    image_pages,
    annotation_pages,
  })
}

#[cfg(feature = "ocr")]
pub(crate) fn extract_all_with_ocr(
  doc: &Document,
  lang: &str,
  min_len: u32,
  max_threads: u32,
) -> Result<RawPdfDocumentOcr> {
  let meta = extract_metadata(doc);
  let (text, (images, annotations)) = rayon::join(
    || extract_text_with_ocr(doc, lang, min_len, max_threads),
    || rayon::join(|| extract_images_raw(doc), || extract_annotations(doc)),
  );
  let text = text?;
  let structured_text = detect_headers_footers_ocr(&text);

  let mut image_pages: Vec<u32> = images
    .iter()
    .map(|i| i.page)
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();
  image_pages.sort_unstable();

  let mut annotation_pages: Vec<u32> = annotations
    .iter()
    .map(|a| a.page)
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();
  annotation_pages.sort_unstable();

  Ok(RawPdfDocumentOcr {
    meta,
    text,
    structured_text,
    images,
    annotations,
    image_pages,
    annotation_pages,
  })
}
