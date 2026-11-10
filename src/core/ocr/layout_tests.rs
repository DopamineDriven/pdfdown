use super::*;

#[test]
fn pixel_boxes_map_onto_the_destination_rectangle() {
  let words = vec![OcrWord {
    text: "Hi".into(),
    left: 0,
    top: 0,
    right: 50,
    bottom: 10,
    confidence: 87.5,
  }];
  // 100×100 px image painted on [10, 20, 110, 220]: 1 px = 1 pt wide, 2 pt tall.
  let mapped = map_ocr_words(words, 100, 100, [10.0, 20.0, 110.0, 220.0]);
  let w = &mapped[0];
  assert_eq!((w.x0, w.x1), (10.0, 60.0));
  assert_eq!((w.y0, w.y1), (200.0, 220.0));
  assert_eq!(w.confidence, Some(0.875));
  assert_eq!(w.baseline, 200.0);
}
