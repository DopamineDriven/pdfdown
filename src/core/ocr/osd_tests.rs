use super::*;

#[test]
fn osd_below_threshold_is_ignored() {
  assert_eq!(pick_first_rotation(Some((90, 2.9))), None);
}

#[test]
fn osd_seeds_the_first_pass() {
  assert_eq!(pick_first_rotation(Some((90, 6.0))), Some(90));
  assert_eq!(pick_first_rotation(Some((0, 10.0))), Some(0));
}

#[test]
fn no_osd_means_no_seed() {
  assert_eq!(pick_first_rotation(None), None);
}

#[test]
fn convincing_upright_reads_skip_the_detector() {
  assert!(upright_is_convincing(182, 95.6), "a real upright page");
  assert!(
    !upright_is_convincing(24, 62.4),
    "the synthetic page read upside down"
  );
  assert!(
    !upright_is_convincing(449, 41.4),
    "a real page read sideways"
  );
  assert!(
    !upright_is_convincing(0, 100.0),
    "no words is never convincing"
  );
}

#[test]
fn osd_orientation_becomes_a_clockwise_correction() {
  // Tesseract reports how the text currently lies; the CLI prints "Orientation 270 / Rotate 90".
  assert_eq!(osd_correction(270), 90);
  assert_eq!(osd_correction(90), 270);
  assert_eq!(osd_correction(180), 180);
  assert_eq!(osd_correction(0), 0);
}
