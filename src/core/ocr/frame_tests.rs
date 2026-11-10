use super::*;

#[test]
fn unrotating_a_pixel_box_undoes_the_raster_turn() {
  // an 800×200 raster turned 90° clockwise is 200×800; a box in the turned raster maps back
  assert_eq!(
    unrotate_pixel_box([140, 10, 180, 110], 90, 200, 800),
    [10, 20, 110, 60]
  );
  // 270° clockwise (= 90° counter-clockwise): the same original box lands elsewhere
  assert_eq!(
    unrotate_pixel_box([20, 690, 60, 790], 270, 200, 800),
    [10, 20, 110, 60]
  );
  // 180° keeps the dimensions
  assert_eq!(
    unrotate_pixel_box([690, 140, 790, 180], 180, 800, 200),
    [10, 20, 110, 60]
  );
  assert_eq!(
    unrotate_pixel_box([10, 20, 110, 60], 0, 800, 200),
    [10, 20, 110, 60]
  );
}

#[test]
fn turning_a_box_in_the_page_frame_moves_its_corners() {
  let page = [0.0, 0.0, 612.0, 792.0];
  assert_eq!(
    turn_box([0.0, 0.0, 10.0, 20.0], 90, page),
    [0.0, 602.0, 20.0, 612.0]
  );
  assert_eq!(
    turn_box([0.0, 0.0, 10.0, 20.0], 270, page),
    [772.0, 0.0, 792.0, 10.0]
  );
  assert_eq!(
    turn_box([0.0, 0.0, 10.0, 20.0], 180, page),
    [602.0, 772.0, 612.0, 792.0]
  );
  assert_eq!(
    turn_box([0.0, 0.0, 10.0, 20.0], 0, page),
    [0.0, 0.0, 10.0, 20.0]
  );
  // a CropBox away from the origin keeps its origin
  let cropped = [100.0, 50.0, 400.0, 250.0];
  assert_eq!(
    turn_box([100.0, 50.0, 110.0, 70.0], 90, cropped),
    [100.0, 340.0, 120.0, 350.0]
  );
}

#[test]
fn a_quarter_turn_followed_by_its_complement_is_the_identity() {
  let page = [0.0, 0.0, 612.0, 792.0];
  let turned_page = turn_box(page, 90, page);
  let b = [50.0, 700.0, 300.0, 720.0];
  assert_eq!(turn_box(turn_box(b, 90, page), 270, turned_page), b);
}

fn ocr_word(b: [i32; 4]) -> OcrWord {
  OcrWord {
    text: "w".into(),
    left: b[0],
    top: b[1],
    right: b[2],
    bottom: b[3],
    confidence: 90.0,
  }
}

fn close(a: [f64; 4], b: [f64; 4]) -> bool {
  a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9)
}

fn geo(rotate: u32) -> PageGeometry {
  PageGeometry {
    width: 612.0,
    height: 792.0,
    rotate,
    bbox: [0.0, 0.0, 612.0, 792.0],
  }
}

#[test]
fn rendered_words_land_in_the_page_frame_and_the_turn_includes_rotate() {
  // /Rotate 90: the portrait page renders landscape (792×612 px at 72 dpi); a word across the
  // displayed top-left corner is a vertical box at the bottom-left of the page's own frame
  let (words, turn) =
    place_rendered_words(vec![ocr_word([0, 0, 100, 20])], 0, (792, 612), &geo(90));
  assert_eq!(turn, 90);
  let w = &words[0];
  let b = [w.x0, w.y0, w.x1, w.y1];
  assert!(close(b, [0.0, 0.0, 20.0, 100.0]), "{b:?}");
  // turning the page frame by that turn puts it back across the displayed top-left corner
  let turned = turn_box(b, turn, geo(90).bbox);
  assert!(close(turned, [0.0, 592.0, 100.0, 612.0]), "{turned:?}");
  // no /Rotate: a box at the top-left of a raster turned 180° is at the bottom-right of the
  // rendered page, and the turn is the OCR turn alone
  let (words, turn) =
    place_rendered_words(vec![ocr_word([0, 0, 100, 20])], 180, (612, 792), &geo(0));
  assert_eq!(turn, 180);
  let w = &words[0];
  let b = [w.x0, w.y0, w.x1, w.y1];
  assert!(close(b, [512.0, 0.0, 612.0, 20.0]), "{b:?}");
  // /Rotate 90 plus a 270° OCR turn is no turn at all
  let (_, turn) = place_rendered_words(vec![], 270, (792, 612), &geo(90));
  assert_eq!(turn, 0);
}

#[test]
fn the_turn_carrying_the_most_confident_text_sets_the_page_rotation() {
  assert_eq!(dominant_rotation(&[(270, 10.0), (180, 3.0)]), 270);
  // tiles vote together: two upright tiles outweigh one bigger tile a retry turned
  assert_eq!(dominant_rotation(&[(90, 6.0), (0, 4.0), (0, 4.0)]), 0);
  assert_eq!(
    dominant_rotation(&[(0, 5.0), (90, 5.0)]),
    0,
    "ties keep the first turn seen"
  );
  assert_eq!(dominant_rotation(&[]), 0);
}
