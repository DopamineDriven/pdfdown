use super::*;

#[test]
fn matrix_composition_follows_pdf_order() {
  let scale = Matrix {
    a: 2.0,
    b: 0.0,
    c: 0.0,
    d: 2.0,
    e: 0.0,
    f: 0.0,
  };
  let shift = Matrix::translate(10.0, 5.0);
  // scale first, then shift: (1,1) -> (2,2) -> (12,7)
  assert_eq!(scale.mul(shift).apply(1.0, 1.0), (12.0, 7.0));
  // shift first, then scale: (1,1) -> (11,6) -> (22,12)
  assert_eq!(shift.mul(scale).apply(1.0, 1.0), (22.0, 12.0));
  assert_eq!(scale.scale_y(), 2.0);
}

#[test]
fn bbox_handles_rotation() {
  let rot90 = Matrix {
    a: 0.0,
    b: 1.0,
    c: -1.0,
    d: 0.0,
    e: 100.0,
    f: 100.0,
  };
  let (x0, y0, x1, y1) = bbox_of(rot90, 0.0, 0.0, 10.0, 2.0);
  assert!((x0 - 98.0).abs() < 1e-9 && (x1 - 100.0).abs() < 1e-9);
  assert!((y0 - 100.0).abs() < 1e-9 && (y1 - 110.0).abs() < 1e-9);
}
