use super::*;

/// `invoice()` reads two pages, so its OCR must not cost a whole document; `layoutWithOcr`
/// keeps reading every page.
#[test]
fn page_filter_ocrs_only_the_listed_pages() {
  let bytes = std::fs::read(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/ocr/__test__/fixtures/multipage-ocr.pdf"
  ))
  .unwrap();
  let doc = Document::load_mem(&bytes).unwrap();
  let all = extract_layout_with_ocr(&doc, &bytes, "eng", 1, 2, 150, 0, None).unwrap();
  assert!(all.len() >= 2);
  assert!(
    all.iter().all(|p| p.source == TextSource::Ocr),
    "{:?}",
    all.iter().map(|p| p.source).collect::<Vec<_>>()
  );
  let first = extract_layout_with_ocr(&doc, &bytes, "eng", 1, 2, 150, 0, Some(&[1])).unwrap();
  assert_eq!(first.len(), all.len());
  assert_eq!(first[0].source, TextSource::Ocr);
  assert!(!first[0].words.is_empty());
  assert!(
    first[1..]
      .iter()
      .all(|p| p.source == TextSource::Native && p.words.is_empty()),
    "pages off the list stay native"
  );
}
