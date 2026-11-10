use super::super::testing::{page, segments_at};
use super::*;

#[test]
fn a_line_splits_into_segments_at_a_wide_gap() {
  let p = page(&[
    (43.0, 700.0, 10.0, false, "From:"),
    (348.0, 700.0, 10.0, false, "Invoice Number INV-3337"),
  ]);
  let segs = segments_at(&p);
  assert_eq!(
    segs.iter().map(|s| s.text.as_str()).collect::<Vec<_>>(),
    ["From:", "Invoice Number INV-3337"]
  );
  assert_eq!(segs[1].words.len(), 3);
  assert_eq!(segs[1].font_size, 10.0);
  assert!(!segs[1].bold);
  assert_eq!(segs[1].ocr_confidence, None);
  let span = span_of(&p, &segs[1].words);
  assert_eq!(span.page, 1);
  assert!((span.x0 - 348.0).abs() < 1e-9 && span.y0 == 700.0 && span.y1 == 710.0);
}
