use super::*;

#[test]
fn source_ppi_comes_from_the_painted_size_and_falls_back_to_a_scan_default() {
  // A 1728 px wide fax raster painted 8.64 in wide (622.08 pt) is a 200 dpi scan.
  assert_eq!(source_ppi(1728, 622.08), 200);
  // 2480 px over an A4 width of 595.28 pt is 300 dpi.
  assert_eq!(source_ppi(2480, 595.28), 300);
  // No usable placement: Tesseract's own guess of 70 dpi is wrong for scans; assume 300.
  assert_eq!(source_ppi(1728, 0.0), DEFAULT_SOURCE_PPI);
  assert_eq!(source_ppi(1728, f64::NAN), DEFAULT_SOURCE_PPI);
  assert_eq!(source_ppi(0, 622.08), DEFAULT_SOURCE_PPI);
  // Tiny or giant placements clamp to the range Tesseract handles.
  assert_eq!(source_ppi(10_000, 10.0), MAX_SOURCE_PPI);
  assert_eq!(source_ppi(100, 10_000.0), MIN_SOURCE_PPI);
}
