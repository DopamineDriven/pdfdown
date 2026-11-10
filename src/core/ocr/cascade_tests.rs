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
